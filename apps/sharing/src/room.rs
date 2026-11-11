//! Room lifecycle and opaque relay records; private payloads in R2.
use crate::metrics::{Metric, record};
use crate::routes::{self, Op, Scope};
use crate::{auth::VerifiedIdentity, refusal::Refusal};
use pop_contracts::hex;
use pop_contracts::relay::frame::{ErrorCode, Frame, Version};

use pop_sharing_protocol::document::DocumentRole;
use pop_sharing_protocol::http::RoomInfo;
use pop_sharing_protocol::http::RoomPhase;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
use worker::*;
// A SQLite snapshot must contain the complete fixed-size database header.
pub(crate) const SQLITE_HEADER_BYTES: usize = 100;
pub(crate) const CHUNK: usize = 1_800_000;
pub(crate) const BACKFILL: usize = 1 << 20;

pub(crate) fn now() -> u64 {
    js_sys::Date::now() as u64
}

#[derive(Debug)]
pub(crate) enum SocketFailure {
    Refused(ErrorCode, String),
    Transient(worker::Error),
}
impl From<worker::Error> for SocketFailure {
    fn from(e: worker::Error) -> Self {
        Self::Transient(e)
    }
}
pub(crate) type SocketResult = std::result::Result<(), SocketFailure>;
pub(crate) fn refused(code: ErrorCode, message: &str) -> SocketFailure {
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
pub(crate) fn fail(s: impl Into<String>) -> worker::Error {
    worker::Error::RustError(s.into())
}
pub(crate) fn version(value: &[(String, u32)]) -> Result<Version> {
    crate::policy::version(value).map_err(fail)
}
pub(crate) fn wire_version(vv: &Version) -> Vec<(String, u32)> {
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
            Refusal::new(500, "unavailable", "The sharing service refused this request").response()?
        }
    };
    let headers = response.headers().clone();
    let mut response = response.with_headers(headers);
    response.headers_mut().set("Cache-Control", "no-store")?;
    if response.status_code() == 429 {
        record(&env, Metric::Limited);
    }
    Ok(response)
}

pub(crate) async fn serve(req: Request, env: Env) -> Result<Response> {
    let mut req = req.clone_mut()?;
    req.headers_mut()?.delete(crate::auth::IDENTITY_HEADER)?;
    req.headers_mut()?.delete(routes::ROUTE_HEADER)?;
    let path = req.path();
    // One table decides what exists: an unlisted method, path or extra segment is a 404
    // before any limiter, identity check or Durable Object is reached.
    let Some(matched) = routes::find(req.method().as_ref(), &path) else {
        return Refusal::new(404, "not_found", "Not found").response();
    };
    if matched.route.internal {
        return Refusal::new(404, "not_found", "Not found").response();
    }
    matched.forward(req.headers_mut()?)?;
    let websocket = req.headers().get("Upgrade")?.as_deref() == Some("websocket");
    // Released 2.0.0 clients advertise a subprotocol. Refuse them before authentication.
    if websocket && req.headers().get("Sec-WebSocket-Protocol")?.is_some() {
        return Refusal::new(426, "requires_update", "Update pop to use this document service").response();
    }
    if matched.route.op == Op::Health {
        return Response::ok("pop-sharing");
    }
    if let Some(binding) = matched.route.limit.binding() {
        // Cloudflare sets this header. Missing IPs share a conservative fallback budget.
        let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_else(|| "unknown".into());
        if !env.rate_limiter(binding)?.limit(ip).await?.success {
            return Refusal::new(429, "limited", "Limited").response();
        }
    }
    let claims = match crate::auth::identity(&req, &env).await {
        Ok(claims) => claims,
        Err(error) => return identity_response(error),
    };
    let identity = VerifiedIdentity::from(claims);
    if !identity.named {
        return Refusal::new(401, "sign_in_required", "Sign in with Apple to use cloud documents").response();
    }
    identity.forward(req.headers_mut()?)?;
    req.headers_mut()?.delete("Authorization")?;
    match (matched.route.scope, matched.document) {
        (Scope::Account, _) => {
            env.durable_object("ACCOUNTS")?.get_by_name(&hash(&identity.uid))?.fetch_with_request(req).await
        }
        (Scope::Document, Some(document)) => {
            env.durable_object("DOCUMENTS")?.get_by_name(document)?.fetch_with_request(req).await
        }
        _ => Refusal::new(404, "not_found", "Not found").response(),
    }
}

// worker 0.8.7 exposes SQL but not transactionSync. Bind that one native method;
// this is platform glue, not a second implementation of room rules.
#[wasm_bindgen]
extern "C" {
    pub(crate) type RawStorage;
    #[wasm_bindgen(method, catch, js_name = transactionSync)]
    pub(crate) fn transaction_sync(
        this: &RawStorage,
        callback: &js_sys::Function,
    ) -> std::result::Result<JsValue, JsValue>;
}

#[derive(Deserialize, Serialize, Clone, Default)]
pub(crate) struct SocketState {
    pub(crate) joined: bool,
    pub(crate) owner: bool,
    pub(crate) generation: u64,
    pub(crate) uid: String,
    pub(crate) ip: String,
    pub(crate) window_start: u64,
    pub(crate) bytes: usize,
    pub(crate) pushes: usize,
}
#[derive(Deserialize)]
pub(crate) struct NumberRow {
    pub(crate) n: i64,
}
#[derive(Deserialize)]
pub(crate) struct RecordRow {
    pub(crate) seq: i64,
    pub(crate) spans: String,
    pub(crate) size: usize,
}
#[derive(Deserialize)]
pub(crate) struct BlobRow {
    #[serde(with = "serde_bytes")]
    pub(crate) bytes: Vec<u8>,
}
#[durable_object]
pub struct CloudDocument {
    pub(crate) state: State,
    pub(crate) env: Env,
    pub(crate) sql: SqlStorage,
    pub(crate) uploads: futures_util::lock::Mutex<()>,
    /// Serializes joins, leaves and registry cleanup; lifecycle changes never wait for it.
    pub(crate) membership: futures_util::lock::Mutex<()>,
}

