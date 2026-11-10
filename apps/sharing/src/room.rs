//! Room lifecycle and opaque relay records; private payloads in R2.
use crate::frame::{Ack, Frame, PROTOCOL, Permission, Record, Span, Version};
use crate::metrics::{Metric, record};
use pop_sharing_protocol::{
    CONTROL_BYTES, SEED_BYTES, SOCKET_PROTOCOL, hex,
    http::{RoomInfo, RoomPhase},
    routes::{self, Op, Scope},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
use worker::*;
#[path = "access.rs"]
mod access;
#[path = "blobs.rs"]
mod blobs;
#[path = "publication.rs"]
pub(crate) mod publication;

// A SQLite snapshot must contain the complete fixed-size database header.
const SQLITE_HEADER_BYTES: usize = 100;
const CHUNK: usize = 1_800_000;
const BACKFILL: usize = 1 << 20;
use pop_sharing_protocol::limits::*;
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

/// R2 keys are the plain document UUID and a suffix; isolation from other deployments
/// comes from separate bucket names at deploy time.
pub(crate) fn object_key(document: &str, suffix: &str) -> String {
    format!("{document}/{suffix}")
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
        Err(error) => {
            // Never log payloads, bearer tokens, or arbitrary exception messages.
            let kind = match error {
                worker::Error::SerdeJsonError(_) => "stored_json",
                worker::Error::BindingError(_) => "binding",
                worker::Error::Io(_) => "io",
                _ => "internal",
            };
            console_error!("Sharing request failed ({})", kind);
            Response::error("Request failed", 500)?
        }
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
    let mut failure = pop_sharing_protocol::http::ApiFailure::refused(status);
    if status < 500 {
        // Refusal bodies originate in our handlers and contain fixed public messages.
        // Internal failures always keep the generic message above.
        let message = response.text().await?;
        if message.len() <= 1024 {
            if let Ok(public) = serde_json::from_str::<pop_sharing_protocol::http::ApiFailure>(&message) {
                failure = public;
            } else if !message.is_empty() {
                failure.message = message;
            }
        }
    }
    let mut response = Response::from_json(&failure)?.with_status(status).with_headers(headers);
    response.headers_mut().set("Content-Type", "application/json")?;
    Ok(response)
}
async fn serve(req: Request, env: Env) -> Result<Response> {
    let path = req.path();
    // One table decides what exists: an unlisted method, path or extra segment is a 404
    // before any limiter, identity check or Durable Object is reached.
    let Some(matched) = routes::find(req.method().as_ref(), &path) else {
        return Response::error("Not found", 404);
    };
    let websocket = req.headers().get("Upgrade")?.as_deref() == Some("websocket");
    let protocol = matched.route.socket_protocol();
    if websocket && (protocol.is_none() || req.headers().get("Sec-WebSocket-Protocol")?.as_deref() != protocol) {
        return requires_update();
    }
    if matched.route.op == Op::Health {
        return Response::ok("pop-sharing");
    }
    if let Some(binding) = matched.route.limit.binding() {
        // Cloudflare sets this header. Missing IPs share a conservative fallback budget.
        let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_else(|| "unknown".into());
        if !env.rate_limiter(binding)?.limit(ip).await?.success {
            return Response::error("Limited", 429);
        }
    }
    let mut response = match (matched.route.scope, matched.document) {
        (Scope::Account, _) => {
            let claims = match crate::auth::identity(&req, &env).await {
                Ok(c) => c,
                Err(e) => return identity_response(e),
            };
            env.durable_object("ACCOUNTS")?.get_by_name(&hash(&claims.sub))?.fetch_with_request(req).await?
        }
        (Scope::Document, Some(document)) => {
            env.durable_object("DOCUMENTS")?.get_by_name(document)?.fetch_with_request(req).await?
        }
        _ => return Response::error("Not found", 404),
    };
    if response.status_code() == 101 {
        let headers = response.headers().clone();
        response = response.with_headers(headers);
        response.headers_mut().set("Sec-WebSocket-Protocol", protocol.unwrap_or(SOCKET_PROTOCOL))?;
    }
    Ok(response)
}

fn requires_update() -> Result<Response> {
    Ok(Response::from_json(&pop_sharing_protocol::http::ApiFailure {
        code: "requires_update".into(),
        message: "Update pop to use this document service".into(),
    })?
    .with_status(426))
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
    owner: bool,
    #[serde(default)]
    generation: u64,
    #[serde(default)]
    uid: String,
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
pub struct CloudDocument {
    state: State,
    env: Env,
    sql: SqlStorage,
    uploads: futures_util::lock::Mutex<()>,
    /// Serializes joins, leaves and registry cleanup; lifecycle changes never wait for it.
    membership: futures_util::lock::Mutex<()>,
}

impl CloudDocument {
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
    fn info(&self) -> Result<RoomInfo> {
        Ok(RoomInfo {
            document: self.text("document")?.ok_or_else(|| fail("Missing document"))?,
            phase: self.phase()?.ok_or_else(|| fail("Missing phase"))?,
            seed_hash: self.text("seed_hash")?,
            seed_size: self.text("seed_size")?.map(|s| s.parse().map_err(|_| fail("Invalid seed size"))).transpose()?,
            app: self.text("app")?,
            compact: self.compact_due()?,
        })
    }
    fn update_usage(&self) -> Result<(usize, usize)> {
        #[derive(Deserialize)]
        struct Usage {
            bytes: usize,
            rows: usize,
        }
        let usage = self
            .sql
            .exec("SELECT coalesce(sum(size),0) AS bytes,count(*) AS rows FROM records WHERE seq>0", None)?
            .one::<Usage>()?;
        Ok((usage.bytes, usage.rows))
    }
    fn compact_due(&self) -> Result<bool> {
        let (bytes, rows) = self.update_usage()?;
        Ok(bytes >= COMPACT_AFTER_BYTES || rows >= COMPACT_AFTER_RECORDS)
    }
    async fn compact(&self, req: &mut Request) -> Result<Response> {
        // Serialize competing archives; Push and Stop may still progress across the R2 await.
        let _guard = self.uploads.lock().await;
        let body = match crate::input::bytes(req, BASE_BYTES).await {
            Ok(bytes) => bytes,
            Err(error) => return error.response(),
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
        // Every live document stored its root with its seed; a missing or unreadable root is
        // a fault in this document, never something a client may supply.
        let root = parse(self.text("root")?).ok_or_else(|| fail("Missing stored root version"))?;
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
        if !self.authorized_guest(req).await? {
            return Response::error("Access revoked", 403);
        }
        let mut covered = vec![];
        for row in self.sql.exec("SELECT seq,spans,size FROM records ORDER BY seq", None)?.next::<RecordRow>() {
            let row = row?;
            let spans: Vec<(String, u32, u32)> = serde_json::from_str(&row.spans)?;
            if row.seq == 0
                || spans.iter().all(|(peer, _, end)| {
                    u64::from_str_radix(peer, 16)
                        .ok()
                        .is_some_and(|peer| *end <= frontier.get(&peer).copied().unwrap_or(0))
                })
            {
                covered.push(row.seq);
            }
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
        Response::empty().map(|r| r.with_status(204))
    }
    async fn authorize_creator(&self, req: &Request) -> Result<crate::auth::CreatorAuthorization> {
        let identity = crate::auth::identity(req, &self.env).await.map(|claims| claims.sub);
        let owner_uid = self.text("owner_uid")?.unwrap_or_default();
        Ok(crate::auth::creator_authorization(identity, &owner_uid))
    }
    async fn authorized_guest(&self, req: &Request) -> Result<bool> {
        Ok(self.identity_role(req).await?.is_some())
    }
    async fn stop_room(&self, room: &str, reason: Metric) -> Result<Response> {
        if self.phase()? != Some(RoomPhase::Ended) {
            self.set_phase(RoomPhase::Ended)?;
            self.put("ended_at", &now().to_string())?;
            self.queue_registry_releases()?;
            self.sql.exec("DELETE FROM members; DELETE FROM meta WHERE key='capability'", None)?;
            record(&self.env, reason);
        }
        // Identity/deletion tombstones persist: the UUID can never be reclaimed by
        // another account. Retry cleanup and discovery publication independently.
        if self.text("cleanup_done")?.is_none() {
            self.schedule_cleanup(5_000).await?;
            self.state.storage().sync().await?;
            self.end_connections(crate::frame::ERROR_ENDED, "Document deleted", 1008);
            let bucket = self.env.bucket("SEEDS")?;
            for prefix in [object_key(room, "")] {
                loop {
                    let objects = bucket.list().prefix(prefix.clone()).execute().await?.objects();
                    if objects.is_empty() {
                        break;
                    }
                    for item in objects {
                        bucket.delete(item.key()).await?;
                    }
                }
            }
            self.sql.exec("DELETE FROM resource_blobs; DELETE FROM records; DELETE FROM chunks; DELETE FROM meta WHERE key IN ('seed_hash','seed_size','root','heads','base_version')",None)?;
            self.state.storage().sync().await?;
            if let Some(uid) = self.text("owner_uid")?
                && !self.release_registration(&uid, None).await?
            {
                return Err(fail("Account release failed"));
            }
            self.put("cleanup_done", "1")?;
            self.state.storage().sync().await?;
        }
        self.flush_registry().await?;
        Response::ok("deleted")
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
    /// The immutable seed uploaded before the room went live.
    async fn read_seed(&self, req: &Request, room: &str) -> Result<Response> {
        let object = self.env.bucket("SEEDS")?.get(object_key(room, "seed")).execute().await?;
        if self.phase()? == Some(RoomPhase::Ended) || !self.authorized_guest(req).await? {
            return Response::error("Sharing ended", 410);
        }
        match object {
            Some(object) => {
                Response::from_body(object.body().ok_or_else(|| fail("Missing seed body"))?.response_body()?)
            }
            None => Response::error("Seed missing", 404),
        }
    }
    pub(super) fn end_connections(&self, code: u8, message: &str, close: u16) {
        for ws in self.state.get_websockets() {
            let _ = ws.serialize_attachment(SocketState::default());
            let _ = self.send(&ws, Frame::Error { code, message: message.into() });
            let _ = ws.close(Some(close), Some(message));
        }
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
        // Lost acknowledgements are retried even when later writes filled the room.
        // The socket budget above still charges duplicates to prevent abuse.
        if added.is_empty() {
            return self.send(ws, Frame::Ack { id, status: Ack::Stored });
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
        let access = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        if !access.owner
            && (access.generation != self.generation()?
                || access.uid.is_empty()
                || (self
                    .sql
                    .exec(
                        "SELECT count(*) AS n FROM members WHERE uid=? AND generation=?",
                        vec![access.uid.into(), (self.generation()? as i64).into()],
                    )?
                    .one::<NumberRow>()?
                    .n
                    == 0))
        {
            let _ = ws.close(Some(1008), Some("Access revoked"));
            return Ok(());
        }
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
    sql.exec("CREATE TABLE IF NOT EXISTS pending_registry(uid TEXT PRIMARY KEY,generation INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS members(uid TEXT PRIMARY KEY,generation INTEGER NOT NULL)",None).expect("document access tables");
    sql.exec("CREATE TABLE IF NOT EXISTS garbage(key TEXT PRIMARY KEY); CREATE TABLE IF NOT EXISTS resource_blobs(id TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS records(seq INTEGER PRIMARY KEY AUTOINCREMENT,spans TEXT NOT NULL,size INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS chunks(seq INTEGER NOT NULL,part INTEGER NOT NULL,bytes BLOB NOT NULL,PRIMARY KEY(seq,part))",None).expect("create sharing tables");
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
impl DurableObject for CloudDocument {
    fn new(state: State, env: Env) -> Self {
        state.set_websocket_auto_response(
            &worker::worker_sys::WebSocketRequestResponsePair::new("ping", "pong").expect("valid ping/pong"),
        );
        let sql = state.storage().sql();
        initialize(&sql);
        Self {
            state,
            env,
            sql,
            uploads: futures_util::lock::Mutex::new(()),
            membership: futures_util::lock::Mutex::new(()),
        }
    }
    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let path = req.path();
        if req.method() == Method::Post
            && self.text("document")?.is_some_and(|document| path == format!("/internal/documents/{document}/library"))
        {
            #[derive(Deserialize)]
            struct LibraryIdentity {
                uid: String,
            }
            let input: LibraryIdentity =
                crate::input::body(&mut req, CONTROL_BYTES).await.map_err(|_| fail("Invalid library identity"))?;
            if self.phase()? != Some(RoomPhase::Live) {
                return Response::error("Document not live", 409);
            }
            let Some(role) = self.role_for_uid(&input.uid)? else {
                return Response::error("Denied", 403);
            };
            let revision = self.text("library_revision")?.and_then(|s| s.parse::<u64>().ok()).unwrap_or(1);
            if self.text("library_revision")?.is_none() {
                self.put("library_revision", "1")?;
            }
            return Response::from_json(&publication::Description {
                entry: pop_sharing_protocol::library::AccountEntry {
                    document: self.text("document")?.ok_or_else(|| fail("Missing document"))?,
                    role,
                    title: self.text("title")?.unwrap_or_else(|| "Untitled".into()),
                    template_id: self.text("template_id")?.unwrap_or_default(),
                    sharing: self.text("capability")?.is_some(),
                    revision,
                },
                generation: if role == pop_sharing_protocol::document::DocumentRole::Owner {
                    None
                } else {
                    Some(self.generation()?)
                },
            });
        }
        let parts: Vec<_> = path.split('/').collect();
        if path.starts_with("/internal/documents/")
            && parts.len() == 5
            && parts[4] == "init"
            && req.method() == Method::Post
        {
            let input: crate::account::DocumentInit = match crate::input::body(&mut req, CONTROL_BYTES).await {
                Ok(input) => input,
                Err(error) => return error.response(),
            };
            if !pop_sharing_protocol::valid_document_id(&input.document) || input.owner_uid.is_empty() {
                return Response::error("Invalid metadata", 400);
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            let creating = self.phase()?.is_none();
            if creating {
                self.put("owner_uid", &input.owner_uid)?;
                self.put("document", &input.document)?;
                self.put("access_generation", "1")?;
                self.set_phase(RoomPhase::Uploading)?;
                record(&self.env, Metric::DocumentCreated);
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            if self.text("owner_uid")?.as_ref() != Some(&input.owner_uid)
                || self.text("document")?.as_ref() != Some(&input.document)
            {
                return Response::error("Denied", 403);
            }
            self.state.storage().sync().await?;
            return Response::empty().map(|r| r.with_status(204));
        }
        let Some(matched) = routes::find(req.method().as_ref(), &path) else {
            return Response::error("Not found", 404);
        };
        let (Some(room), op) = (matched.document, matched.route.op) else {
            return Response::error("Not found", 404);
        };
        let Some(phase) = self.phase()? else {
            return Response::error("Unknown room", 404);
        };
        if phase == RoomPhase::Ended && self.text("owner_uid")?.is_none() {
            return Response::error("Sharing ended", 410);
        }
        if op == Op::DeleteDocument {
            match self.authorize_creator(&req).await? {
                crate::auth::CreatorAuthorization::Allowed => {}
                decision => {
                    return creator_response(decision)?.ok_or_else(|| fail("Unexpected creator authorization"));
                }
            }
            return self.stop_room(room, Metric::DocumentDeleted).await;
        }
        if phase == RoomPhase::Ended {
            return Response::error("Document deleted", 410);
        }
        if matches!(
            op,
            Op::PatchDocument
                | Op::GetSharing
                | Op::EnableSharing
                | Op::DisableSharing
                | Op::Join
                | Op::Leave
                | Op::GetMetadata
        ) {
            return self.access_route(&mut req, op).await;
        }
        if op == Op::GetDocument {
            let guest = self.authorized_guest(&req).await?;
            let owner = if self
                .identity_role(&req)
                .await?
                .is_some_and(|(_, role)| role == pop_sharing_protocol::document::DocumentRole::Owner)
            {
                true
            } else if guest {
                false
            } else {
                match self.authorize_creator(&req).await? {
                    crate::auth::CreatorAuthorization::Allowed => true,
                    decision => {
                        return creator_response(decision)?.ok_or_else(|| fail("Unexpected creator authorization"));
                    }
                }
            };
            if !guest && !owner {
                return Response::error("Denied", 403);
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            if !owner && self.phase()? != Some(RoomPhase::Live) {
                return Response::error("Publication pending", 409);
            }
            return Response::from_json(&self.info()?);
        }
        let owner_write = matches!(op, Op::Open | Op::PutSeed);
        let owner = if owner_write
            || (matches!(op, Op::PutBlob | Op::ListBlobs) && phase == RoomPhase::Uploading)
            || (matches!(op, Op::GetSeed | Op::ListBlobs | Op::GetBlob) && !self.authorized_guest(&req).await?)
        {
            match self.authorize_creator(&req).await? {
                crate::auth::CreatorAuthorization::Allowed => {}
                decision => {
                    return creator_response(decision)?.ok_or_else(|| fail("Unexpected creator authorization"));
                }
            }
            true
        } else {
            if !self.authorized_guest(&req).await? {
                return Response::error("Denied", 403);
            }
            false
        };
        // Serializing preparation and open prevents a late seed write
        // from changing a Live room. Stop stays independent and can cancel an upload.
        let _upload = if owner_write { Some(self.uploads.lock().await) } else { None };
        let phase = self.phase()?.ok_or_else(|| fail("Missing phase"))?;
        if phase == RoomPhase::Ended {
            return Response::error("Sharing ended", 410);
        }
        if op == Op::Open {
            let input: pop_sharing_protocol::http::OpenRoom = match crate::input::body(&mut req, CONTROL_BYTES).await {
                Ok(input) => input,
                Err(error) => return error.response(),
            };
            if !crate::resource_types::hash_id(&input.seed_sha256) {
                return Response::error("Invalid seed hash", 400);
            }
            // Reading the body yielded to Stop, which also removes the seed.
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            // The one fence: only the stored seed can go live, so an older upload attempt
            // cannot publish a seed uploaded after it. Attachments are not checked here;
            // the creator uploads every one the seed references before Open.
            match self.text("seed_hash")? {
                None => return Response::error("Seed not ready", 409),
                Some(stored) if stored != input.seed_sha256 => return Response::error("Different seed", 409),
                Some(_) => {}
            }
            if phase == RoomPhase::Live {
                return Response::from_json(&self.info()?);
            }
            let bucket = self.env.bucket("SEEDS")?;
            let seed_size: u64 =
                self.text("seed_size")?.unwrap_or_default().parse().map_err(|_| fail("Invalid seed size"))?;
            if bucket.head(object_key(room, "seed")).await?.is_none_or(|o| o.size() != seed_size) {
                return Response::error("Seed not ready", 409);
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            self.set_phase(RoomPhase::Live)?;
            self.publish_library().await?;
            record(&self.env, Metric::DocumentReady);
            return Response::from_json(&self.info()?);
        }
        if owner_write && phase != RoomPhase::Uploading {
            return Response::error("Room is live", 409);
        }
        if !owner && phase != RoomPhase::Live {
            return Response::error("Publication pending", 409);
        }
        if matches!(op, Op::ListBlobs | Op::GetBlob | Op::PutBlob) {
            return self.blob_api(&mut req, room, op, matched.hash).await;
        }
        if op == Op::Compact {
            return self.compact(&mut req).await;
        }
        if matches!(op, Op::GetSeed | Op::PutSeed) {
            let bucket = self.env.bucket("SEEDS")?;
            let key = object_key(room, "seed");
            if op == Op::PutSeed {
                let digest = req.headers().get("X-Sharing-SHA256")?.unwrap_or_default();
                let app = req.headers().get("X-Sharing-App")?.unwrap_or_default();
                let seed: Vec<(String, u32)> =
                    match req.headers().get("X-Sharing-Version")?.and_then(|v| serde_json::from_str(&v).ok()) {
                        Some(v) => v,
                        None => return Response::error("Seed version required", 400),
                    };
                let heads = match version(&seed) {
                    Ok(heads) => heads,
                    Err(_) => return Response::error("Invalid seed version", 400),
                };
                if !crate::resource_types::hash_id(&digest)
                    || !crate::resource_types::hash_id(&app)
                    || heads.len() != seed.len()
                {
                    return Response::error("Invalid seed metadata", 400);
                }
                let body = match crate::input::bytes(&mut req, SEED_BYTES).await {
                    Ok(bytes) => bytes,
                    Err(error) => return error.response(),
                };
                if body.len() < SQLITE_HEADER_BYTES || hex(&Sha256::digest(&body)) != digest {
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
            return self.read_seed(&req, room).await;
        }
        if op == Op::Sync && req.headers().get("Upgrade")?.as_deref() == Some("websocket") {
            if self.state.get_websockets().len() >= pop_sharing_protocol::limits::ROOM_CONNECTIONS {
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
            let claims = match crate::auth::identity(&req, &self.env).await {
                Ok(claims) => claims,
                Err(error) => return identity_response(error),
            };
            let uid = claims.sub;
            let owner = self.text("owner_uid")?.as_deref() == Some(uid.as_str());
            let generation = self.generation()?;
            let member = !uid.is_empty()
                && self
                    .sql
                    .exec(
                        "SELECT count(*) AS n FROM members WHERE uid=? AND generation=?",
                        vec![uid.clone().into(), (generation as i64).into()],
                    )?
                    .one::<NumberRow>()?
                    .n
                    > 0;
            if self.phase()? != Some(RoomPhase::Live) || (!owner && !member) {
                return Response::error("Access revoked", 403);
            }
            let pair = WebSocketPair::new()?;
            self.state.accept_web_socket(&pair.server);
            pair.server.serialize_attachment(SocketState {
                joined: false,
                owner,
                generation,
                uid,
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
        if self.phase()? == Some(RoomPhase::Ended) {
            let document = self.text("document")?.ok_or_else(|| fail("Missing document"))?;
            self.stop_room(&document, Metric::DocumentDeleted).await?;
        }
        {
            let _guard = self.uploads.lock().await;
            if let Some(document) = self.text("document")? {
                self.sweep_garbage(&document).await?;
            }
        }
        self.flush_registry().await?;
        self.flush_library().await?;
        let pending = self
            .sql
            .exec("SELECT ((SELECT count(*) FROM pending_registry)+(SELECT count(*) FROM garbage)) AS n", None)?
            .one::<NumberRow>()?
            .n
            > 0;
        if pending {
            self.schedule_cleanup(60_000).await?;
        }
        Response::ok("retained")
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
fn creator_response(decision: crate::auth::CreatorAuthorization) -> Result<Option<Response>> {
    match decision {
        crate::auth::CreatorAuthorization::Allowed => Ok(None),
        crate::auth::CreatorAuthorization::Denied => Ok(Some(Response::error("Denied", 403)?)),
        crate::auth::CreatorAuthorization::Unavailable => {
            Ok(Some(identity_response(crate::auth::IdentityError::Unavailable)?))
        }
    }
}
