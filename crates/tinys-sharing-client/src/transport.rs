//! Single native Rust socket supervisor. Swift only starts/stops it and reads status.
use super::*;
use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
use tinys_core::{
    owner::SyncEvent,
    sync::{MAX_FRAME, PauseKind},
};
use tinys_sharing_protocol::{
    envelope,
    failure::{self, Outcome, Route},
    frame::Frame,
    limits,
};
use tungstenite::{Message, client::IntoClientRequest, stream::MaybeTlsStream};

/// Keep TLS ownership outside Tungstenite: a rejected upgrade consumes its borrowed
/// stream and returns only the body bytes buffered alongside the response headers.
fn socket_stream<'a>(
    stream: TcpStream,
    host: &str,
    tls: bool,
    deadline: Instant,
    stop: &'a AtomicBool,
) -> Result<MaybeTlsStream<HandshakeIo<'a>>> {
    // Apply the deadline below TLS, including its internal record reads and writes.
    let stream = HandshakeIo { stream, deadline: Some(deadline), stop };
    if !tls {
        return Ok(MaybeTlsStream::Plain(stream));
    }
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(fail)?;
    let connection = rustls::ClientConnection::new(Arc::new(config), name).map_err(fail)?;
    Ok(MaybeTlsStream::Rustls(rustls::StreamOwned::new(connection, stream)))
}

// Tungstenite drops its stream on a refused upgrade. Borrow through a cell so the
// error path can finish reading it, while successful sockets keep the same TLS state.
struct StreamRef<'a, S>(&'a RefCell<S>);
impl<S: Read> Read for StreamRef<'_, S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.borrow_mut().read(bytes)
    }
}
impl<S: Write> Write for StreamRef<'_, S> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

struct HandshakeIo<'a> {
    stream: TcpStream,
    deadline: Option<Instant>,
    stop: &'a AtomicBool,
}
impl HandshakeIo<'_> {
    fn remaining(&self) -> io::Result<Option<Duration>> {
        self.deadline
            .map(|deadline| {
                if self.stop.load(Ordering::Relaxed) {
                    return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Sharing stopped"));
                }
                deadline
                    .checked_duration_since(Instant::now())
                    .filter(|remaining| !remaining.is_zero())
                    .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Socket handshake timed out"))
            })
            .transpose()
    }
    fn prepare(&self) -> io::Result<()> {
        if let Some(remaining) = self.remaining()? {
            self.stream.set_read_timeout(Some(remaining))?;
            self.stream.set_write_timeout(Some(remaining))?;
        }
        Ok(())
    }
}
impl Read for HandshakeIo<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.prepare()?;
        self.stream.read(bytes)
    }
}
impl Write for HandshakeIo<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.prepare()?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.prepare()?;
        self.stream.flush()
    }
}

/// Decode a bounded HTTP rejection body, including the bytes Tungstenite read ahead.
fn refusal_body(response: &tungstenite::handshake::client::Response, stream: impl Read) -> io::Result<Vec<u8>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "Invalid socket refusal body");
    let limit = tinys_sharing_protocol::CONTROL_BYTES;
    let prefix = io::Cursor::new(response.body().as_deref().unwrap_or_default());
    // Also bound framing overhead, including chunk extensions and trailers.
    let mut reader = BufReader::new(prefix.chain(stream).take((limit * 2 + 1) as u64));
    let mut body = Vec::new();
    let headers = response.headers();
    if headers.get_all("content-length").iter().count() > 1 || headers.get_all("transfer-encoding").iter().count() > 1 {
        return Err(invalid());
    }
    if let Some(encoding) = headers.get("transfer-encoding") {
        if headers.contains_key("content-length") || !encoding.as_bytes().eq_ignore_ascii_case(b"chunked") {
            return Err(invalid());
        }
        loop {
            let mut line = Vec::new();
            reader.read_until(b'\n', &mut line)?;
            let httparse::Status::Complete((used, size)) = httparse::parse_chunk_size(&line).map_err(|_| invalid())?
            else {
                return Err(invalid());
            };
            if used != line.len() || size > (limit - body.len()) as u64 {
                return Err(invalid());
            }
            if size == 0 {
                let mut trailers = Vec::new();
                loop {
                    let start = trailers.len();
                    if reader.read_until(b'\n', &mut trailers)? == 0 {
                        return Err(invalid());
                    }
                    if &trailers[start..] == b"\r\n" {
                        break;
                    }
                }
                let mut fields = [httparse::EMPTY_HEADER; 64];
                if !httparse::parse_headers(&trailers, &mut fields).map_err(|_| invalid())?.is_complete() {
                    return Err(invalid());
                }
                break;
            }
            let start = body.len();
            body.resize(start + size as usize, 0);
            reader.read_exact(&mut body[start..])?;
            let mut end = [0; 2];
            reader.read_exact(&mut end)?;
            if end != *b"\r\n" {
                return Err(invalid());
            }
        }
    } else if let Some(length) = headers.get("content-length") {
        let length: usize = length.to_str().map_err(|_| invalid())?.parse().map_err(|_| invalid())?;
        if length > limit {
            return Err(invalid());
        }
        body.resize(length, 0);
        reader.read_exact(&mut body)?;
    } else {
        reader.take((limit + 1) as u64).read_to_end(&mut body)?;
        if body.len() > limit {
            return Err(invalid());
        }
    }
    Ok(body)
}