impl CloudDocument {
    pub(crate) fn heads(&self) -> Result<Version> {
        let text = self.stored()?.heads.ok_or_else(|| fail("Missing heads"))?;
        version(&serde_json::from_str::<Vec<(String, u32)>>(&text)?)
    }
    pub(crate) fn transaction(&self, f: impl FnMut() -> Result<()> + 'static) -> Result<()> {
        let mut f = f;
        let callback = Closure::wrap(Box::new(move || {
            f().map(|_| JsValue::UNDEFINED).map_err(|e| JsValue::from_str(&e.to_string()))
        }) as Box<dyn FnMut() -> std::result::Result<JsValue, JsValue>>);
        let storage = self.state.storage();
        let raw: &RawStorage = storage.as_raw().unchecked_ref();
        raw.transaction_sync(callback.as_ref().unchecked_ref()).map_err(worker::Error::from)?;
        Ok(())
    }
    pub(crate) fn info(&self) -> Result<RoomInfo> {
        let state = self.stored()?;
        Ok(RoomInfo {
            document: state.document,
            phase: state.phase,
            seed_hash: state.seed_hash,
            seed_size: state.seed_size,
            app: state.app,
            compact: self.compact_due()?,
        })
    }
}

pub(crate) fn initialize(sql: &SqlStorage) {
    crate::document_state::initialize(sql);
    sql.exec("CREATE TABLE IF NOT EXISTS pending_registry(uid TEXT PRIMARY KEY,generation INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS members(uid TEXT PRIMARY KEY,generation INTEGER NOT NULL)",None).expect("document access tables");
    sql.exec("CREATE TABLE IF NOT EXISTS garbage(key TEXT PRIMARY KEY); CREATE TABLE IF NOT EXISTS resource_blobs(id TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS records(seq INTEGER PRIMARY KEY AUTOINCREMENT,spans TEXT NOT NULL,size INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS chunks(seq INTEGER NOT NULL,part INTEGER NOT NULL,bytes BLOB NOT NULL,PRIMARY KEY(seq,part))",None).expect("create sharing tables");
}

pub(crate) fn decode_hex(text: &str) -> Result<Vec<u8>> {
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
        let Some((route, document, hash)) = routes::resolve(&req)? else {
            return Refusal::new(404, "not_found", "Not found").response();
        };
        if route.scope != Scope::Document {
            return Refusal::new(404, "not_found", "Not found").response();
        }
        let Some(room) = document.as_deref() else {
            return Refusal::new(404, "not_found", "Not found").response();
        };
        let state = self.document_state()?;
        let identity = if route.op == Op::Init { None } else { Some(VerifiedIdentity::from_request(&req)?) };
        let uid = identity.as_ref().map_or("", |i| i.uid.as_str());
        let role = if state.is_some() {
            self.role_in_state(uid, state.as_ref().ok_or_else(|| fail("Missing state"))?)?
        } else {
            None
        };
        if let Err(refusal) =
            routes::authorize(route, routes::Caller { internal: route.internal, role }, state.as_ref().map(|s| s.phase))
        {
            return refusal.response();
        }
        match route.op {
            Op::Init => self.initialize_document(&mut req).await,
            Op::LibraryDescription => self.describe_document(role.ok_or_else(|| fail("Missing role"))?),
            Op::DeleteDocument | Op::InternalDelete => self.delete_document(room).await,
            Op::PatchDocument => self.patch_document(&mut req).await,
            Op::GetSharing => self.get_sharing(role.ok_or_else(|| fail("Missing role"))?),
            Op::EnableSharing => self.enable_sharing().await,
            Op::DisableSharing => self.disable_sharing().await,
            Op::Join => self.join(&mut req, uid, role).await,
            Op::Leave | Op::InternalLeave => self.leave(uid, role).await,
            Op::GetMetadata => self.get_metadata(),
            Op::GetDocument => Response::from_json(&self.info()?),
            Op::Open => self.open(&mut req, room).await,
            Op::PutSeed => self.put_seed(&mut req, room).await,
            Op::GetSeed => self.read_seed(uid, room).await,
            Op::ListBlobs => Response::from_json(&self.blobs()?),
            Op::GetBlob => self.read_blob(room, hash.as_deref(), uid).await,
            Op::PutBlob => self.put_blob(&mut req, room, hash.as_deref(), uid).await,
            Op::Compact => self.compact(&mut req, uid).await,
            Op::Sync => self.connect_socket(&req, uid, role == Some(DocumentRole::Owner)).await,
            _ => Refusal::new(404, "not_found", "Not found").response(),
        }
    }
    async fn alarm(&self) -> Result<Response> {
        if self.phase()? == Some(RoomPhase::Ended) {
            let document = self.document_state()?.map(|s| s.document).ok_or_else(|| fail("Missing document"))?;
            self.stop_room(&document, Metric::DocumentDeleted).await?;
        }
        {
            let _guard = self.uploads.lock().await;
            if let Some(document) = self.document_state()?.map(|s| s.document) {
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
                    let _ = self.send(&ws, Frame::Error { code: code.into(), message });
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
    let refusal = match error {
        crate::auth::IdentityError::Denied => {
            Refusal::new(401, "sign_in_required", "Sign in required or identity service unavailable")
        }
        crate::auth::IdentityError::Unavailable => {
            Refusal::new(503, "unavailable", "The sharing service refused this request")
        }
    };
    refusal.response()
}
