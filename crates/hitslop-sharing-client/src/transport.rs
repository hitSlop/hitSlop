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

pub(crate) fn http(
    key: &Credential,
    method: &str,
    route: &str,
    token: Option<&str>,
    body: &[u8],
    headers: &[(&str, &str)],
) -> Result<Vec<u8>> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(30))).build(),
    );
    let mut request =
        ureq::http::Request::builder().method(method).uri(format!("{}/rooms/{}/{}", key.origin, key.room, route));
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response =
        agent.run(request.body(body.to_vec()).map_err(fail)?).map_err(|_| fail("Sharing service is unavailable"))?;
    if response.status().as_u16() == 410 {
        return Err("Sharing ended".into());
    }
    if response.status().as_u16() != 200 {
        return Err(fail(format!("Relay HTTP status {}", response.status().as_u16())));
    }
    // ureq's limit reader errors when read_to_end probes EOF at the exact limit.
    // Reserve one byte for that probe, then enforce our own inclusive Rust bound.
    let limit = if route == "seed" {
        hitslop_sharing_protocol::SEED_BYTES as u64
    } else if route.starts_with("blob/") {
        hitslop_core::ATTACHMENT_FILE_BYTES as u64
    } else {
        hitslop_sharing_protocol::CONTROL_BYTES as u64
    };
    let bytes =
        response.body_mut().with_config().limit(limit + 1).read_to_vec().map_err(|_| fail("Relay body unavailable"))?;
    if bytes.len() as u64 > limit {
        return Err(fail("Relay body exceeds its byte limit"));
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
pub(crate) struct Connection {
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Connection {
    pub fn start(owner: Arc<Owner>, key: Credential) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new("connecting".into()));
        let (running, output) = (stop.clone(), status.clone());
        let thread = thread::spawn(move || {
            let mut backoff = Duration::from_millis(250);
            while !running.load(Ordering::Relaxed) {
                match connected(&owner, &key, &running, &output) {
                    Ok(true) => break,
                    Ok(false) => *output.lock().unwrap() = "offline; reconnecting".into(),
                    Err(error) => *output.lock().unwrap() = format!("offline; reconnecting: {error}"),
                }
                let until = Instant::now() + backoff;
                while Instant::now() < until && !running.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(25));
                }
                backoff = (backoff * 2).min(Duration::from_secs(8));
            }
            owner.sync_detach();
            let _ = request(&owner, Request::State);
        });
        Self { stop, status, thread: Some(thread) }
    }
    pub fn status(&self) -> String {
        self.status.lock().unwrap().clone()
    }
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn connected(owner: &Arc<Owner>, key: &Credential, stop: &AtomicBool, status: &Mutex<String>) -> Result<bool> {
    let mut req = format!("{}/rooms/{}/sync", key.origin.replacen("https://", "wss://", 1), key.room)
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
            *status.lock().unwrap() = "Ended".into();
            return Ok(true);
        }
        Err(_) => return Err("Relay socket unavailable".into()),
    };
    let stream: &TcpStream = match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_ref(),
        _ => return Err(fail("Unsupported TLS transport")),
    };
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
    let mut core_status = "Syncing".to_owned();
    let mut uploaded = Vec::new();
    let result = (|| {
        uploaded = resources::remote_blobs(key)?;
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
                        core_status = format!("{s:?}");
                        *status.lock().unwrap() = core_status.clone();
                        if terminal {
                            return Ok(true);
                        }
                    }
                    SyncEvent::Send { bytes } => {
                        let frame = Frame::decode(&bytes).map_err(fail)?;
                        if matches!(&frame, Frame::Push { .. }) {
                            *status.lock().unwrap() = "Syncing files".into();
                            resources::upload_blobs(owner, key, &mut uploaded)?;
                        }
                        let message = envelope::encode(frame).map_err(fail)?;
                        socket.send(Message::Binary(message.into())).map_err(|_| fail("Socket send failed"))?;
                    }
                }
            }
            match socket.read() {
                Ok(Message::Binary(bytes)) => {
                    let frame = envelope::decode(&bytes).map_err(fail)?;
                    if matches!(&frame,Frame::Error{message,..} if message=="Sharing ended") {
                        *status.lock().unwrap() = "Ended".into();
                        return Ok(true);
                    }
                    if matches!(frame, Frame::Records(_)) {
                        *status.lock().unwrap() = "Receiving files".into();
                        resources::download_blobs(owner, key, &mut uploaded)?;
                    }
                    if !admission.frame(frame.encode()) {
                        return Ok(false);
                    }
                    request(owner, Request::State)?;
                    *status.lock().unwrap() = core_status.clone();
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
    let _ = request(owner, Request::State);
    result
}
