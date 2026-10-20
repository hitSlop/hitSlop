//! Single native Rust socket supervisor. Swift only starts/stops it and reads status.
use super::*;
use hitslop_core::{owner::SyncEvent, sync::MAX_FRAME};
use hitslop_sharing_protocol::{envelope, frame::Frame};
use std::{
    collections::VecDeque,
    net::{TcpStream, ToSocketAddrs},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
use tungstenite::{Message, client::IntoClientRequest, stream::MaybeTlsStream};

fn trace(event: &str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var("HITSLOP_SHARING_TRACE").is_ok_and(|v| v == "1")) {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        eprintln!("hitslop.sharing time={time} event={event}");
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
        hitslop_sharing_protocol::SEED_BYTES as u64
    } else if route.starts_with("blob/") {
        hitslop_core::ATTACHMENT_FILE_BYTES as u64
    } else {
        hitslop_sharing_protocol::CONTROL_BYTES as u64
    };
    let suffix = if route.is_empty() { String::new() } else { format!("/{route}") };
    http_request_typed(method, &format!("{}/v1/rooms/{}{suffix}", key.origin, key.room), token, body, headers, limit)
}
pub(crate) enum HttpFailure {
    Ended,
    Other(String),
}
impl From<HttpFailure> for String {
    fn from(value: HttpFailure) -> Self {
        match value {
            HttpFailure::Ended => "Sharing ended".into(),
            HttpFailure::Other(message) => message,
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
    match response.status().as_u16() {
        200 => {}
        401 => return Err("Sign in to your sharing account first".into()),
        410 if method == "DELETE" => return Ok(Vec::new()),
        410 => return Err(HttpFailure::Ended),
        409 => return Err("Sharing changed or creation is unfinished. Retry sharing.".into()),
        status => return Err(HttpFailure::Other(format!("Sharing service refused this request ({status})"))),
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
        if self.events.len() >= hitslop_sharing_protocol::QUEUED_FRAMES
            || self.bytes.saturating_add(bytes) > hitslop_sharing_protocol::QUEUED_BYTES
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
    value: Mutex<(Phase, String)>,
    notice: Notice,
}
impl Status {
    fn set(&self, phase: Phase, detail: String) {
        let changed = {
            let mut old = self.value.lock().unwrap();
            let value = (phase, detail);
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
        let status = Arc::new(Status { value: Mutex::new((Phase::Offline, "Connecting".into())), notice });
        let (running, output) = (stop.clone(), status.clone());
        let thread = thread::spawn(move || {
            let mut backoff = Duration::from_millis(250);
            let mut bootstrapped = false;
            while !running.load(Ordering::Relaxed) {
                let attempt = (|| {
                    if !bootstrapped {
                        trace("bootstrap_start");
                        bootstrap(&owner, &key)?;
                        bootstrapped = true;
                        trace("bootstrap_done");
                    }
                    connected(&owner, &key, &running, &output).map_err(HttpFailure::Other)
                })();
                match attempt {
                    Ok(true) => break,
                    Ok(false) => output.set(Phase::Offline, "Offline; reconnecting".into()),
                    Err(HttpFailure::Ended) => {
                        output.set(Phase::Ended, "Ended".into());
                        break;
                    }
                    Err(HttpFailure::Other(error)) => {
                        output.set(Phase::Offline, format!("Offline; reconnecting: {error}"))
                    }
                }
                let until = Instant::now() + backoff;
                while Instant::now() < until && !running.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(25));
                }
                backoff = (backoff * 2).min(Duration::from_secs(8));
            }
            trace("supervisor_stopped");
            owner.sync_detach();
            let _ = request(&owner, Request::State);
        });
        Self { stop, status, thread: Some(thread) }
    }
    pub fn status(&self) -> (Phase, String) {
        self.status.value.lock().unwrap().clone()
    }
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn connected(owner: &Arc<Owner>, key: &RoomKey, stop: &AtomicBool, status: &Status) -> Result<bool> {
    let mut req = format!("{}/v1/rooms/{}/sync", key.origin.replacen("https://", "wss://", 1), key.room)
        .into_client_request()
        .map_err(fail)?;
    req.headers_mut()
        .insert("Sec-WebSocket-Protocol", hitslop_sharing_protocol::SOCKET_PROTOCOL.parse().map_err(fail)?);
    req.headers_mut().insert("Authorization", format!("Bearer {}", key.capability).parse().map_err(fail)?);
    let host = req.uri().host().ok_or("Invalid relay host")?.to_owned();
    let addresses = (host.as_str(), 443).to_socket_addrs().map_err(|_| fail("Relay address unavailable"))?;
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
    stream.set_read_timeout(Some(Duration::from_secs(10))).map_err(fail)?;
    stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(fail)?;
    let (mut socket, _) = match tungstenite::client_tls_with_config(req, stream, None, None) {
        Ok(socket) => socket,
        Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response)))
            if response.status().as_u16() == 410 =>
        {
            status.set(Phase::Ended, "Ended".into());
            return Ok(true);
        }
        Err(_) => return Err("Relay socket unavailable".into()),
    };
    let stream: &TcpStream = match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_ref(),
        _ => return Err(fail("Unsupported TLS transport")),
    };
    trace("socket_opened");
    stream.set_read_timeout(Some(Duration::from_millis(5))).map_err(fail)?;
    stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(fail)?;
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
    let result = (|| {
        trace("inventory_start");
        uploaded = resources::remote_blobs(key)?;
        trace("inventory_done");
        while !stop.load(Ordering::Relaxed) {
            let pending = std::mem::take(&mut *queue.lock().unwrap());
            if pending.overflow {
                return Err(fail("Socket queue budget exceeded"));
            }
            for event in pending.events {
                match event {
                    SyncEvent::Reconnect => return Ok(false),
                    SyncEvent::Status { status: s } => {
                        let terminal = matches!(s, hitslop_core::sync::Status::Paused { .. });
                        let phase = match &s {
                            hitslop_core::sync::Status::Offline => Phase::Offline,
                            hitslop_core::sync::Status::Syncing => Phase::Syncing,
                            hitslop_core::sync::Status::Synced => Phase::Synced,
                            hitslop_core::sync::Status::Paused { .. } => Phase::Paused,
                        };
                        core_status = (phase, format!("{s:?}"));
                        status.set(core_status.0.clone(), core_status.1.clone());
                        if terminal {
                            return Ok(true);
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
                    let frame = envelope::decode(&bytes).map_err(fail)?;
                    trace_frame(&frame, true);
                    if matches!(&frame,Frame::Error{message,..} if message=="Sharing ended") {
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
                    status.set(core_status.0.clone(), core_status.1.clone());
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
    result
}
