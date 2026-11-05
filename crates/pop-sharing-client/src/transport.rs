//! Single native Rust socket supervisor. Swift only starts/stops it and reads status.
use super::*;
use futures_util::{SinkExt, StreamExt};
use pop_core::sync::{MAX_FRAME, PauseKind};
use pop_runtime::owner::{SyncConnection, SyncEvent};
use pop_sharing_protocol::{
    envelope,
    failure::{self, Outcome, Route},
    frame::Frame,
    limits,
    maintenance::{self, FullState, MaintenanceAction, Step},
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{self, Message},
};
use tokio_util::sync::CancellationToken;

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
    // A supervisor's synchronous helpers all share its cancellation scope. Public
    // synchronous entry points outside a supervisor use independent operations.
    static OPERATION: RefCell<Option<CancellationToken>> = const { RefCell::new(None) };
}
fn cancellation() -> CancellationToken {
    OPERATION.with(|operation| operation.borrow().clone().unwrap_or_default())
}
fn scoped<T>(token: CancellationToken, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<CancellationToken>);
    impl Drop for Reset {
        fn drop(&mut self) {
            OPERATION.with(|operation| *operation.borrow_mut() = self.0.take());
        }
    }
    let previous = OPERATION.with(|operation| operation.replace(Some(token)));
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
    let attachment = matches!(&request, Request::PutAttachment { .. });
    let (send, reply) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |result| {
            let _ = send.send(result);
        }),
    );
    wait_reply(&reply, Duration::from_secs(20))?.map_err(|failure| owner_failure(failure, attachment))
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
struct Socket(Option<AsyncSocket>);
impl Socket {
    fn send(&mut self, message: Message) -> TransportResult<()> {
        let mut socket = self.0.take().ok_or("Socket closed")?;
        let (socket, result) = run_network(async move {
            let result = tokio::time::timeout(Duration::from_secs(10), socket.send(message))
                .await
                .map_err(|_| HttpFailure::from("Socket send timed out"))?
                .map_err(|_| HttpFailure::from("Socket send failed"));
            Ok((socket, result))
        })?;
        self.0 = Some(socket);
        result
    }
    fn read(&mut self) -> TransportResult<Option<Message>> {
        let mut socket = self.0.take().ok_or("Socket closed")?;
        let (socket, result) = run_network(async move {
            let result = match tokio::time::timeout(Duration::from_millis(5), socket.next()).await {
                Err(_) => Ok(None),
                Ok(Some(Ok(message))) => Ok(Some(message)),
                Ok(Some(Err(_))) | Ok(None) => Err("Socket closed".into()),
            };
            Ok((socket, result))
        })?;
        self.0 = Some(socket);
        result
    }
    fn close(&mut self) {
        if let Some(mut socket) = self.0.take() {
            let _ = run_network(async move {
                let _ = tokio::time::timeout(Duration::from_millis(100), socket.close(None)).await;
                Ok(())
            });
        }
    }
}
fn upgrade(key: &RoomKey) -> TransportResult<Socket> {
    let client = network()?.http.clone();
    let url = format!("{}/v1/documents/{}/sync", key.origin, key.room);
    let capability = key.bearer().to_owned();
    let anonymous = key.capability().is_some();
    run_network(async move {
        let nonce = tungstenite::handshake::client::generate_key();
        let response = client
            .get(&url)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", &nonce)
            .header("Sec-WebSocket-Protocol", pop_sharing_protocol::SOCKET_PROTOCOL)
            .bearer_auth(capability)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            // Invalid HTTP headers are network failures: Hyper cannot expose a
            // trustworthy status. Once headers are accepted, body errors keep status.
            .map_err(|_| HttpFailure::from("Relay socket unavailable"))?;
        let status = response.status().as_u16();
        if status == 403 && anonymous {
            return Err(HttpFailure::Ended);
        }
        if status != 101 {
            let bytes = response_body(response, pop_sharing_protocol::CONTROL_BYTES as u64).await.unwrap_or_default();
            return Err(refusal(status, &bytes, Route::of(&url)));
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
            || headers.get("Sec-WebSocket-Protocol").map(|h| h.as_bytes())
                != Some(pop_sharing_protocol::SOCKET_PROTOCOL.as_bytes())
        {
            return Err("Invalid relay socket upgrade".into());
        }
        let upgraded = response.upgrade().await.map_err(|_| HttpFailure::from("Relay socket unavailable"))?;
        let mut config = tungstenite::protocol::WebSocketConfig::default();
        config.max_message_size = Some(MAX_FRAME);
        config.max_frame_size = Some(MAX_FRAME);
        let socket =
            WebSocketStream::from_raw_socket(upgraded, tungstenite::protocol::Role::Client, Some(config)).await;
        Ok(Socket(Some(socket)))
    })
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
        pop_core::ATTACHMENT_FILE_BYTES as u64
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
        key.capability().is_some(),
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
impl From<pop_core::Error> for HttpFailure {
    fn from(error: pop_core::Error) -> Self {
        pop_runtime::owner::Failure::from(error).into()
    }
}
impl From<pop_runtime::owner::Failure> for HttpFailure {
    fn from(error: pop_runtime::owner::Failure) -> Self {
        owner_failure(error, false)
    }
}
fn owner_failure(error: pop_runtime::owner::Failure, attachment: bool) -> HttpFailure {
    let kind = match error.reason {
        Some(pop_core::Code::RequiresUpdate) => PauseKind::NeedsUpdate,
        Some(pop_core::Code::MissingDependencies) => PauseKind::MissingHistory,
        Some(pop_core::Code::TooLarge) if attachment => PauseKind::FilesFull,
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
    http_request_authorized(method, url, token, body, headers, limit, false)
}
fn http_request_authorized(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: &[u8],
    headers: &[(&str, &str)],
    limit: u64,
    anonymous: bool,
) -> std::result::Result<Vec<u8>, HttpFailure> {
    let client = network()?.http.clone();
    let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(fail)?;
    let url = url.to_owned();
    let mut request = client.request(method.clone(), &url).body(body.to_vec());
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    run_network(async move {
        let response = request.send().await.map_err(|_| HttpFailure::from("Sharing service is unavailable"))?;
        let status = response.status().as_u16();
        if status == 403 && anonymous {
            return Err(HttpFailure::Ended);
        }
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
        if self.events.len() >= pop_sharing_protocol::QUEUED_FRAMES
            || self.bytes.saturating_add(bytes) > pop_sharing_protocol::QUEUED_BYTES
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
}
impl Status {
    fn set(&self, phase: Phase, detail: String) {
        self.update(phase, detail, None);
    }
    fn paused(&self, kind: PauseKind, reason: String) {
        self.update(Phase::Paused, reason, Some(kind));
    }
    fn update(&self, phase: Phase, detail: String, kind: Option<PauseKind>) {
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
            if let Some(callback) = callback {
                callback();
            }
        }
    }
}
pub(crate) struct Connection {
    stop: Arc<AtomicBool>,
    cancellation: CancellationToken,
    status: Arc<Status>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Connection {
    pub fn start(owner: Arc<Owner>, key: RoomKey, notice: Notice, access: Arc<epochs::Access>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let cancellation = CancellationToken::new();
        let scope = cancellation.clone();
        let status = Arc::new(Status { value: Mutex::new((Phase::Offline, "Connecting".into(), None)), notice });
        let (running, output) = (stop.clone(), status.clone());
        let thread = thread::spawn(move || {
            scoped(scope, || {
                let mut backoff = Duration::from_millis(limits::BACKOFF_MIN_MS);
                let mut retried_full = false;
                while !running.load(Ordering::Relaxed) {
                    let attempt = (|| {
                        trace("bootstrap_start");
                        epochs::bootstrap(&owner, &key, &access)?;
                        trace("bootstrap_done");
                        connected(&owner, &key, &running, &output, &mut retried_full, &access)
                    })();
                    let ended = matches!(&attempt, Err(HttpFailure::Ended))
                        || matches!(&attempt, Ok(true)) && output.value.lock().unwrap().0 == Phase::Ended;
                    if ended && access.identity.is_none() {
                        // A guest keeps the existing canonical file. Clearing membership
                        // first permits the ordinary unshared save policy to reclaim space.
                        let retained = epochs::retry_busy(&owner, || Request::Unshare)
                            .and_then(|_| epochs::retry_busy(&owner, || Request::Flush));
                        let detail = match retained {
                            Ok(_) => "Sharing ended. You keep your local copy.".into(),
                            Err(error) => format!(
                                "Sharing ended. Your local edits remain open; retry saving: {}",
                                String::from(error)
                            ),
                        };
                        output.set(Phase::Ended, detail);
                        break;
                    }
                    match attempt {
                        Ok(true) => break,
                        Ok(false) => output.set(Phase::Offline, "Offline; reconnecting".into()),
                        Err(HttpFailure::Ended) => {
                            output.set(Phase::Ended, "Ended".into());
                            break;
                        }
                        Err(HttpFailure::Paused(pause)) => {
                            output.paused(pause.kind, pause.reason);
                            break;
                        }
                        Err(error) => {
                            let error: String = error.into();
                            output.set(Phase::Offline, format!("Offline; reconnecting: {error}"))
                        }
                    }
                    let until = Instant::now() + backoff;
                    while Instant::now() < until && !running.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(25));
                    }
                    backoff = (backoff * 2).min(Duration::from_millis(limits::BACKOFF_MAX_MS));
                }
                trace("supervisor_stopped");
            })
        });
        Self { stop, cancellation, status, thread: Some(thread) }
    }
    pub fn status(&self) -> (Phase, String, Option<PauseKind>) {
        self.status.value.lock().unwrap().clone()
    }
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.cancellation.cancel();
        if let Some(thread) = self.thread.take()
            && thread.thread().id() != thread::current().id()
        {
            let _ = thread.join();
        }
    }
}
fn full_state(waiting: bool, retried: bool) -> FullState {
    if !waiting {
        FullState::Available
    } else if retried {
        FullState::Retried
    } else {
        FullState::Waiting
    }
}
struct Attached<'a>(&'a SyncConnection);
impl Drop for Attached<'_> {
    fn drop(&mut self) {
        self.0.detach();
    }
}
fn connected(
    owner: &Arc<Owner>,
    key: &RoomKey,
    stop: &AtomicBool,
    status: &Status,
    retried_full: &mut bool,
    access: &epochs::Access,
) -> std::result::Result<bool, HttpFailure> {
    let mut confirmed_epoch = access
        .journal
        .transaction()
        .map_err(epochs::local)?
        .current()
        .map_err(epochs::local)?
        .ok_or_else(|| epochs::local("The relay epoch has not been installed"))?
        .epoch;
    let mut socket = match upgrade(key) {
        Ok(socket) => socket,
        Err(error) => {
            match error {
                HttpFailure::Ended => status.set(Phase::Ended, "Ended".into()),
                HttpFailure::Paused(pause) => status.paused(pause.kind, pause.reason),
                error => return Err(error),
            }
            return Ok(true);
        }
    };
    trace("socket_opened");
    let queue = Arc::new(Mutex::new(Queue::default()));
    let sink = queue.clone();
    let admission = owner.sync_connection(Arc::new(move |e| sink.lock().unwrap().enqueue(e)));
    let attached = Attached(&admission);
    request(owner, Request::State)?;
    trace("owner_attached");
    let mut core_status = (Phase::Syncing, "Syncing".to_owned());
    let mut uploaded = Vec::new();
    let mut check_at = Instant::now();
    let mut compact_at = None;
    let mut waiting = false;
    let mut files_waiting = false;
    let deep = {
        let saved = Store::open(owner.path(), Mode::Snapshot).map_err(HttpFailure::from)?;
        let deep = saved.document().map_err(HttpFailure::from)?.supports_deep_compaction();
        saved.close().map_err(HttpFailure::from)?;
        deep
    };

    let ping_every = Duration::from_millis(limits::PING_MS);
    let mut ping_at = Instant::now() + ping_every;
    let result = (|| {
        trace("inventory_start");
        uploaded = resources::remote_blobs(key)?;
        trace("inventory_done");
        while !stop.load(Ordering::Relaxed) {
            let now = Instant::now();
            if now >= ping_at {
                socket.send(Message::Text("ping".into())).map_err(|_| fail("Socket send failed"))?;
                ping_at = now + ping_every;
            }
            if now >= check_at {
                let info = room_info(key, None)?;
                check_at = now + Duration::from_millis(limits::ROOM_RECHECK_MS);
                let full = if files_waiting {
                    if *retried_full { FullState::FilesRetried } else { FullState::FilesWaiting }
                } else {
                    full_state(waiting, *retried_full)
                };
                let step = Step::Probe { compact: info.compact };
                let mut action = maintenance::decide(full, step, 0);
                if matches!(action, MaintenanceAction::Jitter { .. }) {
                    let mut random = [0u8; 2];
                    getrandom::getrandom(&mut random).map_err(|_| fail("Cannot schedule compaction"))?;
                    action = maintenance::decide(full, step, u16::from_le_bytes(random));
                }
                match action {
                    MaintenanceAction::Outgrown => {
                        status.paused(PauseKind::Outgrown, "This document has outgrown sharing".into());
                        return Ok(true);
                    }
                    MaintenanceAction::Jitter { milliseconds } => {
                        compact_at = Some(now + Duration::from_millis(milliseconds));
                    }
                    _ => {}
                }
            }
            if compact_at.is_some_and(|at| now >= at) {
                // Recheck after jitter: another writer may already have compacted.
                let info = room_info(key, None)?;
                let full = if files_waiting {
                    if *retried_full { FullState::FilesRetried } else { FullState::FilesWaiting }
                } else {
                    full_state(waiting, *retried_full)
                };
                let action = maintenance::decide(full, Step::Recheck { compact: info.compact }, 0);
                if action == MaintenanceAction::Reconnect {
                    *retried_full = true;
                    return Ok(false);
                }
                if action == MaintenanceAction::Idle {
                    compact_at = None;
                }
                if action == MaintenanceAction::ReadBase {
                    if files_waiting && deep {
                        let epoch = resolution::epoch(key, access.identity.as_deref().unwrap_or(key.bearer()))?;
                        epochs::publish(
                            owner,
                            key,
                            access,
                            pop_sharing_protocol::lifecycle::EpochBase {
                                seed_hash: epoch.seed_hash,
                                heads: epoch.heads,
                            },
                        )?;
                        *retried_full = true;
                        return Ok(false);
                    }
                    let (send, receive) = std::sync::mpsc::channel();
                    admission.base(move |base| {
                        let _ = send.send(base);
                    });
                    match wait_reply(&receive, Duration::from_secs(30))? {
                        Err(pause) => {
                            status.paused(pause.kind, pause.reason);
                            return Ok(true);
                        }
                        Ok(None) => {
                            // Not eligible yet (edits in flight); each retry re-reads RoomInfo,
                            // so stay well inside the guest metadata budget.
                            if let MaintenanceAction::RetryBase { milliseconds } =
                                maintenance::decide(full, Step::BaseUnavailable, 0)
                            {
                                compact_at = Some(now + Duration::from_millis(milliseconds));
                            }
                        }
                        Ok(Some(base)) => {
                            compact_at = None;
                            let result = if deep {
                                epochs::publish(
                                    owner,
                                    key,
                                    access,
                                    pop_sharing_protocol::lifecycle::EpochBase {
                                        seed_hash: confirmed_epoch.seed_hash.clone(),
                                        heads: pop_sharing_protocol::frame::version_pairs(
                                            base.frontier.iter().map(|(&peer, &end)| (peer, end as u32)),
                                        ),
                                    },
                                )
                            } else {
                                compact(key, &base)
                            };
                            match result {
                                Ok(()) | Err(HttpFailure::Conflict) => {
                                    if deep {
                                        *retried_full = waiting;
                                        return Ok(false);
                                    }
                                    if maintenance::decide(full, Step::Compacted, 0) == MaintenanceAction::Reconnect {
                                        *retried_full = true;
                                        return Ok(false);
                                    }
                                }
                                // Pauses, an outgrown room among them, end the connection below.
                                Err(error) => return Err(error),
                            }
                        }
                    }
                }
            }
            let pending = std::mem::take(&mut *queue.lock().unwrap());
            if pending.overflow {
                return Err(fail("Socket queue budget exceeded").into());
            }
            for event in pending.events {
                match event {
                    SyncEvent::Reconnect => return Ok(false),
                    SyncEvent::Status { status: s } => {
                        if let pop_core::sync::Status::Paused { kind, reason } = &s {
                            status.paused(*kind, reason.clone());
                            if *kind == PauseKind::RoomFull {
                                waiting = true;
                                check_at = Instant::now();
                                continue;
                            }
                            return Ok(true);
                        }
                        if s == pop_core::sync::Status::Synced
                            && epochs::confirm(owner, &admission, access, &mut confirmed_epoch)?
                        {
                            *retried_full = false;
                            waiting = false;
                            files_waiting = false;
                        }
                        let phase = match &s {
                            pop_core::sync::Status::Offline => Phase::Offline,
                            pop_core::sync::Status::Syncing => Phase::Syncing,
                            pop_core::sync::Status::Synced => Phase::Synced,
                            pop_core::sync::Status::Paused { .. } => Phase::Paused,
                        };
                        core_status = (phase, format!("{s:?}"));
                        if !waiting {
                            status.set(core_status.0.clone(), core_status.1.clone());
                        }
                    }
                    SyncEvent::Send { bytes } => {
                        let frame = Frame::decode(&bytes).map_err(fail)?;
                        trace_frame(&frame, false);
                        if matches!(&frame, Frame::Push { .. }) {
                            status.set(Phase::Syncing, "Syncing files".into());
                            trace("upload_blobs_start");
                            match resources::upload_blobs(owner, key, &mut uploaded) {
                                Err(HttpFailure::Paused(pause)) if deep && pause.kind == PauseKind::FilesFull => {
                                    files_waiting = true;
                                    waiting = true;
                                    check_at = Instant::now();
                                    status.paused(pause.kind, pause.reason);
                                    continue;
                                }
                                result => result?,
                            }
                            trace("upload_blobs_done");
                        }
                        let message = envelope::encode(frame).map_err(fail)?;
                        socket.send(Message::Binary(message.into())).map_err(|_| fail("Socket send failed"))?;
                    }
                }
            }
            match socket.read() {
                Ok(Some(Message::Binary(bytes))) => {
                    let frame = envelope::decode(&bytes).map_err(|e| {
                        HttpFailure::Paused(pop_core::sync::Pause { kind: PauseKind::Refused, reason: fail(e) })
                    })?;
                    trace_frame(&frame, true);
                    if matches!(&frame,Frame::Error{code,..} if *code == pop_sharing_protocol::frame::ERROR_ENDED) {
                        status.set(Phase::Ended, "Ended".into());
                        return Ok(true);
                    }
                    if matches!(frame, Frame::Records(_)) {
                        status.set(Phase::Syncing, "Receiving files".into());
                        trace("download_blobs_start");
                        resources::download_blobs(owner, key, &mut uploaded)?;
                        trace("download_blobs_done");
                    }
                    if !admission.frame(frame.encode()) {
                        return Ok(false);
                    }
                    request(owner, Request::State)?;
                    if !waiting {
                        status.set(core_status.0.clone(), core_status.1.clone());
                    }
                }
                Ok(Some(Message::Close(_))) => return Ok(false),
                Ok(_) => {}
                Err(_) => return Ok(false),
            }
        }
        Ok(false)
    })();
    socket.close();
    drop(attached);
    trace("socket_detached");
    let _ = request(owner, Request::State);
    match result {
        // Any pause (a refusal, an outgrown room, full attachments) ends this connection for good.
        Err(HttpFailure::Paused(pause)) => {
            status.paused(pause.kind, pause.reason);
            Ok(true)
        }
        result => result,
    }
}

