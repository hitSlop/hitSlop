//! Room lifecycle and opaque relay records; private payloads in R2.
use crate::frame::{Ack, Frame, PROTOCOL, Permission, Record, Span, Version};
use crate::metrics::{Metric, record};
use hitslop_sharing_protocol::{
    CONTROL_BYTES, SEED_BYTES, SOCKET_PROTOCOL,
    http::{CreatedRoom, RoomInfo, RoomInit, RoomPhase},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
use worker::*;
#[path = "resources.rs"]
mod resources;

const CHUNK: usize = 1_800_000;
const BACKFILL: usize = 1 << 20;
use hitslop_sharing_protocol::limits::*;
const DAY_MS: u64 = 86_400_000;
fn now() -> u64 {
    js_sys::Date::now() as u64
}

#[derive(Debug)]
enum SocketFailure {
    Refused(u8, String),
    Transient(worker::Error),
}
impl From<worker::Error> for SocketFailure {
    fn from(e: worker::Error) -> Self {
        Self::Transient(e)
    }
}
type SocketResult = std::result::Result<(), SocketFailure>;
fn refused(code: u8, message: &str) -> SocketFailure {
    SocketFailure::Refused(code, message.into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn hash(key: &str) -> String {
    hex(&Sha256::digest(key.as_bytes()))
}
fn fail(s: impl Into<String>) -> worker::Error {
    worker::Error::RustError(s.into())
}
fn version(value: &[(String, u32)]) -> Result<Version> {
    value
        .iter()
        .map(|(peer, end)| {
            if peer.len() != 16 || *end > i32::MAX as u32 {
                return Err(fail("Invalid version"));
            }
            Ok((u64::from_str_radix(peer, 16).map_err(|_| fail("Invalid peer"))?, *end))
        })
        .collect()
}
fn wire_version(vv: &Version) -> Vec<(String, u32)> {
    vv.iter().map(|(p, n)| (format!("{p:016x}"), *n)).collect()
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _: Context) -> Result<Response> {
    let response = match serve(req, env.clone()).await {
        Ok(r) => r,
        Err(_) => Response::error("Request failed", 500)?,
    };
    let headers = response.headers().clone();
    let mut response = response.with_headers(headers);
    response.headers_mut().set("Cache-Control", "no-store")?;
    if response.status_code() < 400 {
        return Ok(response);
    }
    let status = response.status_code();
    if status == 429 {
        record(&env, Metric::Limited);
    }
    let headers = response.headers().clone();
    let mut response = Response::from_json(&hitslop_sharing_protocol::http::ApiFailure::refused(status))?
        .with_status(status)
        .with_headers(headers);
    response.headers_mut().set("Content-Type", "application/json")?;
    Ok(response)
}
async fn serve(mut req: Request, env: Env) -> Result<Response> {
    if req.path().starts_with("/assets/")
        || req.path().starts_with("/attachments/")
        || req.path().starts_with("/_resources/")
    {
        return resources::entry(req, env).await;
    }
    let origin = req.headers().get("Origin")?;
    let allowed = env.var("BROWSER_ORIGIN").ok().map(|v| v.to_string());
    if origin.is_some() && origin != allowed {
        return Response::error("Origin refused", 403);
    }
    if req.method() == Method::Options {
        if origin.is_none() {
            return Response::error("Origin required", 403);
        }
        let mut response = Response::empty()?.with_status(204);
        response.headers_mut().set("Access-Control-Allow-Origin", origin.as_deref().unwrap())?;
        response.headers_mut().set("Access-Control-Allow-Methods", "GET, HEAD, POST, PUT, DELETE")?;
        response.headers_mut().set(
            "Access-Control-Allow-Headers",
            "Authorization, Content-Type, X-Sharing-SHA256, X-Sharing-App, X-Sharing-Version, X-Sharing-Root",
        )?;
        response.headers_mut().set("Vary", "Origin")?;
        return Ok(response);
    }
    let browser_socket = req.headers().get("Upgrade")?.as_deref() == Some("websocket") && origin.is_some();
    if browser_socket {
        req = req.clone_mut()?;
        let protocols = req.headers().get("Sec-WebSocket-Protocol")?.unwrap_or_default();
        let values: Vec<_> = protocols.split(',').map(str::trim).collect();
        if values.len() != 2 || values[0] != SOCKET_PROTOCOL {
            return Response::error("Protocol refused", 403);
        }
        let Some(cap) = values[1].strip_prefix("cap.") else {
            return Response::error("Denied", 403);
        };
        if !crate::resource_types::hash_id(cap) {
            return Response::error("Denied", 403);
        }
        req.headers_mut()?.set("Authorization", &format!("Bearer {cap}"))?;
        // Only the public protocol is selected; never echo credentials in replies.
        req.headers_mut()?.delete("Sec-WebSocket-Protocol")?;
    }
    if req.headers().get("Upgrade")?.as_deref() == Some("websocket")
        && !browser_socket
        && req.headers().get("Sec-WebSocket-Protocol")?.as_deref() != Some(SOCKET_PROTOCOL)
    {
        return Response::error("Protocol refused", 403);
    }
    let path = req.path();
    if path == "/health" {
        return Response::ok("hitslop-sharing");
    }
    if let Some(binding) = crate::abuse::guest_limit(req.method().as_ref(), &path) {
        // Cloudflare sets this header. Missing IPs share a conservative fallback budget.
        let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_else(|| "unknown".into());
        if !env.rate_limiter(binding)?.limit(ip).await?.success {
            let mut response = Response::error("Limited", 429)?;
            if let Some(origin) = origin {
                response.headers_mut().set("Access-Control-Allow-Origin", &origin)?;
                response.headers_mut().set("Vary", "Origin")?;
            }
            return Ok(response);
        }
    }
    let mut response = if path == "/v1/rooms" || path == "/v1/account" || path.starts_with("/v1/account/rooms/") {
        let claims = match crate::auth::identity(&req, &env).await {
            Ok(c) => c,
            Err(e) => return identity_response(e),
        };
        env.durable_object("ACCOUNTS")?.get_by_name(&hash(&claims.sub))?.fetch_with_request(req).await?
    } else {
        let parts: Vec<_> = path.split('/').collect();
        if !(4..=6).contains(&parts.len())
            || parts[1] != "v1"
            || parts[2] != "rooms"
            || parts[3].len() != 32
            || !parts[3].bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Response::error("Not found", 404);
        }
        env.durable_object("ROOMS")?.get_by_name(parts[3])?.fetch_with_request(req).await?
    };
    let headers = response.headers().clone();
    response = response.with_headers(headers);
    if let Some(origin) = origin {
        response.headers_mut().set("Access-Control-Allow-Origin", &origin)?;
        response.headers_mut().set("Vary", "Origin")?;
    }
    if response.status_code() == 101 {
        response.headers_mut().set("Sec-WebSocket-Protocol", SOCKET_PROTOCOL)?;
    }
    Ok(response)
}

// worker 0.8.7 exposes SQL but not transactionSync. Bind that one native method;
// this is platform glue, not a second implementation of room rules.
#[wasm_bindgen]
extern "C" {
    type RawStorage;
    #[wasm_bindgen(method, catch, js_name = transactionSync)]
    fn transaction_sync(this: &RawStorage, callback: &js_sys::Function) -> std::result::Result<JsValue, JsValue>;
}

#[derive(Deserialize, Serialize, Clone, Default)]
struct SocketState {
    joined: bool,
    #[serde(default)]
    ip: String,
    #[serde(default)]
    window_start: u64,
    #[serde(default)]
    bytes: usize,
    #[serde(default)]
    pushes: usize,
}
#[derive(Deserialize)]
struct TextRow {
    value: String,
}
#[derive(Deserialize)]
struct NumberRow {
    n: i64,
}
#[derive(Deserialize)]
struct RecordRow {
    seq: i64,
    spans: String,
    size: usize,
}
#[derive(Deserialize)]
struct BlobRow {
    #[serde(with = "serde_bytes")]
    bytes: Vec<u8>,
}
#[durable_object]
pub struct SlopRoom {
    state: State,
    env: Env,
    sql: SqlStorage,
    uploads: futures_util::lock::Mutex<()>,
}

impl SlopRoom {
    fn text(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .sql
            .exec("SELECT value FROM meta WHERE key=?", vec![key.into()])?
            .to_array::<TextRow>()?
            .into_iter()
            .next()
            .map(|r| r.value))
    }
    fn put(&self, key: &str, value: &str) -> Result<()> {
        self.sql.exec(
            "INSERT INTO meta VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            vec![key.into(), value.into()],
        )?;
        Ok(())
    }
    fn heads(&self) -> Result<Version> {
        let text = self.text("heads")?.unwrap_or_else(|| "[]".into());
        version(&serde_json::from_str::<Vec<(String, u32)>>(&text)?)
    }
    fn transaction(&self, f: impl FnMut() -> Result<()> + 'static) -> Result<()> {
        let mut f = f;
        let callback = Closure::wrap(Box::new(move || {
            f().map(|_| JsValue::UNDEFINED).map_err(|e| JsValue::from_str(&e.to_string()))
        }) as Box<dyn FnMut() -> std::result::Result<JsValue, JsValue>>);
        let storage = self.state.storage();
        let raw: &RawStorage = storage.as_raw().unchecked_ref();
        raw.transaction_sync(callback.as_ref().unchecked_ref()).map_err(worker::Error::from)?;
        Ok(())
    }
    fn phase(&self) -> Result<Option<RoomPhase>> {
        self.text("phase")?.map(|v| serde_json::from_str(&v).map_err(Into::into)).transpose()
    }
    fn set_phase(&self, phase: RoomPhase) -> Result<()> {
        self.put("phase", &serde_json::to_string(&phase)?)
    }
    fn info(&self, owner: bool) -> Result<RoomInfo> {
        Ok(RoomInfo {
            document: self.text("document")?.ok_or_else(|| fail("Missing document"))?,
            phase: self.phase()?.ok_or_else(|| fail("Missing phase"))?,
            seed_hash: self.text("seed_hash")?,
            seed_size: self.text("seed_size")?.map(|s| s.parse().map_err(|_| fail("Invalid seed size"))).transpose()?,
            app: self.text("app")?,
            compact: self.compact_due()?,
            capability: if owner { self.text("capability")? } else { None },
        })
    }
    fn update_usage(&self) -> Result<(usize, usize)> {
        let bytes =
            self.sql.exec("SELECT coalesce(sum(size),0) AS n FROM records WHERE seq>0", None)?.one::<NumberRow>()?.n
                as usize;
        let rows = self.sql.exec("SELECT count(*) AS n FROM records WHERE seq>0", None)?.one::<NumberRow>()?.n as usize;
        Ok((bytes, rows))
    }
    fn compact_due(&self) -> Result<bool> {
        let (bytes, rows) = self.update_usage()?;
        Ok(bytes >= COMPACT_AFTER_BYTES || rows >= COMPACT_AFTER_RECORDS)
    }
    async fn sweep_archives(&self) -> Result<()> {
        let Some(room) = self.text("room")? else {
            return Ok(());
        };
        let bucket = self.env.bucket("SEEDS")?;
        let mut cursor = None;
        loop {
            let mut list = bucket.list().prefix(format!("rooms/{room}/compacted/"));
            if let Some(c) = cursor {
                list = list.cursor(c);
            }
            let listed = list.execute().await?;
            for item in listed.objects() {
                if bucket
                    .head(item.key())
                    .await?
                    .and_then(|o| o.custom_metadata().ok())
                    .and_then(|m| m.get("expires").and_then(|s| s.parse::<u64>().ok()))
                    .is_some_and(|expiry| expiry <= now())
                {
                    bucket.delete(item.key()).await?;
                }
            }
            if !listed.truncated() {
                break;
            }
            cursor = listed.cursor();
        }
        Ok(())
    }
    async fn compact(&self, req: &mut Request, room: &str) -> Result<Response> {
        // Serialize competing archives; Push and Stop may still progress across the R2 await.
        let _guard = self.uploads.lock().await;
        let body = match bounded_body(req, BASE_BYTES).await {
            Ok(body) => body,
            Err(worker::Error::RustError(message)) if message == "Request exceeds its byte limit" => {
                return Response::error(message, 413);
            }
            Err(error) => return Err(error),
        };
        match self.phase()? {
            Some(RoomPhase::Live) => {}
            Some(RoomPhase::Uploading) => return Response::error("Publication pending", 409),
            Some(RoomPhase::Ended) => return Response::error("Sharing ended", 410),
            None => return Response::error("Unknown room", 404),
        }
        let digest = req.headers().get("X-Sharing-SHA256")?.unwrap_or_default();
        let parse = |text: Option<String>| -> Option<Version> {
            let text = text?;
            if text.len() > CONTROL_BYTES {
                return None;
            }
            let pairs: Vec<(String, u32)> = serde_json::from_str(&text).ok()?;
            let vv = version(&pairs).ok()?;
            (vv.len() == pairs.len()).then_some(vv)
        };
        let Some(frontier) = parse(req.headers().get("X-Sharing-Version")?) else {
            return Response::error("Invalid version", 400);
        };
        let root = match self.text("root")? {
            Some(root) => parse(Some(root)),
            None => parse(req.headers().get("X-Sharing-Root")?),
        };
        let Some(root) = root else {
            return Response::error("Root version required", 400);
        };
        let previous_json = self.text("base_version")?;
        let previous = match &previous_json {
            Some(value) => parse(Some(value.clone())).ok_or_else(|| fail("Invalid stored base version"))?,
            None => root.clone(),
        };
        let heads = self.heads()?;
        if self.phase()? != Some(RoomPhase::Live) {
            return Response::error("Sharing ended", 410);
        }
        if !self.compact_due()?
            || frontier.iter().any(|(p, n)| *n > heads.get(p).copied().unwrap_or(0))
            || previous.iter().chain(root.iter()).any(|(p, n)| frontier.get(p).copied().unwrap_or(0) < *n)
            || !frontier.iter().any(|(p, n)| *n > previous.get(p).copied().unwrap_or(0))
        {
            return Response::error("Compaction conflict", 409);
        }
        if !crate::resource_types::hash_id(&digest) || hex(&Sha256::digest(&body)) != digest {
            return Response::error("Base checksum mismatch", 400);
        }
        let mut covered = vec![];
        let mut archive = vec![];
        for row in self.sql.exec("SELECT seq,spans,size FROM records ORDER BY seq", None)?.next::<RecordRow>() {
            let row = row?;
            let spans: Vec<(String, u32, u32)> = serde_json::from_str(&row.spans)?;
            let spans: Vec<Span> = spans
                .into_iter()
                .map(|(p, start, end)| {
                    Ok(Span { peer: u64::from_str_radix(&p, 16).map_err(|_| fail("Stored peer"))?, start, end })
                })
                .collect::<Result<_>>()?;
            if row.seq == 0 || spans.iter().all(|s| s.end <= frontier.get(&s.peer).copied().unwrap_or(0)) {
                let frame = Frame::Records(vec![Record { spans, bytes: self.bytes(row.seq, row.size)? }]).encode();
                archive.extend_from_slice(&(frame.len() as u32).to_be_bytes());
                archive.extend(frame);
                covered.push(row.seq);
            }
        }
        let n = self.text("archive_counter")?.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0) + 1;
        self.put("archive_counter", &n.to_string())?;
        let key = format!("rooms/{room}/compacted/{n}");
        let bucket = self.env.bucket("SEEDS")?;
        if self.state.storage().get_alarm().await?.is_none_or(|alarm| alarm as u64 > now() + DAY_MS) {
            self.state.storage().set_alarm(std::time::Duration::from_millis(DAY_MS)).await?;
        }
        bucket
            .put(key.clone(), archive)
            .custom_metadata(std::collections::HashMap::from([(
                "expires".into(),
                (now() + COMPACTED_ARCHIVE_DAYS * DAY_MS).to_string(),
            )]))
            .execute()
            .await?;
        if self.phase()? != Some(RoomPhase::Live) {
            bucket.delete(key).await?;
            return Response::error("Sharing ended", 410);
        }
        // No await within the swap. Recheck the generation and due state inside transactionSync.
        let sql = self.sql.clone();
        let frontier_json = serde_json::to_string(&wire_version(&frontier))?;
        let root_json = serde_json::to_string(&wire_version(&root))?;
        let spans: Vec<_> = frontier
            .iter()
            .filter_map(|(p, end)| {
                let start = root.get(p).copied().unwrap_or(0);
                (*end > start).then(|| (format!("{p:016x}"), start, *end))
            })
            .collect();
        let spans_json = serde_json::to_string(&spans)?;
        let installed = std::rc::Rc::new(std::cell::Cell::new(false));
        let swapped = installed.clone();
        self.transaction(move || {
            let get = |key: &str| -> Result<Option<String>> {
                Ok(sql
                    .exec("SELECT value FROM meta WHERE key=?", vec![key.into()])?
                    .to_array::<TextRow>()?
                    .into_iter()
                    .next()
                    .map(|r| r.value))
            };
            let bytes =
                sql.exec("SELECT coalesce(sum(size),0) AS n FROM records WHERE seq>0", None)?.one::<NumberRow>()?.n
                    as usize;
            let rows = sql.exec("SELECT count(*) AS n FROM records WHERE seq>0", None)?.one::<NumberRow>()?.n as usize;
            if get("phase")?.as_deref() != Some("\"live\"")
                || get("base_version")? != previous_json
                || (bytes < COMPACT_AFTER_BYTES && rows < COMPACT_AFTER_RECORDS)
            {
                return Ok(());
            }
            let committed_heads =
                version(&serde_json::from_str::<Vec<(String, u32)>>(&get("heads")?.unwrap_or_else(|| "[]".into()))?)?;
            if frontier.iter().any(|(p, n)| *n > committed_heads.get(p).copied().unwrap_or(0))
                || previous.iter().chain(root.iter()).any(|(p, n)| frontier.get(p).copied().unwrap_or(0) < *n)
                || !frontier.iter().any(|(p, n)| *n > previous.get(p).copied().unwrap_or(0))
            {
                return Ok(());
            }
            for seq in &covered {
                sql.exec("DELETE FROM chunks WHERE seq=?", vec![(*seq).into()])?;
                sql.exec("DELETE FROM records WHERE seq=?", vec![(*seq).into()])?;
            }
            sql.exec(
                "INSERT INTO records(seq,spans,size) VALUES(0,?,?)",
                vec![spans_json.clone().into(), (body.len() as i64).into()],
            )?;
            for (part, chunk) in body.chunks(CHUNK).enumerate() {
                sql.exec("INSERT INTO chunks VALUES(0,?,?)", vec![(part as i64).into(), chunk.to_vec().into()])?;
            }
            for (key, value) in [("base_version", &frontier_json), ("root", &root_json)] {
                sql.exec(
                    "INSERT INTO meta VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    vec![key.into(), value.clone().into()],
                )?;
            }
            swapped.set(true);
            Ok(())
        })?;
        if !installed.get() {
            return Response::error("Compaction conflict", 409);
        }
        self.state.storage().sync().await?;
        self.sweep_archives().await?;
        let expiry = now() + COMPACTED_ARCHIVE_DAYS * DAY_MS;
        if self.state.storage().get_alarm().await?.is_none_or(|alarm| alarm as u64 > expiry) {
            self.state.storage().set_alarm(std::time::Duration::from_millis(COMPACTED_ARCHIVE_DAYS * DAY_MS)).await?;
        }
        Response::empty().map(|r| r.with_status(204))
    }
    async fn active(&self) -> Result<()> {
        let last = self.text("last_active")?.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        if now().saturating_sub(last) >= 3_600_000 {
            self.put("last_active", &now().to_string())?;
            if self.state.storage().get_alarm().await?.is_none() {
                self.state.storage().set_alarm(std::time::Duration::from_millis(ROOM_IDLE_DAYS * DAY_MS)).await?;
            }
        }
        Ok(())
    }
    async fn authorized(&self, req: &Request, admin: bool) -> Result<bool> {
        if admin {
            let claims = match crate::auth::identity(req, &self.env).await {
                Ok(c) => c,
                Err(_) => return Ok(false),
            };
            return Ok(self.text("owner_uid")?.as_ref() == Some(&claims.sub));
        }
        let key = req.headers().get("Authorization")?.unwrap_or_default();
        let Some(key) = key.strip_prefix("Bearer ") else { return Ok(false) };
        // Compare fixed-length digests so response timing does not reveal a matching prefix.
        Ok(self.text("capability")?.is_some_and(|stored| hash(&stored) == hash(key)))
    }
    async fn stop_room(&self, room: &str, reason: Metric) -> Result<Response> {
        let was_ended = self.phase()? == Some(RoomPhase::Ended);
        self.set_phase(RoomPhase::Ended)?;
        if self.text("ended_at")?.is_none() {
            self.put("ended_at", &now().to_string())?;
        }
        if !was_ended {
            record(&self.env, reason);
        }
        // A retry alarm finishes cleanup and quota release after a transient failure.
        self.state.storage().set_alarm(std::time::Duration::from_secs(60)).await?;
        for ws in self.state.get_websockets() {
            let _ = ws.serialize_attachment(SocketState::default());
            let _ = self.send(&ws, Frame::Error { code: crate::frame::ERROR_ENDED, message: "Sharing ended".into() });
            let _ = ws.close(Some(1008), Some("Sharing ended"));
        }
        let bucket = self.env.bucket("SEEDS")?;
        loop {
            let listed = bucket.list().prefix(format!("{room}/")).execute().await?;
            let items = listed.objects();
            if items.is_empty() {
                break;
            }
            for item in items {
                bucket.delete(item.key()).await?;
            }
        }
        loop {
            let listed = bucket.list().prefix(format!("rooms/{room}/compacted/")).execute().await?;
            if listed.objects().is_empty() {
                break;
            }
            for item in listed.objects() {
                bucket.delete(item.key()).await?;
            }
        }
        self.sql.exec("DELETE FROM resource_sessions; DELETE FROM resource_blobs; DELETE FROM records; DELETE FROM chunks; DELETE FROM meta WHERE key NOT IN ('phase','ended_at','owner_uid','document','room')", None)?;
        self.state.storage().sync().await?;
        let uid = self.text("owner_uid")?.ok_or_else(|| fail("Missing owner"))?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_body(Some(room.into()));
        let response = self
            .env
            .durable_object("ACCOUNTS")?
            .get_by_name(&hash(&uid))?
            .fetch_with_request(Request::new_with_init("https://room.internal/internal/release", &init)?)
            .await?;
        if response.status_code() != 200 {
            return Err(fail("Account release failed"));
        }
        let ended = self.text("ended_at")?.unwrap_or_else(|| now().to_string());
        self.sql.exec("DELETE FROM meta", None)?;
        self.set_phase(RoomPhase::Ended)?;
        self.put("ended_at", &ended)?;
        let ended_ms = ended.parse::<u64>().map_err(|_| fail("Invalid ended time"))?;
        self.state
            .storage()
            .set_alarm(std::time::Duration::from_millis(
                (ended_ms + ENDED_TOMBSTONE_DAYS * DAY_MS).saturating_sub(now()).max(1),
            ))
            .await?;
        Response::ok("ended")
    }
    fn bytes(&self, seq: i64, size: usize) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(size);
        for chunk in
            self.sql.exec("SELECT bytes FROM chunks WHERE seq=? ORDER BY part", vec![seq.into()])?.next::<BlobRow>()
        {
            bytes.extend(chunk?.bytes);
        }
        if bytes.len() != size {
            return Err(fail("Incomplete stored record"));
        }
        Ok(bytes)
    }
    fn send(&self, ws: &WebSocket, frame: Frame) -> Result<()> {
        ws.send_with_bytes(crate::envelope::encode(frame).map_err(fail)?)
    }
    fn hello(&self, ws: &WebSocket, protocol: u8, app: &[u8], vv: Version) -> Result<()> {
        if protocol != PROTOCOL || self.text("app")?.as_deref() != Some(&hex(app)) {
            return Err(fail("Different app or protocol"));
        }
        self.send(ws, Frame::Welcome { permission: Permission::Write, version: self.heads()? })?;
        let mut batch = vec![];
        let mut size = 0;
        for row in self.sql.exec("SELECT seq,spans,size FROM records ORDER BY seq", None)?.next::<RecordRow>() {
            let row = row?;
            let spans: Vec<(String, u32, u32)> = serde_json::from_str(&row.spans)?;
            let spans: Vec<Span> = spans
                .into_iter()
                .map(|(p, start, end)| {
                    Ok(Span { peer: u64::from_str_radix(&p, 16).map_err(|_| fail("Stored peer"))?, start, end })
                })
                .collect::<Result<_>>()?;
            if !spans.iter().any(|s| s.end > vv.get(&s.peer).copied().unwrap_or(0)) {
                continue;
            }
            if size + row.size > BACKFILL && !batch.is_empty() {
                self.send(ws, Frame::Records(std::mem::take(&mut batch)))?;
                size = 0;
            }
            size += row.size;
            batch.push(Record { spans, bytes: self.bytes(row.seq, row.size)? });
        }
        if !batch.is_empty() {
            self.send(ws, Frame::Records(batch))?;
        }
        self.send(ws, Frame::CaughtUp)?;
        let mut s = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        s.joined = true;
        ws.serialize_attachment(s)
    }
    async fn push(&self, ws: &WebSocket, id: u64, records: Vec<Record>, wire_bytes: usize) -> Result<()> {
        let mut budget = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        if now().saturating_sub(budget.window_start) >= 60_000 {
            budget.window_start = now();
            budget.bytes = 0;
            budget.pushes = 0;
        }
        budget.bytes = budget.bytes.saturating_add(wire_bytes);
        budget.pushes += 1;
        let limited = budget.bytes > SOCKET_PUSH_BYTES_PER_MINUTE || budget.pushes > SOCKET_PUSHES_PER_MINUTE;
        ws.serialize_attachment(budget)?;
        if limited {
            ws.close(Some(1013), Some("Push budget exceeded"))?;
            return Ok(());
        }
        if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.joined) {
            return Err(fail("Hello required"));
        }
        let mut heads = self.heads()?;
        let mut added = vec![];
        // Validate every record before storing any of the push.
        for record in records {
            if record.spans.iter().any(|s| s.start > heads.get(&s.peer).copied().unwrap_or(0)) {
                return self.send(ws, Frame::Ack { id, status: Ack::Gap });
            }
            if record.spans.iter().all(|s| s.end <= heads.get(&s.peer).copied().unwrap_or(0)) {
                continue;
            }
            for s in &record.spans {
                heads.entry(s.peer).and_modify(|n| *n = (*n).max(s.end)).or_insert(s.end);
            }
            added.push(record);
        }
        let (stored, rows) = self.update_usage()?;
        if stored + added.iter().map(|r| r.bytes.len()).sum::<usize>() > ROOM_RECORD_BYTES
            || rows >= ROOM_RECORDS
            || rows + added.len() > ROOM_RECORDS
        {
            return self.send(ws, Frame::Ack { id, status: Ack::Quota });
        }
        let sql = self.sql.clone();
        let save = added.clone();
        let heads_json = serde_json::to_string(&wire_version(&heads))?;
        self.transaction(move || {
            for record in &save {
                let spans: Vec<_> = record.spans.iter().map(|s| (format!("{:016x}", s.peer), s.start, s.end)).collect();
                let seq = sql
                    .exec(
                        "INSERT INTO records(spans,size) VALUES(?,?) RETURNING seq AS n",
                        vec![serde_json::to_string(&spans)?.into(), (record.bytes.len() as i64).into()],
                    )?
                    .one::<NumberRow>()?
                    .n;
                for (part, chunk) in record.bytes.chunks(CHUNK).enumerate() {
                    sql.exec(
                        "INSERT INTO chunks VALUES(?,?,?)",
                        vec![seq.into(), (part as i64).into(), chunk.to_vec().into()],
                    )?;
                }
            }
            sql.exec("UPDATE meta SET value=? WHERE key='heads'", vec![heads_json.clone().into()])?;
            Ok(())
        })?;
        self.state.storage().sync().await?;
        if self.phase()? == Some(RoomPhase::Ended) {
            return Err(fail("Sharing ended"));
        }
        self.active().await?;
        self.send(ws, Frame::Ack { id, status: Ack::Stored })?;
        for other in self.state.get_websockets() {
            // Echo is harmless and keeps broadcast routing independent of JS object identity.
            if other.deserialize_attachment::<SocketState>().ok().flatten().is_some_and(|s| s.joined) {
                for record in &added {
                    if self.send(&other, Frame::Records(vec![record.clone()])).is_err() {
                        let _ = other.close(Some(1011), None::<String>);
                        break;
                    }
                }
            }
        }
        Ok(())
    }
    async fn message(&self, ws: &WebSocket, msg: WebSocketIncomingMessage) -> SocketResult {
        if self.phase()? != Some(RoomPhase::Live) {
            return Err(refused(crate::frame::ERROR_ENDED, "Sharing ended"));
        }
        let WebSocketIncomingMessage::Binary(bytes) = msg else {
            return Err(refused(crate::frame::ERROR_MALFORMED, "Binary frame required"));
        };
        let frame =
            crate::envelope::decode(&bytes).map_err(|e| refused(crate::frame::ERROR_MALFORMED, &e.to_string()))?;
        match frame {
            Frame::Hello { protocol, app_digest, version } => {
                if protocol != PROTOCOL {
                    return Err(refused(crate::frame::ERROR_PROTOCOL, "Different app or protocol"));
                }
                let expected_app = self.text("app")?.ok_or_else(|| fail("Missing app"))?;
                if expected_app != hex(&app_digest) {
                    return Err(refused(crate::frame::ERROR_APP, "Different app or protocol"));
                }
                self.hello(ws, protocol, &app_digest, version)?;
                self.active().await?;
                Ok(())
            }
            Frame::Push { id, records } => {
                if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.joined) {
                    return Err(refused(crate::frame::ERROR_MALFORMED, "Hello required"));
                }
                self.push(ws, id, records, bytes.len()).await?;
                Ok(())
            }
            _ => Err(refused(crate::frame::ERROR_MALFORMED, "Client frame required")),
        }
    }
}