fn upgrade<S: Read + Write>(
    request: tungstenite::handshake::client::Request,
    stream: &RefCell<S>,
    route: Route,
) -> std::result::Result<tungstenite::WebSocket<StreamRef<'_, S>>, HttpFailure> {
    match tungstenite::client(request, StreamRef(stream)) {
        Ok((socket, _)) => Ok(socket),
        Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response))) => {
            let body = refusal_body(&response, StreamRef(stream)).unwrap_or_default();
            Err(refusal(response.status().as_u16(), &body, route))
        }
        Err(_) => Err("Relay socket unavailable".into()),
    }
}

fn trace(event: &str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var("TINYS_SHARING_TRACE").is_ok_and(|v| v == "1")) {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        eprintln!("tinys.sharing time={time} event={event}");
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
    let limit = if route == "seed" {
        tinys_sharing_protocol::SEED_BYTES as u64
    } else if route.starts_with("blob/") {
        tinys_core::ATTACHMENT_FILE_BYTES as u64
    } else {
        tinys_sharing_protocol::CONTROL_BYTES as u64
    };
    let suffix = if route.is_empty() { String::new() } else { format!("/{route}") };
    http_request_typed(method, &format!("{}/v1/rooms/{}{suffix}", key.origin, key.room), token, body, headers, limit)
}
#[derive(Debug)]
pub(crate) enum HttpFailure {
    Ended,
    /// Another writer got there first, or creation is unfinished.
    Conflict,
    Paused(tinys_core::sync::Pause),
    Other(String),
}
impl From<tinys_core::owner::Failure> for HttpFailure {
    fn from(error: tinys_core::owner::Failure) -> Self {
        match error.reason {
            Some(tinys_core::Code::RequiresUpdate) => {
                Self::Paused(tinys_core::sync::Pause { kind: PauseKind::NeedsUpdate, reason: error.message })
            }
            Some(_) => Self::Paused(tinys_core::sync::Pause { kind: PauseKind::Refused, reason: error.message }),
            None => Self::Other(error.message),
        }
    }
}
impl From<tinys_core::store::Error> for HttpFailure {
    fn from(error: tinys_core::store::Error) -> Self {
        tinys_core::owner::Failure::from(error).into()
    }
}
impl From<HttpFailure> for String {
    fn from(value: HttpFailure) -> Self {
        match value {
            HttpFailure::Ended => "Sharing ended".into(),
            HttpFailure::Other(message) => message,
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
    let body = serde_json::from_slice::<tinys_sharing_protocol::http::ApiFailure>(body).ok();
    let paused = |kind, reason| HttpFailure::Paused(tinys_core::sync::Pause { kind, reason });
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
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(30))).build(),
    );
    let mut request = ureq::http::Request::builder().method(method).uri(url);
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response =
        agent.run(request.body(body.to_vec()).map_err(fail)?).map_err(|_| fail("Sharing service is unavailable"))?;
    let status = response.status().as_u16();
    if !matches!(status, 200 | 204) {
        let bytes = response
            .body_mut()
            .with_config()
            .limit(tinys_sharing_protocol::CONTROL_BYTES as u64)
            .read_to_vec()
            .unwrap_or_default();
        let failure = refusal(status, &bytes, Route::of(url));
        return match failure {
            HttpFailure::Ended if method == "DELETE" => Ok(Vec::new()),
            failure => Err(failure),
        };
    }
    let bytes =
        response.body_mut().with_config().limit(limit + 1).read_to_vec().map_err(|_| fail("Relay body unavailable"))?;
    if bytes.len() as u64 > limit {
        return Err(fail("Relay body exceeds its byte limit").into());
    }
    Ok(bytes)
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
        if self.events.len() >= tinys_sharing_protocol::QUEUED_FRAMES
            || self.bytes.saturating_add(bytes) > tinys_sharing_protocol::QUEUED_BYTES
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
    status: Arc<Status>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Connection {
    pub fn start(owner: Arc<Owner>, key: RoomKey, notice: Notice) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Status { value: Mutex::new((Phase::Offline, "Connecting".into(), None)), notice });
        let (running, output) = (stop.clone(), status.clone());
        let thread = thread::spawn(move || {
            let mut backoff = Duration::from_millis(limits::BACKOFF_MIN_MS);
            let mut bootstrapped = false;
            let mut retried_full = false;
            while !running.load(Ordering::Relaxed) {
                let attempt = (|| {
                    if !bootstrapped {
                        trace("bootstrap_start");
                        bootstrap(&owner, &key)?;
                        bootstrapped = true;
                        trace("bootstrap_done");
                    }
                    connected(&owner, &key, &running, &output, &mut retried_full)
                })();
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
            owner.sync_detach();
            let _ = request(&owner, Request::State);
        });
        Self { stop, status, thread: Some(thread) }
    }
    pub fn status(&self) -> (Phase, String, Option<PauseKind>) {
        self.status.value.lock().unwrap().clone()
    }
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn connected(
    owner: &Arc<Owner>,
    key: &RoomKey,
    stop: &AtomicBool,
    status: &Status,
    retried_full: &mut bool,
) -> std::result::Result<bool, HttpFailure> {
    let mut req = format!(
        "{}/v1/rooms/{}/sync",
        key.origin.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1),
        key.room
    )
    .into_client_request()
    .map_err(fail)?;
    req.headers_mut().insert("Sec-WebSocket-Protocol", tinys_sharing_protocol::SOCKET_PROTOCOL.parse().map_err(fail)?);
    req.headers_mut().insert("Authorization", format!("Bearer {}", key.capability).parse().map_err(fail)?);
    let route = Route::of(&req.uri().to_string());
    let host = req.uri().host().ok_or("Invalid relay host")?.to_owned();
    let addresses =
        (host.as_str(), req.uri().port_u16().unwrap_or(if req.uri().scheme_str() == Some("ws") { 80 } else { 443 }))
            .to_socket_addrs()
            .map_err(|_| fail("Relay address unavailable"))?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = None;
    for address in addresses {
        if stop.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let Some(timeout) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        if let Ok(connected) = TcpStream::connect_timeout(&address, timeout) {
            stream = Some(connected);
            break;
        }
    }
    let stream = stream.ok_or("Relay socket unavailable")?;
    let stream = RefCell::new(socket_stream(stream, &host, req.uri().scheme_str() == Some("wss"), deadline, stop)?);
    let mut socket = match upgrade(req, &stream, route) {
        Ok(socket) => socket,
        // The relay answered the upgrade: permanent refusals and an ended room stop here.
        Err(error) => {
            match error {
                HttpFailure::Ended => status.set(Phase::Ended, "Ended".into()),
                HttpFailure::Paused(pause) => status.paused(pause.kind, pause.reason),
                _ => return Err("Relay socket unavailable".into()),
            }
            return Ok(true);
        }
    };
    trace("socket_opened");
    {
        let mut stream = stream.borrow_mut();
        let stream = match &mut *stream {
            MaybeTlsStream::Plain(stream) => stream,
            MaybeTlsStream::Rustls(stream) => stream.get_mut(),
            _ => unreachable!("socket_stream only creates plain or Rustls streams"),
        };
        stream.deadline = None;
        stream.stream.set_read_timeout(Some(Duration::from_millis(5))).map_err(fail)?;
        stream.stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(fail)?;
    }
    socket.set_config(|c| {
        c.max_message_size = Some(MAX_FRAME);
        c.max_frame_size = Some(MAX_FRAME);
    });
    let queue = Arc::new(Mutex::new(Queue::default()));
    let sink = queue.clone();
    let admission = owner.sync_connection(Arc::new(move |e| sink.lock().unwrap().enqueue(e)));
    request(owner, Request::State)?;
    trace("owner_attached");
    let mut core_status = (Phase::Syncing, "Syncing".to_owned());
    let mut uploaded = Vec::new();
    let mut check_at = Instant::now();
    let mut compact_at = None;
    let mut waiting = false;

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
                if waiting && !info.compact && *retried_full {
                    status.paused(PauseKind::Outgrown, "This document has outgrown sharing".into());
                    return Ok(true);
                }
                if info.compact || waiting {
                    let mut random = [0u8; 2];
                    getrandom::getrandom(&mut random).map_err(|_| fail("Cannot schedule compaction"))?;
                    compact_at = Some(
                        now + Duration::from_millis(
                            u16::from_le_bytes(random) as u64 * limits::COMPACT_JITTER_MS / 65_535,
                        ),
                    );
                }
            }
            if compact_at.is_some_and(|at| now >= at) {
                // Recheck after jitter: another writer may already have compacted.
                let info = room_info(key, None)?;
                if !info.compact {
                    compact_at = None;
                    if waiting {
                        *retried_full = true;
                        return Ok(false);
                    }
                } else {
                    let (send, receive) = std::sync::mpsc::channel();
                    admission.base(move |base| {
                        let _ = send.send(base);
                    });
                    match receive.recv_timeout(Duration::from_secs(30)).map_err(|_| fail("Owner unavailable"))? {
                        Err(pause) => {
                            status.paused(pause.kind, pause.reason);
                            return Ok(true);
                        }
                        Ok(None) => {
                            // Not eligible yet (edits in flight); each retry re-reads RoomInfo,
                            // so stay well inside the guest metadata budget.
                            compact_at = Some(now + Duration::from_millis(limits::COMPACT_RETRY_MS));
                        }
                        Ok(Some(base)) => {
                            compact_at = None;
                            match compact(key, &base) {
                                Ok(()) | Err(HttpFailure::Conflict) => {
                                    if waiting {
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
                        if let tinys_core::sync::Status::Paused { kind, reason } = &s {
                            status.paused(*kind, reason.clone());
                            if *kind == PauseKind::RoomFull {
                                waiting = true;
                                check_at = Instant::now();
                                continue;
                            }
                            return Ok(true);
                        }
                        if s == tinys_core::sync::Status::Synced {
                            *retried_full = false;
                        }
                        let phase = match &s {
                            tinys_core::sync::Status::Offline => Phase::Offline,
                            tinys_core::sync::Status::Syncing => Phase::Syncing,
                            tinys_core::sync::Status::Synced => Phase::Synced,
                            tinys_core::sync::Status::Paused { .. } => Phase::Paused,
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
                            resources::upload_blobs(owner, key, &mut uploaded)?;
                            trace("upload_blobs_done");
                        }
                        let message = envelope::encode(frame).map_err(fail)?;
                        socket.send(Message::Binary(message.into())).map_err(|_| fail("Socket send failed"))?;
                    }
                }
            }
            match socket.read() {
                Ok(Message::Binary(bytes)) => {
                    let frame = envelope::decode(&bytes).map_err(|e| {
                        HttpFailure::Paused(tinys_core::sync::Pause { kind: PauseKind::Refused, reason: fail(e) })
                    })?;
                    trace_frame(&frame, true);
                    if matches!(&frame,Frame::Error{code,..} if *code == tinys_sharing_protocol::frame::ERROR_ENDED) {
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
                Ok(Message::Close(_)) => return Ok(false),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => return Ok(false),
            }
        }
        Ok(false)
    })();
    let _ = socket.close(None);
    let _ = socket.flush();
    admission.detach();
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
pub(crate) fn compact(key: &RoomKey, base: &tinys_core::sync::Base) -> std::result::Result<(), HttpFailure> {
    let version =
        |vv: &loro::VersionVector| tinys_sharing_protocol::frame::version_json(vv.iter().map(|(&p, &n)| (p, n as u32)));
    http_typed(
        key,
        "POST",
        "compact",
        Some(&key.capability),
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

    /// Each read exposes one network fragment, regardless of the caller's buffer size.
    struct Fragmented {
        parts: VecDeque<std::io::Cursor<Vec<u8>>>,
    }
    impl Read for Fragmented {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            while let Some(part) = self.parts.front_mut() {
                let size = part.read(output)?;
                if size != 0 {
                    return Ok(size);
                }
                self.parts.pop_front();
            }
            Ok(0)
        }
    }
    impl Write for Fragmented {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn a_fragmented_upgrade_refusal_keeps_its_update_reason() {
        let body = br#"{"code":"requires_update","message":"Update tinys to keep sharing"}"#;
        for buffered in [0, 12, body.len()] {
            let mut header = format!("HTTP/1.1 400 Reply\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
            header.extend_from_slice(&body[..buffered]);
            let stream = RefCell::new(Fragmented {
                parts: [header, body[buffered..].to_vec()].into_iter().map(io::Cursor::new).collect(),
            });
            let request = "ws://localhost/v1/rooms/test/sync".into_client_request().unwrap();
            let Err(HttpFailure::Paused(pause)) = upgrade(request, &stream, Route::Other) else {
                panic!("a permanent refusal must pause");
            };
            assert_eq!(pause.kind, PauseKind::NeedsUpdate, "{buffered} body bytes arrived with the headers");
            assert_eq!(pause.reason, "Update tinys to keep sharing");
        }
    }

    fn fragmented_refusal(headers: &str, body: &[u8], status: u16) -> HttpFailure {
        let stream = RefCell::new(Fragmented {
            parts: std::iter::once(format!("HTTP/1.1 {status} Reply\r\n{headers}\r\n").into_bytes())
                .chain(body.chunks(if body.len() > 4096 { 4096 } else { 3 }).map(<[u8]>::to_vec))
                .map(io::Cursor::new)
                .collect(),
        });
        let request = "ws://localhost/v1/rooms/test/sync".into_client_request().unwrap();
        match upgrade(request, &stream, Route::Other) {
            Err(error) => error,
            Ok(_) => panic!("the server refused the upgrade"),
        }
    }

    #[test]
    fn upgrade_refusals_decode_chunked_and_connection_closed_bodies() {
        let body = br#"{"code":"requires_update","message":"Update tinys to keep sharing"}"#;
        let chunked = format!(
            "d;extension=yes\r\n{}\r\n{:x}\r\n{}\r\n0\r\nX-Trailer: ignored\r\n\r\n",
            std::str::from_utf8(&body[..13]).unwrap(),
            body.len() - 13,
            std::str::from_utf8(&body[13..]).unwrap()
        );
        for (headers, bytes) in [("", body.as_slice()), ("Transfer-Encoding: chunked\r\n", chunked.as_bytes())] {
            let HttpFailure::Paused(pause) = fragmented_refusal(headers, bytes, 400) else { panic!("must pause") };
            assert_eq!(pause.kind, PauseKind::NeedsUpdate);
        }
    }

    #[test]
    fn invalid_upgrade_bodies_keep_the_status_based_refusal() {
        let limit = tinys_sharing_protocol::CONTROL_BYTES;
        for (headers, body) in [
            ("Content-Length: 20\r\n".into(), b"short".to_vec()),
            ("Content-Length: 0\r\n".into(), Vec::new()),
            ("Content-Length: 3\r\n".into(), b"bad".to_vec()),
            (format!("Content-Length: {}\r\n", limit + 1), Vec::new()),
            ("".into(), vec![b'x'; limit + 1]),
            ("Transfer-Encoding: chunked\r\n".into(), b"5\r\nshort\r\n".to_vec()),
            ("Transfer-Encoding: chunked\r\n".into(), format!("{:x}\r\n", limit + 1).into_bytes()),
            ("Transfer-Encoding: chunked\r\n".into(), b"invalid\r\n".to_vec()),
            ("Transfer-Encoding: chunked\r\n".into(), b"0\r\n".to_vec()),
            ("Content-Length: 1\r\nContent-Length: 2\r\n".into(), b"{}".to_vec()),
            ("Transfer-Encoding: chunked\r\nContent-Length: 0\r\n".into(), Vec::new()),
        ] {
            let HttpFailure::Paused(pause) = fragmented_refusal(&headers, &body, 400) else { panic!("must pause") };
            assert_eq!(pause.kind, PauseKind::Refused, "{headers}");
        }
        for status in [429, 503] {
            assert!(matches!(fragmented_refusal("Content-Length: 20\r\n", b"short", status), HttpFailure::Other(_)));
        }
    }

    #[test]
    fn refusal_body_reads_obey_the_handshake_deadline_and_stop() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_peer, _) = listener.accept().unwrap();
        let stop = AtomicBool::new(false);
        let mut stream =
            HandshakeIo { stream, deadline: Some(Instant::now() + Duration::from_millis(30)), stop: &stop };
        let response =
            tungstenite::http::Response::builder().status(400).header("Content-Length", "1").body(None).unwrap();
        let started = Instant::now();
        assert!(refusal_body(&response, &mut stream).is_err());
        assert!(started.elapsed() < Duration::from_secs(2), "a stalled body exceeded its handshake deadline");
        stream.deadline = Some(Instant::now() + Duration::from_secs(10));
        stop.store(true, Ordering::Relaxed);
        assert_eq!(refusal_body(&response, &mut stream).unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
    }

    /// Exercise the actual socket loop and HTTP attachment barrier over loopback.
    #[allow(clippy::result_large_err, reason = "tungstenite fixes the handshake callback error type")]
    fn local_connection(files_full: bool, close: CloseCode) -> (bool, Phase, Option<PauseKind>) {
        let (_dir, owner) = crate::tests::fixture();
        request(
            &owner,
            Request::Share {
                share: Share { room: "a".repeat(32), endpoint: tinys_sharing_protocol::TRUSTED_ORIGIN.into() },
            },
        )
        .unwrap();
        let store = Store::open(owner.path(), Mode::Snapshot).unwrap();
        let checkpoint = store.document().unwrap().checkpoint().unwrap();
        let vv = loro::LoroDoc::decode_import_blob_meta(&checkpoint, false).unwrap().partial_end_vv;
        let version: tinys_core::sync::Version = vv.iter().map(|(&p, &n)| (p, n as u32)).collect();
        if files_full {
            request(&owner, Request::PutAttachment { bytes: b"file".to_vec() }).unwrap();
            request(
                &owner,
                Request::Apply {
                    batch: serde_json::from_value(
                        serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}),
                    )
                    .unwrap(),
                    origin: tinys_core::Origin::Page,
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
            room: "a".repeat(32),
            capability: "b".repeat(64),
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
                        for frame in [
                            Frame::Welcome { permission: tinys_core::sync::Permission::Write, version },
                            Frame::CaughtUp,
                        ] {
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
        let result = connected(&owner, &key, &stop, &status, &mut false);
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
    fn attachment_capacity_is_a_terminal_files_full_pause() {
        let (terminal, phase, kind) = local_connection(true, CloseCode::Normal);
        assert!(terminal);
        assert_eq!(phase, Phase::Paused);
        assert_eq!(kind, Some(PauseKind::FilesFull));
    }

    /// Answer the socket upgrade with `answer` (or hang up) and report what `connected` made of it.
    fn handshake(
        answer: Option<(u16, &'static str)>,
    ) -> (std::result::Result<bool, HttpFailure>, Phase, Option<PauseKind>) {
        let (_dir, owner) = crate::tests::fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: "a".repeat(32),
            capability: "b".repeat(64),
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
            assert!(String::from_utf8(header).unwrap().starts_with(&format!("GET /v1/rooms/{}/sync ", "a".repeat(32))));
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
        let result = connected(&owner, &key, &AtomicBool::new(false), &status, &mut false);
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
            (400, r#"{"code":"requires_update","message":"Update tinys to keep sharing"}"#, PauseKind::NeedsUpdate),
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
