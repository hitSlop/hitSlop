//! One SQLite Durable Object per room; immutable payloads in private R2.
use crate::frame::{Ack, Frame, PROTOCOL, Permission, Record, Span, Version};
use hitslop_sharing_protocol::{CONTROL_BYTES, SEED_BYTES, SOCKET_PROTOCOL, http::RoomInit as Init};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
use worker::*;
#[path = "resources.rs"]
mod resources;

const CHUNK: usize = 1_800_000;
const BACKFILL: usize = 1 << 20;
const ROOM_LIMIT: usize = 16 << 20;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hash(key: &str) -> String {
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
    let response = match serve(req, env).await {
        Ok(r) => r,
        Err(_) => Response::error("Request failed", 500)?,
    };
    if response.status_code() < 400 {
        return Ok(response);
    }
    let status = response.status_code();
    let headers = response.headers().clone();
    let code = match status {
        400 | 413 | 422 => "invalid_request",
        403 => "denied",
        404 => "not_found",
        409 => "conflict",
        410 => "sharing_ended",
        429 => "limited",
        _ => "unavailable",
    };
    let mut response = Response::from_json(&hitslop_sharing_protocol::http::ApiFailure {
        code: code.into(),
        message: "The sharing service refused this request".into(),
    })?
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
            "Authorization, Content-Type, X-Admin-Key, X-Write-Key, X-Preview-Create-Key",
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
        return Response::ok("hitslop-sharing-preview");
    }
    let parts: Vec<_> = path.split('/').collect();
    if !(parts.len() == 4 || parts.len() == 5)
        || parts[1] != "rooms"
        || parts[2].is_empty()
        || parts[2].len() > 80
        || !parts[2].bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Response::error("Not found", 404);
    }
    if parts[3] == "init" {
        let key = env.secret("PREVIEW_CREATE_KEY")?.to_string();
        if req.headers().get("X-Preview-Create-Key")?.as_deref() != Some(key.as_str()) {
            return Response::error("Creation unavailable", 403);
        }

        let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_else(|| "local".into());
        if !env.rate_limiter("CREATE_LIMIT")?.limit(ip).await?.success {
            return Response::error("Rate limited", 429);
        }
    }
    let mut response = env.durable_object("ROOMS")?.get_by_name(parts[2])?.fetch_with_request(req).await?;
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