/// Versions use the seed route's canonical peer encoding.
pub(crate) fn compact(key: &RoomKey, base: &pop_core::sync::Base) -> std::result::Result<(), HttpFailure> {
    let version =
        |vv: &loro::VersionVector| pop_sharing_protocol::frame::version_json(vv.iter().map(|(&p, &n)| (p, n as u32)));
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
            ("X-Sharing-Root", &version(&base.start)),
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};

    #[test]
    fn local_owner_failures_preserve_recovery_and_stop_uncertain_replays() {
        use pop_core::Code;
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
            owner_failure(attachment, true),
            HttpFailure::Paused(pop_core::sync::Pause { kind: PauseKind::FilesFull, .. })
        ));
    }

    fn upgrade_refusal(headers: &str, body: &[u8], status: u16) -> HttpFailure {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
            authorization: Authorization::Capability("b".repeat(64)),
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
    fn local_connection(files_full: bool, close: CloseCode) -> (bool, Phase, Option<PauseKind>) {
        let (_dir, owner) = if files_full { crate::tests::legacy_fixture() } else { crate::tests::fixture() };
        request(
            &owner,
            Request::Share {
                share: Share {
                    room: owner.app().document_uuid.clone().unwrap(),
                    endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into(),
                },
            },
        )
        .unwrap();
        let store = Store::open(owner.path(), Mode::Snapshot).unwrap();
        let checkpoint = store.document().unwrap().checkpoint().unwrap();
        let vv = loro::LoroDoc::decode_import_blob_meta(&checkpoint, false).unwrap().partial_end_vv;
        let version: pop_core::sync::Version = vv.iter().map(|(&p, &n)| (p, n as u32)).collect();
        if files_full {
            request(&owner, Request::PutAttachment { bytes: b"file".to_vec() }).unwrap();
            request(
                &owner,
                Request::Apply {
                    batch: serde_json::from_value(
                        serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}),
                    )
                    .unwrap(),
                    origin: pop_core::Origin::Page,
                },
            )
            .unwrap();
        }
        let info = serde_json::to_vec(&RoomInfo {
            document: owner.app().document_uuid.clone().unwrap(),
            phase: RoomPhase::Live,
            seed_hash: None,
            seed_size: None,
            app: None,
            capability: None,
            compact: false,
        })
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let key = RoomKey {
            room: owner.app().document_uuid.clone().unwrap(),
            authorization: Authorization::Capability("b".repeat(64)),
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
                        for frame in
                            [Frame::Welcome { permission: pop_core::sync::Permission::Write, version }, Frame::CaughtUp]
                        {
                            ws.send(Message::Binary(envelope::encode(frame).unwrap().into())).unwrap();
                        }
                        ws.send(Message::Text("pong".into())).unwrap();
                        ws.send(Message::Pong(vec![1, 2].into())).unwrap();
                        if !files_full {
                            if close == CloseCode::Error {
                                assert_eq!(
                                    ws.read().unwrap(),
                                    Message::Text("ping".into()),
                                    "idle connections keep the relay socket alive"
                                );
                                ws.send(Message::Text("pong".into())).unwrap();
                            }
                            ws.close(Some(CloseFrame { code: close, reason: "retry".into() })).unwrap();
                        }
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
                            (200, &b"[]"[..])
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
        let status = Status { value: Mutex::new((Phase::Offline, String::new(), None)), notice: Arc::default() };
        let result =
            connected(&owner, &key, &stop, &status, &mut false, &crate::tests::test_access(&owner, &key, true));
        stop.store(true, Ordering::Relaxed);
        serving.join().unwrap();
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
        let (phase, _, kind) = status.value.into_inner().unwrap();
        (result.unwrap_or_else(|e| panic!("{}", String::from(e))), phase, kind)
    }
    #[test]
    fn keepalive_pong_and_transient_closes_reconnect_without_pausing() {
        for code in [CloseCode::Error, CloseCode::Again] {
            let (terminal, phase, kind) = local_connection(false, code);
            assert!(!terminal);
            assert_ne!(phase, Phase::Paused);
            assert_eq!(kind, None);
        }
    }
    #[test]
    fn legacy_attachment_capacity_is_a_terminal_files_full_pause() {
        let (terminal, phase, kind) = local_connection(true, CloseCode::Normal);
        assert!(terminal);
        assert_eq!(phase, Phase::Paused);
        assert_eq!(kind, Some(PauseKind::FilesFull));
    }

    /// Answer the socket upgrade with `answer` (or hang up) and report what `connected` made of it.
    fn handshake(
        answer: Option<(u16, &'static str)>,
        identity: bool,
    ) -> (std::result::Result<bool, HttpFailure>, Phase, Option<PauseKind>) {
        let (_dir, owner) = crate::tests::fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
            authorization: if identity {
                Authorization::Identity("identity-token".into())
            } else {
                Authorization::Capability("b".repeat(64))
            },
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
        let status = Status { value: Mutex::new((Phase::Offline, String::new(), None)), notice: Arc::default() };
        let result = connected(
            &owner,
            &key,
            &AtomicBool::new(false),
            &status,
            &mut false,
            &crate::tests::test_access(&owner, &key, true),
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
            let (result, phase, paused) = handshake(Some((code, body)), true);
            assert!(matches!(result, Ok(true)), "{code} is terminal: {result:?}");
            assert_eq!((phase, paused), (Phase::Paused, Some(kind)), "{code}");
        }
        let (result, phase, paused) = handshake(Some((403, "")), false);
        assert!(matches!(result, Ok(true)));
        assert_eq!((phase, paused), (Phase::Ended, None));
        let (result, phase, paused) = handshake(Some((410, "")), false);
        assert!(matches!(result, Ok(true)));
        assert_eq!((phase, paused), (Phase::Ended, None));
    }
    #[test]
    fn transient_handshake_failures_stay_retryable() {
        for answer in [Some((503, "")), Some((429, "")), None] {
            let (result, phase, paused) = handshake(answer, false);
            assert!(matches!(result, Err(HttpFailure::Other(_))), "{answer:?} retries: {result:?}");
            assert_ne!(phase, Phase::Paused);
            assert_eq!(paused, None);
        }
    }
}
