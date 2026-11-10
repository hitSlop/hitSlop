//! Single native Rust socket supervisor. Swift only starts/stops it and reads status.
use crate::client::*;
use crate::{attachments, journal};
use futures_util::{SinkExt, StreamExt};
use pop_contracts::hex;
use pop_contracts::relay::envelope;
use pop_contracts::relay::frame::{Frame, MAX_FRAME};
use pop_core::sync::PauseKind;
use pop_runtime::owner::{Owner, Reply, Request, SyncConnection, SyncEvent};
use pop_sharing_protocol::failure;
use pop_sharing_protocol::failure::{Outcome, Route};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, tungstenite};
use tokio_util::sync::CancellationToken;

pub(crate) mod account;
mod session;
pub(crate) use session::Connection;

type TransportResult<T> = std::result::Result<T, HttpFailure>;
struct Network {
    runtime: tokio::runtime::Runtime,
    http: reqwest::Client,
}
fn network() -> TransportResult<&'static Network> {
    static NETWORK: OnceLock<std::result::Result<Network, String>> = OnceLock::new();
    NETWORK
        .get_or_init(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .max_blocking_threads(2)
                .thread_name("pop.sharing.io")
                .enable_all()
                .build()
                .map_err(fail)?;
            // Cancellation drops DNS/connect/TLS/body futures. Platform getaddrinfo
            // work already running can finish later; its shared thread pool is capped.
            let http = reqwest::Client::builder()
                .http1_only()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(fail)?;
            Ok(Network { runtime, http })
        })
        .as_ref()
        .map_err(|error| HttpFailure::Other(error.clone()))
}
thread_local! {
    // Context for bounded blocking-pool effects. The async supervisor owns
    // cancellation explicitly; one-shot FFI calls are independent.
    static OPERATION: RefCell<Option<Operation>> = const { RefCell::new(None) };
}
struct Operation {
    cancellation: CancellationToken,
    identity: tokio::sync::watch::Receiver<Option<String>>,
}
fn cancellation() -> CancellationToken {
    OPERATION
        .with(|operation| operation.borrow().as_ref().map(|context| context.cancellation.clone()).unwrap_or_default())
}
fn fresh_identity(token: Option<&str>) -> Option<String> {
    OPERATION
        .with(|operation| operation.borrow().as_ref().and_then(|context| context.identity.borrow().clone()))
        .or_else(|| token.map(str::to_owned))
}
fn scoped<T>(context: Operation, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<Operation>);
    impl Drop for Reset {
        fn drop(&mut self) {
            OPERATION.with(|operation| *operation.borrow_mut() = self.0.take());
        }
    }
    let previous = OPERATION.with(|operation| operation.replace(Some(context)));
    let _reset = Reset(previous);
    run()
}
// Spawn onto the shared runtime instead of entering it on the caller. Sync APIs
// therefore also work when their caller is already inside another Tokio runtime.
fn run_network<T: Send + 'static>(
    future: impl Future<Output = TransportResult<T>> + Send + 'static,
) -> TransportResult<T> {
    let network = network()?;
    let token = cancellation();
    let (send, receive) = mpsc::sync_channel(1);
    network.runtime.spawn(async move {
        let result = tokio::select! {
            biased;
            _ = token.cancelled() => Err("Sharing stopped".into()),
            result = future => result,
        };
        let _ = send.send(result);
    });
    receive.recv().map_err(|_| HttpFailure::from("Sharing network stopped"))?
}
pub(crate) fn wait_reply<T>(reply: &mpsc::Receiver<T>, timeout: Duration) -> TransportResult<T> {
    let token = cancellation();
    let deadline = Instant::now() + timeout;
    loop {
        if token.is_cancelled() {
            return Err("Sharing stopped".into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(unconfirmed_owner());
        }
        match reply.recv_timeout(remaining.min(Duration::from_millis(25))) {
            Ok(value) => return Ok(value),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(unconfirmed_owner()),
        }
    }
}
pub(crate) fn pause(duration: Duration) -> TransportResult<()> {
    run_network(async move {
        tokio::time::sleep(duration).await;
        Ok(())
    })
}
fn unconfirmed_owner() -> HttpFailure {
    HttpFailure::Paused(pop_core::sync::Pause {
        kind: PauseKind::Refused,
        reason: "The owner result could not be confirmed; sharing is paused".into(),
    })
}
pub(crate) fn owner_request(owner: &Owner, request: Request) -> TransportResult<Reply> {
    if cancellation().is_cancelled() {
        return Err("Sharing stopped".into());
    }
    let effect = owner_effect(&request);
    let (send, reply) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |result| {
            let _ = send.send(result);
        }),
    );
    wait_reply(&reply, Duration::from_secs(20))?.map_err(|failure| owner_failure(failure, effect))
}
async fn response_body(mut response: reqwest::Response, limit: u64) -> TransportResult<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| HttpFailure::from("Relay body unavailable"))? {
        if chunk.len() as u64 > limit.saturating_sub(bytes.len() as u64) {
            return Err("Relay body exceeds its byte limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
type AsyncSocket = WebSocketStream<reqwest::Upgraded>;
async fn upgrade_async(key: &RoomKey) -> TransportResult<AsyncSocket> {
    let url = format!("{}/v1/documents/{}/sync", key.origin, key.room);
    upgrade_at(&url, key.bearer(), pop_sharing_protocol::SOCKET_PROTOCOL).await
}
async fn upgrade_at(url: &str, capability: &str, protocol: &str) -> TransportResult<AsyncSocket> {
    let client = network()?.http.clone();
    let nonce = tungstenite::handshake::client::generate_key();
    let response = client
        .get(url)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", &nonce)
        .header("Sec-WebSocket-Protocol", protocol)
        .bearer_auth(capability)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        // Invalid HTTP headers are network failures: Hyper cannot expose a
        // trustworthy status. Once headers are accepted, body errors keep status.
        .map_err(|_| HttpFailure::from("Relay socket unavailable"))?;
    let status = response.status().as_u16();
    if status != 101 {
        if status == 401 && protocol == pop_sharing_protocol::library::SOCKET_PROTOCOL {
            return Err(HttpFailure::Paused(pop_core::sync::Pause {
                kind: PauseKind::Refused,
                reason: "Refresh your account identity".into(),
            }));
        }
        let bytes = response_body(response, pop_sharing_protocol::CONTROL_BYTES as u64).await.unwrap_or_default();
        return Err(refusal(status, &bytes, Route::of(url)));
    }
    let headers = response.headers();
    let token = |name: &str, expected: &str| {
        headers
            .get(name)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|value| value.split(',').any(|v| v.trim().eq_ignore_ascii_case(expected)))
    };
    if !token("Connection", "upgrade")
        || !token("Upgrade", "websocket")
        || headers.get("Sec-WebSocket-Accept").map(|h| h.as_bytes())
            != Some(tungstenite::handshake::derive_accept_key(nonce.as_bytes()).as_bytes())
        || headers.get("Sec-WebSocket-Protocol").map(|h| h.as_bytes()) != Some(protocol.as_bytes())
    {
        return Err("Invalid relay socket upgrade".into());
    }
    let upgraded = response.upgrade().await.map_err(|_| HttpFailure::from("Relay socket unavailable"))?;
    let mut config = tungstenite::protocol::WebSocketConfig::default();
    let limit = if protocol == pop_sharing_protocol::library::SOCKET_PROTOCOL {
        pop_sharing_protocol::CONTROL_BYTES
    } else {
        MAX_FRAME
    };
    config.max_message_size = Some(limit);
    config.max_frame_size = Some(limit);
    let socket = WebSocketStream::from_raw_socket(upgraded, tungstenite::protocol::Role::Client, Some(config)).await;
    Ok(socket)
}

fn trace(event: &str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var("POP_SHARING_TRACE").is_ok_and(|v| v == "1")) {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        eprintln!("pop.sharing time={time} event={event}");
    }
}
fn trace_frame(frame: &Frame, incoming: bool) {
    let event = match (incoming, frame) {
        (false, Frame::Hello { .. }) => "send_hello",
        (false, Frame::Push { .. }) => "send_push",
        (true, Frame::Welcome { .. }) => "receive_welcome",
        (true, Frame::Records(_)) => "receive_records",
        (true, Frame::CaughtUp) => "receive_caught_up",
        (true, Frame::Ack { .. }) => "receive_ack",
        (true, Frame::Error { .. }) => "receive_error",
        _ => "unexpected_frame",
    };
    trace(event);
}
pub(crate) fn http(
    key: &RoomKey,
    method: &str,
    route: &str,
    token: Option<&str>,
    body: &[u8],
    headers: &[(&str, &str)],
) -> Result<Vec<u8>> {
    http_typed(key, method, route, token, body, headers).map_err(Into::into)
}
pub(crate) fn http_typed(
    key: &RoomKey,
    method: &str,
    route: &str,
    token: Option<&str>,
    body: &[u8],
    headers: &[(&str, &str)],
) -> std::result::Result<Vec<u8>, HttpFailure> {
    let limit = if route.split('?').next() == Some("seed") {
        pop_sharing_protocol::SEED_BYTES as u64
    } else if route.starts_with("blob/") {
        pop_contracts::wire::ATTACHMENT_FILE_BYTES as u64
    } else {
        pop_sharing_protocol::CONTROL_BYTES as u64
    };
    let suffix = if route.is_empty() {
        String::new()
    } else if route.starts_with('?') {
        route.to_owned()
    } else {
        format!("/{route}")
    };
    http_request_authorized(
        method,
        &format!("{}/v1/documents/{}{suffix}", key.origin, key.room),
        token,
        body,
        headers,
        limit,
    )
}
#[derive(Debug)]
pub(crate) enum HttpFailure {
    Ended,
    /// Another writer got there first, or creation is unfinished.
    Conflict,
    Busy(String),
    Paused(pop_core::sync::Pause),
    Other(String),
}
impl From<pop_contracts::Error> for HttpFailure {
    fn from(error: pop_contracts::Error) -> Self {
        pop_runtime::owner::Failure::from(error).into()
    }
}
impl From<pop_runtime::owner::Failure> for HttpFailure {
    fn from(error: pop_runtime::owner::Failure) -> Self {
        owner_failure(error, OwnerEffect::Other)
    }
}
#[derive(Clone, Copy)]
enum OwnerEffect {
    Other,
    Attachment,
}
fn owner_effect(request: &Request) -> OwnerEffect {
    match request {
        Request::PutAttachment { .. } => OwnerEffect::Attachment,
        _ => OwnerEffect::Other,
    }
}
fn owner_failure(error: pop_runtime::owner::Failure, effect: OwnerEffect) -> HttpFailure {
    let kind = match error.reason {
        Some(pop_contracts::wire::Code::StaleBase) if error.kind == pop_runtime::owner::FailureKind::Rejected => {
            return HttpFailure::Conflict;
        }
        Some(pop_contracts::wire::Code::RequiresUpdate) => PauseKind::NeedsUpdate,
        Some(pop_contracts::wire::Code::MissingDependencies) => PauseKind::MissingHistory,
        Some(pop_contracts::wire::Code::TooLarge) if matches!(effect, OwnerEffect::Attachment) => PauseKind::FilesFull,
        Some(_) => PauseKind::Refused,
        None if error.kind == pop_runtime::owner::FailureKind::Busy => return HttpFailure::Busy(error.message),
        // Full also represents local disk exhaustion, not just format capacity.
        // Other local failures can leave accepted work unconfirmed: never replay it.
        None => PauseKind::Refused,
    };
    HttpFailure::Paused(pop_core::sync::Pause { kind, reason: error.message })
}
impl From<pop_storage::store::Error> for HttpFailure {
    fn from(error: pop_storage::store::Error) -> Self {
        pop_runtime::owner::Failure::from(error).into()
    }
}
impl From<HttpFailure> for String {
    fn from(value: HttpFailure) -> Self {
        match value {
            HttpFailure::Ended => "Sharing ended".into(),
            HttpFailure::Other(message) | HttpFailure::Busy(message) => message,
            HttpFailure::Paused(pause) => pause.reason,
            HttpFailure::Conflict => Outcome::Conflict.message(409),
        }
    }
}
impl From<&str> for HttpFailure {
    fn from(value: &str) -> Self {
        Self::Other(value.into())
    }
}
impl From<String> for HttpFailure {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}
/// What a refused response means, for HTTP requests and the socket handshake alike.
/// `body` is the relay's `ApiFailure` JSON when it sent one.
pub(crate) fn refusal(status: u16, body: &[u8], route: Route) -> HttpFailure {
    let body = serde_json::from_slice::<pop_sharing_protocol::http::ApiFailure>(body).ok();
    let paused = |kind, reason| HttpFailure::Paused(pop_core::sync::Pause { kind, reason });
    let outcome = failure::classify(status, body.as_ref().map(|b| b.code.as_str()), route);
    match outcome {
        Outcome::NeedsUpdate => {
            paused(PauseKind::NeedsUpdate, body.map_or_else(|| outcome.message(status), |b| b.message))
        }
        Outcome::Ended => HttpFailure::Ended,
        Outcome::Conflict => HttpFailure::Conflict,
        Outcome::Refused => paused(PauseKind::Refused, outcome.message(status)),
        Outcome::Outgrown => paused(PauseKind::Outgrown, outcome.message(status)),
        Outcome::FilesFull => paused(PauseKind::FilesFull, outcome.message(status)),
        Outcome::SignIn | Outcome::Failed => HttpFailure::Other(outcome.message(status)),
    }
}
pub(crate) fn http_request(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: &[u8],
    headers: &[(&str, &str)],
    limit: u64,
) -> Result<Vec<u8>> {
    http_request_typed(method, url, token, body, headers, limit).map_err(Into::into)
}
pub(crate) fn http_request_typed(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: &[u8],
    headers: &[(&str, &str)],
    limit: u64,
) -> std::result::Result<Vec<u8>, HttpFailure> {
    http_request_authorized(method, url, token, body, headers, limit)
}
fn http_request_authorized(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: &[u8],
    headers: &[(&str, &str)],
    limit: u64,
) -> std::result::Result<Vec<u8>, HttpFailure> {
    let client = network()?.http.clone();
    let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(fail)?;
    let url = url.to_owned();
    let mut request = client.request(method.clone(), &url).body(body.to_vec());
    if let Some(token) = fresh_identity(token) {
        request = request.bearer_auth(token);
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    run_network(async move {
        let response = request.send().await.map_err(|_| HttpFailure::from("Sharing service is unavailable"))?;
        let status = response.status().as_u16();
        if !matches!(status, 200 | 204) {
            let bytes = response_body(response, pop_sharing_protocol::CONTROL_BYTES as u64).await.unwrap_or_default();
            return match refusal(status, &bytes, Route::of(&url)) {
                HttpFailure::Ended if method == reqwest::Method::DELETE => Ok(Vec::new()),
                failure => Err(failure),
            };
        }
        response_body(response, limit).await
    })
}

/// Control routes use an explicit bearer; locating a room requires no guest capability.
pub(crate) fn room_control(
    origin: &str,
    room: &str,
    method: &str,
    route: &str,
    identity: &str,
    body: &[u8],
) -> TransportResult<Vec<u8>> {
    http_request_typed(
        method,
        &format!("{origin}/v1/documents/{room}/{route}"),
        Some(identity),
        body,
        &[("Content-Type", "application/json")],
        pop_sharing_protocol::CONTROL_BYTES as u64,
    )
}

/// An access lookup keeps HTTP status until deciding whether access was denied.
/// Bad requests, missing routes and expired credentials cannot authorize local removal.
pub(crate) fn access_request(url: &str, identity: &str) -> TransportResult<Option<Vec<u8>>> {
    lookup(url, identity, &[403, 410])
}
/// A GET whose `absent` statuses are the answer `None`; any other refusal is an error.
pub(crate) fn lookup(url: &str, identity: &str, absent: &[u16]) -> TransportResult<Option<Vec<u8>>> {
    let client = network()?.http.clone();
    let request = client.get(url).bearer_auth(fresh_identity(Some(identity)).unwrap_or_else(|| identity.to_owned()));
    let absent = absent.to_vec();
    run_network(async move {
        let response = request.send().await.map_err(|_| HttpFailure::from("Sharing service is unavailable"))?;
        let status = response.status().as_u16();
        let bytes = response_body(response, pop_sharing_protocol::CONTROL_BYTES as u64).await?;
        match status {
            200 => Ok(Some(bytes)),
            status if absent.contains(&status) => Ok(None),
            _ => Err(refusal(status, &bytes, Route::Other)),
        }
    })
}
#[derive(Default)]
struct Queue {
    events: VecDeque<SyncEvent>,
    bytes: usize,
    overflow: bool,
}
impl Queue {
    fn enqueue(&mut self, event: SyncEvent) {
        if self.overflow {
            return;
        }
        let bytes = match &event {
            SyncEvent::Send { bytes } => bytes.len(),
            _ => 0,
        };
        if self.events.len() >= pop_contracts::relay::limits::QUEUED_FRAMES
            || self.bytes.saturating_add(bytes) > pop_contracts::relay::limits::QUEUED_BYTES
        {
            self.events.clear();
            self.bytes = 0;
            self.overflow = true;
            return;
        }
        self.bytes += bytes;
        self.events.push_back(event);
    }
}
struct Status {
    value: Mutex<(Phase, String, Option<PauseKind>)>,
    notice: Notice,
    cancellation: CancellationToken,
}
impl Status {
    fn set(&self, phase: Phase, detail: String) {
        self.update(phase, detail, None);
    }
    fn paused(&self, kind: PauseKind, reason: String) {
        self.update(Phase::Paused, reason, Some(kind));
    }
    fn update(&self, phase: Phase, detail: String, kind: Option<PauseKind>) {
        if self.cancellation.is_cancelled() {
            return;
        }
        let changed = {
            let mut old = self.value.lock().unwrap();
            let value = (phase, detail, kind);
            if *old == value {
                false
            } else {
                *old = value;
                true
            }
        };
        if changed {
            let callback = self.notice.lock().unwrap().clone();
            if let Some(callback) = callback
                && !self.cancellation.is_cancelled()
            {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
            }
        }
    }
}
// The synchronous fixture entry points drive the actual async implementation.
#[cfg(test)]
fn upgrade(key: &RoomKey) -> TransportResult<AsyncSocket> {
    let key = key.clone();
    run_network(async move { upgrade_async(&key).await })
}
#[cfg(test)]
fn connected(
    owner: &Arc<Owner>,
    key: &RoomKey,
    _stop: &AtomicBool,
    status: &Status,
    retried_full: &mut bool,
    journal: &Arc<journal::Journal>,
) -> TransportResult<bool> {
    session::connected_fixture(owner, key, status, retried_full, journal)
}

/// Versions use the seed route's canonical peer encoding.
pub(crate) fn compact(key: &RoomKey, base: &pop_core::sync::Base) -> std::result::Result<(), HttpFailure> {
    let version =
        |vv: &loro::VersionVector| pop_contracts::relay::frame::version_json(vv.iter().map(|(&p, &n)| (p, n as u32)));
    http_typed(
        key,
        "POST",
        "compact",
        Some(key.bearer()),
        &base.bytes,
        &[
            ("Content-Type", "application/octet-stream"),
            ("X-Sharing-SHA256", &hex(&Sha256::digest(&base.bytes))),
            ("X-Sharing-Version", &version(&base.frontier)),
        ],
    )?;
    Ok(())
}

#[cfg(test)]
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
#[cfg(test)]
mod tests {
    use super::*;
    use pop_sharing_protocol::http::{RoomInfo, RoomPhase};
    use pop_storage::store::{Mode, Share, Store};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn access_lookup_requires_an_explicit_permission_denial() {
        for status in [200, 400, 401, 403, 404, 410, 422, 429, 503] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/sharing", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    if stream.read(&mut byte).unwrap() == 0 {
                        break;
                    }
                    request.push(byte[0]);
                }
                write!(stream, "HTTP/1.1 {status} Result\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}")
                    .unwrap();
            });
            let result = access_request(&url, "identity");
            server.join().unwrap();
            match status {
                200 => assert_eq!(result.unwrap(), Some(b"{}".to_vec())),
                403 | 410 => assert_eq!(result.unwrap(), None),
                _ => assert!(result.is_err(), "HTTP {status} cannot authorize removal: {result:?}"),
            }
        }
    }
    use tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};

    #[test]
    fn local_owner_failures_preserve_recovery_and_stop_uncertain_replays() {
        use pop_contracts::wire::Code;
        use pop_runtime::owner::{Failure, FailureKind};
        for (kind, reason, expected) in [
            (FailureKind::Rejected, Some(Code::RequiresUpdate), Some(PauseKind::NeedsUpdate)),
            (FailureKind::Rejected, Some(Code::MissingDependencies), Some(PauseKind::MissingHistory)),
            (FailureKind::Busy, None, None),
            (FailureKind::Full, None, Some(PauseKind::Refused)),
            (FailureKind::Invalidated, None, Some(PauseKind::Refused)),
            (FailureKind::Failed, None, Some(PauseKind::Refused)),
            (FailureKind::SaveFailed, None, Some(PauseKind::Refused)),
            (FailureKind::Replaced, None, Some(PauseKind::Refused)),
            (FailureKind::Closing, None, Some(PauseKind::Refused)),
            (FailureKind::Closed, None, Some(PauseKind::Refused)),
            (FailureKind::Moved, None, Some(PauseKind::Refused)),
            (FailureKind::Locked, None, Some(PauseKind::Refused)),
            (FailureKind::ReadOnly, None, Some(PauseKind::Refused)),
        ] {
            let message = format!("local failure {kind:?} {reason:?}");
            let failure = Failure { kind, reason, message: message.clone(), op_index: None };
            match (HttpFailure::from(failure), expected) {
                (HttpFailure::Paused(pause), Some(expected)) => {
                    assert_eq!(pause.kind, expected, "{kind:?} {reason:?}");
                    assert_eq!(pause.reason, message);
                }
                (HttpFailure::Busy(_), None) => {}
                (actual, expected) => panic!("{kind:?} {reason:?}: expected {expected:?}, received {actual:?}"),
            }
        }
        let (_send, reply) = mpsc::channel::<()>();
        for disconnected in [false, true] {
            let failure = if disconnected {
                let (send, reply) = mpsc::channel::<()>();
                drop(send);
                wait_reply(&reply, Duration::from_secs(1)).unwrap_err()
            } else {
                wait_reply(&reply, Duration::ZERO).unwrap_err()
            };
            assert!(
                matches!(failure, HttpFailure::Paused(pop_core::sync::Pause { kind: PauseKind::Refused, .. })),
                "an unconfirmed owner result must not be automatically replayed: {failure:?}"
            );
        }
        let attachment = Failure {
            kind: FailureKind::Rejected,
            reason: Some(Code::TooLarge),
            message: "attachment capacity".into(),
            op_index: None,
        };
        assert!(matches!(
            owner_failure(attachment, OwnerEffect::Attachment),
            HttpFailure::Paused(pop_core::sync::Pause { kind: PauseKind::FilesFull, .. })
        ));
        let stale = Failure {
            kind: FailureKind::Rejected,
            reason: Some(Code::StaleBase),
            message: "frontier advanced".into(),
            op_index: None,
        };
        assert!(matches!(HttpFailure::from(stale), HttpFailure::Conflict));
    }

    fn upgrade_refusal(headers: &str, body: &[u8], status: u16) -> HttpFailure {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
            identity: "memory-only-test-token".into(),
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        let headers = headers.to_owned();
        let body = body.to_vec();
        let serving = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            write!(stream, "HTTP/1.1 {status} Reply\r\nConnection: close\r\n{headers}\r\n").unwrap();
            for fragment in body.chunks(3) {
                if stream.write_all(fragment).is_err() {
                    break;
                }
            }
        });
        let result = upgrade(&key);
        serving.join().unwrap();
        match result {
            Err(failure) => failure,
            Ok(_) => panic!("a refused upgrade must not create a socket"),
        }
    }

    #[test]
    fn fragmented_and_chunked_upgrade_refusals_preserve_the_update_reason() {
        let body = br#"{"code":"requires_update","message":"Update pop to keep sharing"}"#;
        let chunked = format!(
            "d;extension=yes\r\n{}\r\n{:x}\r\n{}\r\n0\r\nX-Trailer: ignored\r\n\r\n",
            std::str::from_utf8(&body[..13]).unwrap(),
            body.len() - 13,
            std::str::from_utf8(&body[13..]).unwrap()
        );
        for (headers, bytes) in [
            (format!("Content-Length: {}\r\n", body.len()), body.as_slice()),
            (String::new(), body.as_slice()),
            ("Transfer-Encoding: chunked\r\n".into(), chunked.as_bytes()),
        ] {
            let HttpFailure::Paused(pause) = upgrade_refusal(&headers, bytes, 400) else {
                panic!("a permanent upgrade refusal must pause");
            };
            assert_eq!(pause.kind, PauseKind::NeedsUpdate);
            assert_eq!(pause.reason, "Update pop to keep sharing");
        }
    }

    #[test]
    fn invalid_upgrade_bodies_keep_the_status_based_refusal() {
        let limit = pop_sharing_protocol::CONTROL_BYTES;
        for (headers, body) in [
            ("Content-Length: 20\r\n".into(), b"short".to_vec()),
            ("Content-Length: 0\r\n".into(), Vec::new()),
            ("Content-Length: 3\r\n".into(), b"bad".to_vec()),
            (format!("Content-Length: {}\r\n", limit + 1), vec![b'x'; limit + 1]),
            (String::new(), vec![b'x'; limit + 1]),
            ("Transfer-Encoding: chunked\r\n".into(), b"5\r\nshort\r\n".to_vec()),
            ("Transfer-Encoding: chunked\r\n".into(), b"invalid\r\n".to_vec()),
            ("Transfer-Encoding: chunked\r\n".into(), b"0\r\n".to_vec()),
        ] {
            let HttpFailure::Paused(pause) = upgrade_refusal(&headers, &body, 400) else {
                panic!("a permanent upgrade refusal must pause");
            };
            assert_eq!(pause.kind, PauseKind::Refused, "{headers}");
        }
        for status in [429, 503] {
            assert!(matches!(upgrade_refusal("Content-Length: 20\r\n", b"short", status), HttpFailure::Other(_)));
        }
    }

    /// Exercise the actual socket loop and HTTP attachment barrier over loopback.
    #[allow(clippy::result_large_err, reason = "tungstenite fixes the handshake callback error type")]
    fn local_connection(close: CloseCode) -> (bool, Phase, Option<PauseKind>) {
        let (_dir, owner) = crate::client::tests::fixture();
        request(&owner, Request::Share { share: Share { endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into() } })
            .unwrap();
        let store = Store::open(owner.path(), Mode::Snapshot).unwrap();
        let checkpoint = store.document().unwrap().checkpoint().unwrap();
        let vv = loro::LoroDoc::decode_import_blob_meta(&checkpoint, false).unwrap().partial_end_vv;
        let version: pop_contracts::relay::frame::Version = vv.iter().map(|(&p, &n)| (p, n as u32)).collect();
        let info = serde_json::to_vec(&RoomInfo {
            document: owner.app().document_uuid.clone().unwrap(),
            phase: RoomPhase::Live,
            seed_hash: None,
            seed_size: None,
            app: None,
            compact: false,
        })
        .unwrap();
        let remote_bytes = b"quiet-room attachment".to_vec();
        let remote = pop_storage::store::Attachment::from_bytes(&remote_bytes).unwrap();
        let remote_id = remote.id.clone();
        let inventory = serde_json::to_vec(&vec![pop_sharing_protocol::resources::Resource {
            key: remote.id.clone(),
            hash: remote.id,
            size: remote.bytes,
            media_type: remote.media_type,
        }])
        .unwrap();
        let observing_owner = owner.clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let key = RoomKey {
            room: owner.app().document_uuid.clone().unwrap(),
            identity: "memory-only-test-token".into(),
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let serving_stop = stop.clone();
        let serving = thread::spawn(move || {
            let mut handlers = vec![];
            while !serving_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                let info = info.clone();
                let version = version.clone();
                let inventory = inventory.clone();
                let remote_bytes = remote_bytes.clone();
                let remote_id = remote_id.clone();
                let observing_owner = observing_owner.clone();
                handlers.push(thread::spawn(move || {
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_secs(25))).unwrap();
                    let mut peek = [0; 1024];
                    let n = stream.peek(&mut peek).unwrap();
                    let first = String::from_utf8_lossy(&peek[..n]);
                    if first.contains("/sync ") {
                        let mut ws = tungstenite::accept_hdr(
                            stream,
                            |request: &tungstenite::handshake::server::Request,
                             mut response: tungstenite::handshake::server::Response| {
                                response.headers_mut().insert(
                                    "Sec-WebSocket-Protocol",
                                    request.headers()["Sec-WebSocket-Protocol"].clone(),
                                );
                                Ok(response)
                            },
                        )
                        .unwrap();
                        assert!(matches!(ws.read().unwrap(), Message::Binary(_)));
                        for frame in [
                            Frame::Welcome { permission: pop_contracts::relay::frame::Permission::Write, version },
                            Frame::CaughtUp,
                        ] {
                            ws.send(Message::Binary(envelope::encode(frame).unwrap().into())).unwrap();
                        }
                        // No Records frame is sent. The initial connection must hydrate files anyway.
                        let deadline = Instant::now() + Duration::from_secs(5);
                        loop {
                            if matches!(
                                request(&observing_owner, Request::ReadAttachment { id: remote_id.clone() }).unwrap(),
                                Reply::Bytes { bytes: Some(_) }
                            ) {
                                break;
                            }
                            assert!(
                                Instant::now() < deadline,
                                "a quiet connection never downloaded its missing attachment"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        ws.send(Message::Text("pong".into())).unwrap();
                        ws.send(Message::Pong(vec![1, 2].into())).unwrap();
                        if close == CloseCode::Error {
                            assert_eq!(
                                ws.read().unwrap(),
                                Message::Text("ping".into()),
                                "idle connections keep the relay socket alive"
                            );
                            ws.send(Message::Text("pong".into())).unwrap();
                        }
                        ws.close(Some(CloseFrame { code: close, reason: "retry".into() })).unwrap();
                        // Read the client's close/flush so the stream remains live through its barrier.
                        let _ = ws.read();
                    } else {
                        let mut header = vec![];
                        while !header.ends_with(b"\r\n\r\n") {
                            let mut byte = [0];
                            stream.read_exact(&mut byte).unwrap();
                            header.push(byte[0]);
                        }
                        let header = String::from_utf8(header).unwrap();
                        let size: usize = header
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned))
                            .unwrap_or_else(|| "0".into())
                            .parse()
                            .unwrap();
                        stream.read_exact(&mut vec![0; size]).unwrap();
                        let (code, body) = if header.starts_with("PUT ") {
                            (413, &[][..])
                        } else if header.contains("/blobs ") {
                            (200, inventory.as_slice())
                        } else if header.contains(&format!("/blob/{remote_id} ")) {
                            (200, remote_bytes.as_slice())
                        } else {
                            (200, info.as_slice())
                        };
                        write!(
                            stream,
                            "HTTP/1.1 {code} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .unwrap();
                        stream.write_all(body).unwrap();
                    }
                }));
            }
            for handler in handlers {
                handler.join().unwrap();
            }
        });
        let status = Status {
            value: Mutex::new((Phase::Offline, String::new(), None)),
            notice: Arc::default(),
            cancellation: CancellationToken::new(),
        };
        let result = connected(
            &owner,
            &key,
            &stop,
            &status,
            &mut false,
            &crate::client::tests::test_journal(&owner, &key, true),
        );
        stop.store(true, Ordering::Relaxed);
        serving.join().unwrap();
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
        let (phase, _, kind) = status.value.into_inner().unwrap();
        (result.unwrap_or_else(|e| panic!("{}", String::from(e))), phase, kind)
    }
    #[test]
    fn keepalive_pong_and_transient_closes_reconnect_without_pausing() {
        for code in [CloseCode::Error, CloseCode::Again] {
            let (terminal, phase, kind) = local_connection(code);
            assert!(!terminal);
            assert_ne!(phase, Phase::Paused);
            assert_eq!(kind, None);
        }
    }

    /// Answer the socket upgrade with `answer` (or hang up) and report what `connected` made of it.
    fn handshake(
        answer: Option<(u16, &'static str)>,
    ) -> (std::result::Result<bool, HttpFailure>, Phase, Option<PauseKind>) {
        let (_dir, owner) = crate::client::tests::fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
            identity: "identity-token".into(),
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        let serving = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            assert!(String::from_utf8(header).unwrap().starts_with(&format!(
                "GET /v1/documents/{}/sync ",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned()
            )));
            if let Some((code, body)) = answer {
                write!(
                    stream,
                    "HTTP/1.1 {code} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let status = Status {
            value: Mutex::new((Phase::Offline, String::new(), None)),
            notice: Arc::default(),
            cancellation: CancellationToken::new(),
        };
        let result = connected(
            &owner,
            &key,
            &AtomicBool::new(false),
            &status,
            &mut false,
            &crate::client::tests::test_journal(&owner, &key, true),
        );
        // The relay saw one upgrade: a terminal answer does not loop back for another.
        serving.join().unwrap();
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
        let (phase, _, kind) = status.value.into_inner().unwrap();
        (result, phase, kind)
    }
    #[test]
    fn permanent_handshake_refusals_pause_instead_of_reconnecting() {
        for (code, body, kind) in [
            (403, "", PauseKind::Refused),
            (404, "", PauseKind::Refused),
            (400, r#"{"code":"requires_update","message":"Update pop to keep sharing"}"#, PauseKind::NeedsUpdate),
        ] {
            let (result, phase, paused) = handshake(Some((code, body)));
            assert!(matches!(result, Ok(true)), "{code} is terminal: {result:?}");
            assert_eq!((phase, paused), (Phase::Paused, Some(kind)), "{code}");
        }
        let (result, phase, paused) = handshake(Some((410, "")));
        assert!(matches!(result, Ok(true)));
        assert_eq!((phase, paused), (Phase::Ended, None));
    }
    #[test]
    fn transient_handshake_failures_stay_retryable() {
        for answer in [Some((503, "")), Some((429, "")), None] {
            let (result, phase, paused) = handshake(answer);
            assert!(matches!(result, Err(HttpFailure::Other(_))), "{answer:?} retries: {result:?}");
            assert_ne!(phase, Phase::Paused);
            assert_eq!(paused, None);
        }
    }
}