fn initialize(sql: &SqlStorage) {
    sql.exec("CREATE TABLE IF NOT EXISTS resource_sessions(token TEXT PRIMARY KEY,host TEXT NOT NULL,app TEXT NOT NULL,expires REAL NOT NULL); CREATE TABLE IF NOT EXISTS resource_blobs(id TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS records(seq INTEGER PRIMARY KEY AUTOINCREMENT,spans TEXT NOT NULL,size INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS chunks(seq INTEGER NOT NULL,part INTEGER NOT NULL,bytes BLOB NOT NULL,PRIMARY KEY(seq,part))",None).expect("create sharing tables");
}

fn decode_hex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return Err(fail("Invalid hex"));
    }
    text.as_bytes()
        .chunks(2)
        .map(|p| {
            std::str::from_utf8(p).ok().and_then(|s| u8::from_str_radix(s, 16).ok()).ok_or_else(|| fail("Invalid hex"))
        })
        .collect()
}
impl DurableObject for SlopRoom {
    fn new(state: State, env: Env) -> Self {
        state.set_websocket_auto_response(
            &worker::worker_sys::WebSocketRequestResponsePair::new("ping", "pong").expect("valid ping/pong"),
        );
        let sql = state.storage().sql();
        initialize(&sql);
        Self { state, env, sql, uploads: futures_util::lock::Mutex::new(()) }
    }
    async fn fetch(&self, mut req: Request) -> Result<Response> {
        if req.path().starts_with("/assets/")
            || req.path().starts_with("/attachments/")
            || req.path().starts_with("/_resources/")
        {
            return self.resource_fetch(req).await;
        }
        let path = req.path();
        let parts: Vec<_> = path.split('/').collect();
        if path.starts_with("/internal/rooms/")
            && parts.len() == 5
            && parts[4] == "init"
            && req.method() == Method::Post
        {
            let room = parts[3];
            let body = bounded_body(&mut req, CONTROL_BYTES).await?;
            let input: RoomInit = serde_json::from_slice(&body)?;
            if !hitslop_sharing_protocol::valid_copy(&input.document) || input.owner_uid.is_empty() {
                return Response::error("Invalid metadata", 400);
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            if self.phase()?.is_none() {
                self.put("owner_uid", &input.owner_uid)?;
                self.put("document", &input.document)?;
                self.put("room", room)?;
                self.put("capability", &random_hex(32)?)?;
                self.set_phase(RoomPhase::Uploading)?;
                record(&self.env, Metric::RoomCreated);
            }
            // A retry also repairs a failed alarm write; no accepted Uploading room
            // is left without its abandonment cleanup.
            if self.phase()? == Some(RoomPhase::Uploading) {
                self.state.storage().set_alarm(std::time::Duration::from_secs(24 * 60 * 60)).await?;
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            if self.text("owner_uid")?.as_ref() != Some(&input.owner_uid)
                || self.text("document")?.as_ref() != Some(&input.document)
            {
                return Response::error("Denied", 403);
            }
            return Response::from_json(&CreatedRoom {
                room: room.into(),
                capability: self.text("capability")?.ok_or_else(|| fail("Missing capability"))?,
                phase: self.phase()?.ok_or_else(|| fail("Missing phase"))?,
            });
        }
        if !(4..=6).contains(&parts.len()) {
            return Response::error("Not found", 404);
        }
        let room = parts[3];
        let route = parts.get(4).copied().unwrap_or("");
        let Some(phase) = self.phase()? else {
            return Response::error("Unknown room", 404);
        };
        if phase == RoomPhase::Live && self.text("last_active")?.is_none() {
            // Rooms created before idle expiry get their first deadline on access.
            self.active().await?;
        }
        if phase == RoomPhase::Ended && self.text("owner_uid")?.is_none() {
            return Response::error("Sharing ended", 410);
        }
        if route.is_empty() && req.method() == Method::Delete {
            if !self.authorized(&req, true).await? {
                return Response::error("Denied", 403);
            }
            return self.stop_room(room, Metric::RoomEndedStop).await;
        }
        if phase == RoomPhase::Ended {
            return Response::error("Sharing ended", 410);
        }
        if route.is_empty() && req.method() == Method::Get {
            let guest = self.authorized(&req, false).await?;
            let owner = !guest && self.authorized(&req, true).await?;
            if !guest && !owner {
                return Response::error("Denied", 403);
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            if guest && self.phase()? != Some(RoomPhase::Live) {
                return Response::error("Publication pending", 409);
            }
            return Response::from_json(&self.info(owner)?);
        }
        let owner_write = matches!(
            (route, req.method()),
            ("open", Method::Post) | ("seed", Method::Put) | ("resources", Method::Post) | ("asset", Method::Put)
        );
        let blob_upload = route == "blob" && req.method() == Method::Put;
        let owner = if owner_write || (blob_upload && phase == RoomPhase::Uploading) {
            if !self.authorized(&req, true).await? {
                return Response::error("Denied", 403);
            }
            true
        } else {
            if !self.authorized(&req, false).await? {
                return Response::error("Denied", 403);
            }
            false
        };
        // Serializing preparation and open prevents a late seed/manifest/asset write
        // from changing a Live room. Stop stays independent and can cancel an upload.
        let _upload = if owner { Some(self.uploads.lock().await) } else { None };
        let phase = self.phase()?.ok_or_else(|| fail("Missing phase"))?;
        if phase == RoomPhase::Ended {
            return Response::error("Sharing ended", 410);
        }
        if route == "open" && req.method() == Method::Post {
            if phase == RoomPhase::Live {
                return Response::from_json(&self.info(true)?);
            }
            if self.text("seed_hash")?.is_none() {
                return Response::error("Seed not ready", 409);
            }
            let Some(manifest) = self.text("resources")? else {
                return Response::error("App not ready", 409);
            };
            let manifest: crate::resource_types::Manifest = serde_json::from_str(&manifest)?;
            if self.text("app")?.as_ref() != Some(&manifest.app) {
                return Response::error("Different app", 409);
            }
            let bucket = self.env.bucket("SEEDS")?;
            let seed_size: u64 =
                self.text("seed_size")?.unwrap_or_default().parse().map_err(|_| fail("Invalid seed size"))?;
            if bucket.head(format!("{room}/seed")).await?.is_none_or(|o| o.size() != seed_size) {
                return Response::error("Seed not ready", 409);
            }
            for item in manifest.resources {
                if bucket.head(format!("{room}/assets/{}", item.hash)).await?.is_none_or(|o| o.size() != item.size) {
                    return Response::error("App not ready", 409);
                }
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            self.set_phase(RoomPhase::Live)?;
            self.active().await?;
            record(&self.env, Metric::RoomOpened);
            return Response::from_json(&self.info(true)?);
        }
        if owner_write && phase != RoomPhase::Uploading {
            return Response::error("Room is live", 409);
        }
        if !owner && phase != RoomPhase::Live {
            return Response::error("Publication pending", 409);
        }
        if matches!(route, "resources" | "asset" | "blobs" | "blob") {
            return self.resource_api(&mut req, room, route).await;
        }
        if route == "compact" && req.method() == Method::Post {
            return self.compact(&mut req, room).await;
        }
        if route == "seed" {
            let bucket = self.env.bucket("SEEDS")?;
            let key = format!("{room}/seed");
            if req.method() == Method::Put {
                let digest = req
                    .headers()
                    .get("X-Sharing-SHA256")?
                    .or_else(|| req.url().ok()?.query_pairs().find(|(k, _)| k == "sha256").map(|(_, v)| v.into_owned()))
                    .unwrap_or_default();
                let app = req.headers().get("X-Sharing-App")?.unwrap_or_default();
                let seed: Vec<(String, u32)> =
                    match req.headers().get("X-Sharing-Version")?.and_then(|v| serde_json::from_str(&v).ok()) {
                        Some(v) => v,
                        None => return Response::error("Seed version required", 400),
                    };
                let heads = version(&seed)?;
                if !crate::resource_types::hash_id(&digest)
                    || !crate::resource_types::hash_id(&app)
                    || heads.len() != seed.len()
                {
                    return Response::error("Invalid seed metadata", 400);
                }
                let body = bounded_body(&mut req, SEED_BYTES).await?;
                if body.len() < 100 || hex(&Sha256::digest(&body)) != digest {
                    return Response::error("Seed checksum mismatch", 422);
                }
                if self.phase()? == Some(RoomPhase::Ended) {
                    return Response::error("Sharing ended", 410);
                }
                let size = body.len();
                bucket.put(key.clone(), body).sha256(decode_hex(&digest)?).execute().await?;
                if self.phase()? == Some(RoomPhase::Ended) {
                    bucket.delete(key).await?;
                    return Response::error("Sharing ended", 410);
                }
                self.put("seed_hash", &digest)?;
                self.put("seed_size", &size.to_string())?;
                self.put("app", &app)?;
                self.put("root", &serde_json::to_string(&wire_version(&heads))?)?;
                self.put("heads", &serde_json::to_string(&wire_version(&heads))?)?;
                return Response::ok("ready");
            }
            if req.method() == Method::Get {
                if let Some(object) = bucket.get(key).execute().await? {
                    return Response::from_body(
                        object.body().ok_or_else(|| fail("Seed has no body"))?.response_body()?,
                    );
                }
                return Response::error("Seed missing", 404);
            }
            return Response::error("Method refused", 405);
        }
        if route == "sync" && req.headers().get("Upgrade")?.as_deref() == Some("websocket") {
            if self.state.get_websockets().len() >= hitslop_sharing_protocol::limits::ROOM_CONNECTIONS {
                return Response::error("Room connection limit", 429);
            }
            let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_else(|| "unknown".into());
            let mut count = 0;
            for socket in self.state.get_websockets() {
                if socket.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.ip == ip) {
                    count += 1;
                }
            }
            if count >= ROOM_IP_CONNECTIONS {
                return Response::error("Room IP connection limit", 429);
            }
            let pair = WebSocketPair::new()?;
            self.state.accept_web_socket(&pair.server);
            pair.server.serialize_attachment(SocketState {
                joined: false,
                ip,
                window_start: now(),
                bytes: 0,
                pushes: 0,
            })?;
            record(&self.env, Metric::GuestJoin);
            return Response::from_websocket(pair.client);
        }
        Response::error("Not found", 404)
    }
    async fn alarm(&self) -> Result<Response> {
        if self.phase()? == Some(RoomPhase::Ended) && self.text("owner_uid")?.is_none() {
            let ended = self.text("ended_at")?.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            if now() >= ended + ENDED_TOMBSTONE_DAYS * DAY_MS {
                self.state.storage().delete_all().await?;
                initialize(&self.sql);
            } else {
                self.state
                    .storage()
                    .set_alarm(std::time::Duration::from_millis(ended + ENDED_TOMBSTONE_DAYS * DAY_MS - now()))
                    .await?;
            }
            return Response::ok("ended");
        }
        if matches!(self.phase()?, Some(RoomPhase::Uploading | RoomPhase::Ended)) {
            let room = self.text("room")?.ok_or_else(|| fail("Missing room"))?;
            return self.stop_room(&room, Metric::RoomEndedAlarm).await;
        }
        if self.phase()? == Some(RoomPhase::Live) {
            self.sweep_archives().await?;
            let last = self.text("last_active")?.and_then(|s| s.parse::<u64>().ok()).unwrap_or_else(now);
            if self.state.get_websockets().is_empty() && now() >= last + ROOM_IDLE_DAYS * DAY_MS {
                return self
                    .stop_room(&self.text("room")?.ok_or_else(|| fail("Missing room"))?, Metric::RoomEndedIdle)
                    .await;
            }
            self.state
                .storage()
                .set_alarm(std::time::Duration::from_millis(if self.state.get_websockets().is_empty() {
                    (last + ROOM_IDLE_DAYS * DAY_MS).saturating_sub(now()).clamp(1, DAY_MS)
                } else {
                    DAY_MS
                }))
                .await?;
        }
        Response::ok("live")
    }
    async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
        if let Err(error) = self.message(&ws, message).await {
            match error {
                SocketFailure::Refused(code, message) => {
                    let _ = self.send(&ws, Frame::Error { code, message });
                    let _ = ws.close(Some(1008), Some("Protocol refused"));
                }
                SocketFailure::Transient(error) => {
                    let _ = error;
                    let _ = ws.close(Some(1011), None::<String>);
                }
            }
        }
        Ok(())
    }
    async fn websocket_close(&self, ws: WebSocket, code: usize, _: String, _: bool) -> Result<()> {
        let _ = ws.close(Some(if code == 1005 { 1000 } else { code as u16 }), None::<String>);
        Ok(())
    }
    async fn websocket_error(&self, ws: WebSocket, _: worker::Error) -> Result<()> {
        let _ = ws.close(Some(1011), None::<String>);
        Ok(())
    }
}

pub(crate) async fn bounded_body(req: &mut Request, limit: usize) -> Result<Vec<u8>> {
    use futures_util::StreamExt;
    let mut stream = req.stream()?;
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(fail("Request exceeds its byte limit"));
        }
        body.extend(chunk);
    }
    Ok(body)
}

pub(crate) fn random_hex(size: usize) -> Result<String> {
    let mut bytes = vec![0; size];
    getrandom::getrandom(&mut bytes).map_err(|_| fail("Random unavailable"))?;
    Ok(hex(&bytes))
}
pub(crate) fn identity_response(error: crate::auth::IdentityError) -> Result<Response> {
    Response::error(
        "Sign in required or identity service unavailable",
        match error {
            crate::auth::IdentityError::Denied => 401,
            crate::auth::IdentityError::Unavailable => 503,
        },
    )
}
