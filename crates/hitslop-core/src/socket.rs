//! One bounded newline-JSON exchange per Unix connection. The writer lock remains the
//! authority; the listener owns only its socket and the owner's discovery publication.
use crate::{
    Code,
    command::{self, ExportHandler, Result},
    owner::{Failure, FailureKind, Owner},
};
use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::{
    fs::{MetadataExt, PermissionsExt},
    net::{UnixListener, UnixStream},
};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

fn io(error: impl std::fmt::Display) -> Failure {
    Failure::new(FailureKind::Failed, error.to_string())
}
fn configure(stream: &UnixStream) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let enabled: libc::c_int = 1;
        // SAFETY: a live socket and a pointer to the correctly sized integer option.
        if unsafe {
            libc::setsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_NOSIGPIPE,
                (&enabled as *const libc::c_int).cast(),
                std::mem::size_of_val(&enabled) as libc::socklen_t,
            )
        } != 0
        {
            return Err(io(format!("Cannot configure socket: {}", std::io::Error::last_os_error())));
        }
    }
    stream.set_nonblocking(true).map_err(io)
}
fn wait(stream: &UnixStream, events: libc::c_short, deadline: Instant) -> Result<()> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io("Socket timed out; outcome may be unknown"))?;
        let millis = remaining.as_nanos().div_ceil(1_000_000).min(i32::MAX as u128) as i32;
        let mut descriptor = libc::pollfd { fd: stream.as_raw_fd(), events, revents: 0 };
        // SAFETY: the descriptor points to one initialized pollfd; the stream keeps its
        // socket alive. Nonblocking I/O after readiness preserves the absolute deadline.
        let ready = unsafe { libc::poll(&mut descriptor, 1, millis) };
        if ready > 0 {
            return Ok(());
        }
        if ready == 0 {
            continue;
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(io(error));
        }
    }
}
fn read_line(stream: &mut UnixStream, limit: Option<usize>, deadline: Instant) -> Result<String> {
    let mut output = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        wait(stream, libc::POLLIN, deadline)?;
        let count = match stream.read(&mut chunk) {
            Err(error) if matches!(error.kind(), std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock) => {
                continue;
            }
            other => other.map_err(|error| io(format!("Socket read failed: {error}")))?,
        };
        if count == 0 {
            return Err(io("Host disconnected before its reply; outcome may be unknown"));
        }
        let delimiter = chunk[..count].iter().position(|b| *b == b'\n');
        let length = delimiter.unwrap_or(count);
        if limit.is_some_and(|limit| output.len() + length > limit) {
            return Err(Failure::rejected(Code::InvalidRequest, "Oversized socket request"));
        }
        output.extend_from_slice(&chunk[..length]);
        if delimiter.is_some() {
            return String::from_utf8(output).map_err(io);
        }
    }
}
fn write_line(stream: &mut UnixStream, input: &str, deadline: Instant) -> Result<()> {
    for bytes in [input.as_bytes(), b"\n"] {
        let mut offset = 0;
        while offset < bytes.len() {
            wait(stream, libc::POLLOUT, deadline)?;
            let count = match stream.write(&bytes[offset..]) {
                Err(error)
                    if matches!(error.kind(), std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock) =>
                {
                    continue;
                }
                other => other.map_err(|error| io(format!("Socket write failed: {error}")))?,
            };
            if count == 0 {
                return Err(io("Socket write failed; outcome may be unknown"));
            }
            offset += count;
        }
    }
    Ok(())
}
/// The helper's one exchange. Mutations are never retried here, including partial writes.
pub fn call(path: &Path, input: &str) -> Result<String> {
    if input.len() > command::MAX_REQUEST_BYTES {
        return Err(Failure::rejected(Code::InvalidRequest, "Oversized socket request"));
    }
    let mut stream =
        UnixStream::connect(path).map_err(|_| io("Live document unavailable; writer lock remains authoritative"))?;
    configure(&stream)?;
    let deadline = Instant::now() + command::CLIENT_TIMEOUT;
    write_line(&mut stream, input, deadline)?;
    read_line(&mut stream, None, deadline)
}
/// The most clients served at once; a connection past it is closed unanswered.
const CLIENTS: usize = 16;
struct Shared {
    owner: Arc<Owner>,
    exporter: Arc<dyn ExportHandler>,
    stopped: AtomicBool,
    /// Clients being served, each holding an `Admitted` place.
    clients: AtomicUsize,
}
/// A served client's place, given back when its thread ends, however it ends.
struct Admitted(Arc<Shared>);
impl Admitted {
    fn new(shared: &Arc<Shared>) -> Option<Self> {
        let free = |served: usize| (served < CLIENTS).then_some(served + 1);
        shared.clients.fetch_update(Ordering::AcqRel, Ordering::Acquire, free).ok()?;
        Some(Self(shared.clone()))
    }
}
impl Drop for Admitted {
    fn drop(&mut self) {
        self.0.clients.fetch_sub(1, Ordering::AcqRel);
    }
}
pub struct Server {
    path: PathBuf,
    shared: Arc<Shared>,
}
impl Server {
    /// Starts the listener and publishes discovery only once it can answer.
    pub fn start(owner: Arc<Owner>, exporter: Arc<dyn ExportHandler>) -> Result<Self> {
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        let folder = PathBuf::from(format!("/tmp/hitslop-{uid}"));
        if let Err(error) = fs::create_dir(&folder)
            && error.kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(io(error));
        }
        let info = fs::symlink_metadata(&folder).map_err(io)?;
        if !info.is_dir() || info.uid() != uid {
            return Err(io("Unsafe socket directory"));
        }
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).map_err(io)?;
        let path = folder.join(format!("{}.sock", crate::random_hex(16)));
        let listener = UnixListener::bind(&path).map_err(io)?;
        let setup = fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(io);
        if let Err(error) = setup {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        let shared =
            Arc::new(Shared { owner, exporter, stopped: AtomicBool::new(false), clients: AtomicUsize::new(0) });
        let worker = shared.clone();
        let server = Self { path, shared };
        std::thread::Builder::new()
            .name("hitslop.socket".into())
            .spawn(move || {
                while !worker.stopped.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if worker.stopped.load(Ordering::Acquire) {
                                break;
                            }
                            accept(&worker, stream);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            })
            .map_err(io)?;
        server.publish()?;
        Ok(server)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn publish(&self) -> Result<()> {
        if self.shared.stopped.load(Ordering::Acquire) {
            return Err(Failure::new(FailureKind::Closed, "Socket server is stopped"));
        }
        self.shared.owner.publish_discovery(
            &serde_json::json!({"socket":self.path,"documentPath":self.shared.owner.path()}).to_string(),
        )
    }
    pub fn withdraw(&self) {
        self.shared.owner.withdraw_discovery();
    }
    pub fn stop(&self) {
        if self.shared.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        self.withdraw();
        // Wake the parked accept thread; it observes `stopped` before dispatching.
        let _ = UnixStream::connect(&self.path);
        // Admitted exports render an independent saved copy. Closing the editor stops
        // admission but lets those replies finish without keeping the writer lock.
        let _ = fs::remove_file(&self.path);
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}
fn accept(shared: &Arc<Shared>, mut stream: UnixStream) {
    if configure(&stream).is_err() || shared.stopped.load(Ordering::Acquire) {
        return;
    }
    let Some(admitted) = Admitted::new(shared) else { return };
    // The place moves into the thread, and a spawn that fails drops it here.
    let _ = std::thread::Builder::new().name("hitslop.socket.client".into()).spawn(move || {
        let worker = &admitted.0;
        let input = read_line(&mut stream, Some(command::MAX_REQUEST_BYTES), Instant::now() + Duration::from_secs(10));
        if let Ok(input) = input {
            let deadline = Instant::now() + command::COMMAND_TIMEOUT;
            let response = if worker.stopped.load(Ordering::Acquire) {
                command::failure(Failure::new(FailureKind::Closing, "Socket server is closing"), false, false)
            } else {
                command::serve(&worker.owner, &input, Some(&worker.exporter), deadline)
            };
            let _ = write_line(&mut stream, &response, deadline);
        }
        let _ = stream.shutdown(Shutdown::Both);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_buffered_before_peer_close_is_readable() {
        let (mut client, mut peer) = UnixStream::pair().unwrap();
        configure(&client).unwrap();
        peer.write_all(b"{\"ok\":false}\n").unwrap();
        drop(peer);
        assert_eq!(read_line(&mut client, None, Instant::now() + Duration::from_secs(1)).unwrap(), r#"{"ok":false}"#);
    }

    #[test]
    fn idle_reads_and_backpressured_writes_obey_the_deadline() {
        let (mut client, _peer) = UnixStream::pair().unwrap();
        configure(&client).unwrap();
        let start = Instant::now();
        assert!(read_line(&mut client, None, start + Duration::from_millis(20)).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
        let payload = "x".repeat(8 * 1024 * 1024);
        let start = Instant::now();
        assert!(write_line(&mut client, &payload, start + Duration::from_millis(20)).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