#[derive(Deserialize, Serialize, Clone)]
struct SocketState {
    joined: bool,
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
    fn authorized(&self, req: &Request, admin: bool) -> Result<bool> {
        let key = req.headers().get("Authorization")?.unwrap_or_default();
        let Some(key) = key.strip_prefix("Bearer ") else { return Ok(false) };
        let h = hash(key);
        Ok(self.text("admin")?.as_ref() == Some(&h) || (!admin && self.text("write")?.as_ref() == Some(&h)))
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
    async fn push(&self, ws: &WebSocket, id: u64, records: Vec<Record>) -> Result<()> {
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
        let stored =
            self.sql.exec("SELECT coalesce(sum(size),0) AS n FROM records", None)?.one::<NumberRow>()?.n as usize;
        if stored + added.iter().map(|r| r.bytes.len()).sum::<usize>() > ROOM_LIMIT {
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
        if self.text("ended")?.is_some() {
            return Err(fail("Sharing ended"));
        }
        self.send(ws, Frame::Ack { id, status: Ack::Stored })?;
        for other in self.state.get_websockets() {
            // Echo is harmless and keeps broadcast routing independent of JS object identity.
            if other.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.joined) {
                for record in &added {
                    self.send(&other, Frame::Records(vec![record.clone()]))?;
                }
            }
        }
        Ok(())
    }
    async fn message(&self, ws: &WebSocket, msg: WebSocketIncomingMessage) -> Result<()> {
        if self.text("ended")?.is_some() {
            return Err(fail("Sharing ended"));
        }
        let WebSocketIncomingMessage::Binary(bytes) = msg else {
            return Err(fail("Binary frame required"));
        };
        match crate::envelope::decode(&bytes).map_err(fail)? {
            Frame::Hello { protocol, app_digest, version } => self.hello(ws, protocol, &app_digest, version),
            Frame::Push { id, records } => self.push(ws, id, records).await,
            _ => Err(fail("Client frame required")),
        }
    }
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
        sql.exec("CREATE TABLE IF NOT EXISTS resource_sessions(token TEXT PRIMARY KEY,host TEXT NOT NULL,app TEXT NOT NULL,expires REAL NOT NULL); CREATE TABLE IF NOT EXISTS resource_blobs(id TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS records(seq INTEGER PRIMARY KEY AUTOINCREMENT,spans TEXT NOT NULL,size INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS chunks(seq INTEGER NOT NULL,part INTEGER NOT NULL,bytes BLOB NOT NULL,PRIMARY KEY(seq,part))",None).expect("create sharing tables");
        Self { state, env, sql }
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
        let room = parts[2];
        let route = parts[3];
        if route == "init" && req.method() == Method::Post {
            if self.text("ended")?.is_some() {
                return Response::error("Sharing ended", 410);
            }
            let admin = req.headers().get("X-Admin-Key")?.unwrap_or_default();
            let write = req.headers().get("X-Write-Key")?.unwrap_or_default();
            if admin.len() < 32 || write.len() < 32 {
                return Response::error("Room keys required", 400);
            }
            let body = bounded_body(&mut req, CONTROL_BYTES).await?;
            let init: Init = serde_json::from_slice(&body)?;
            if self.text("ended")?.is_some() {
                return Response::error("Sharing ended", 410);
            }
            let heads = version(&init.seed)?;
            let encoded = serde_json::to_string(&init)?;
            if let Some(existing) = self.text("init")? {
                return if existing == encoded
                    && self.text("admin")?.as_deref() == Some(&hash(&admin))
                    && self.text("write")?.as_deref() == Some(&hash(&write))
                {
                    Response::ok("existing")
                } else {
                    Response::error("Conflict", 409)
                };
            }
            if !crate::resource_types::hash_id(&init.app)
                || !crate::resource_types::hash_id(&init.seed_hash)
                || heads.len() != init.seed.len()
            {
                return Response::error("Invalid metadata", 400);
            }
            let sql = self.sql.clone();
            self.transaction(move || {
                for (key, value) in [
                    ("init", encoded.clone()),
                    ("app", init.app.clone()),
                    ("seed_hash", init.seed_hash.clone()),
                    ("heads", serde_json::to_string(&wire_version(&heads))?),
                    ("admin", hash(&admin)),
                    ("write", hash(&write)),
                ] {
                    sql.exec("INSERT INTO meta VALUES(?,?)", vec![key.into(), value.into()])?;
                }
                Ok(())
            })?;
            self.state.storage().sync().await?;
            if self.text("ended")?.is_some() {
                return Response::error("Sharing ended", 410);
            }
            return Response::ok("created");
        }
        if self.text("app")?.is_none() {
            return Response::error("Unknown room", 404);
        }
        if !self.authorized(&req, route == "stop" || (route == "seed" && req.method() == Method::Put))? {
            return Response::error("Denied", 403);
        }
        if self.text("ended")?.is_some() && route != "stop" {
            return Response::error("Sharing ended", 410);
        }
        if matches!(route, "resources" | "asset" | "blobs" | "blob") {
            return self.resource_api(&mut req, room, route).await;
        }
        if route == "seed" {
            if !matches!(req.method(), Method::Get | Method::Put) {
                return Response::error("Method refused", 405);
            }
            let bucket = self.env.bucket("SEEDS")?;
            let key = format!("{room}/seed");
            if req.method() == Method::Put {
                let size =
                    req.headers().get("Content-Length")?.and_then(|s| s.parse::<usize>().ok()).unwrap_or(usize::MAX);
                if size > SEED_BYTES {
                    return Response::error("Seed too large", 413);
                }
                if self.text("ready")?.is_some() {
                    return Response::ok("existing");
                }
                let digest = decode_hex(&self.text("seed_hash")?.ok_or_else(|| fail("Seed hash absent"))?)?;
                let body = bounded_body(&mut req, SEED_BYTES).await?;
                if body.len() != size {
                    return Response::error("Seed length mismatch", 422);
                }
                bucket.put(key, body).sha256(digest).execute().await?;
                if self.text("ended")?.is_some() {
                    bucket.delete(format!("{room}/seed")).await?;
                    return Response::error("Sharing ended", 410);
                }
                self.put("ready", "1")?;
                self.state.storage().sync().await?;
                return Response::ok("ready");
            }
            if let Some(object) = bucket.get(key).execute().await? {
                return Response::from_body(object.body().ok_or_else(|| fail("Seed has no body"))?.response_body()?);
            }
            return Response::error("Seed missing", 404);
        }
        if route == "stop" && req.method() == Method::Delete {
            self.put("ended", "1")?;
            self.state.storage().sync().await?;
            for ws in self.state.get_websockets() {
                let _ = self.send(&ws, Frame::Error { code: 1, message: "Sharing ended".into() });
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
            self.sql.exec("DELETE FROM resource_sessions; DELETE FROM resource_blobs", None)?;
            self.sql.exec("DELETE FROM records; DELETE FROM chunks", None)?;
            return Response::ok("ended");
        }
        if route == "stats" {
            let n = self.sql.exec("SELECT count(*) AS n FROM records", None)?.one::<NumberRow>()?.n;
            let bytes = self.sql.exec("SELECT coalesce(sum(size),0) AS n FROM records", None)?.one::<NumberRow>()?.n;
            return Response::from_json(
                &serde_json::json!({"records":n,"bytes":bytes,"heads":wire_version(&self.heads()?),"sockets":self.state.get_websockets().len()}),
            );
        }
        if route == "sync" && req.headers().get("Upgrade")?.as_deref() == Some("websocket") {
            if self.text("ready")?.is_none() {
                return Response::error("Seed not ready", 409);
            }
            let pair = WebSocketPair::new()?;
            self.state.accept_web_socket(&pair.server);
            pair.server.serialize_attachment(SocketState { joined: false })?;
            return Response::from_websocket(pair.client);
        }
        Response::error("Not found", 404)
    }
    async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
        if let Err(error) = self.message(&ws, message).await {
            let _ = self.send(&ws, Frame::Error { code: 1, message: error.to_string() });
            let _ = ws.close(Some(1008), Some("Protocol refused"));
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

async fn bounded_body(req: &mut Request, limit: usize) -> Result<Vec<u8>> {
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
