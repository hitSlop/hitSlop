//! Bounded preview pipes. Only this thread touches stdin/stdout; owner callbacks never wait.
use serde_json::Value;
use std::os::fd::RawFd;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_MILLIS: i32 = 50;

struct Nonblocking {
    fd: RawFd,
    original: libc::c_int,
}
impl Nonblocking {
    fn new(fd: RawFd) -> Result<Self, String> {
        // SAFETY: the process's standard stream remains open throughout serve().
        let original = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if original == -1 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let stream = Self { fd, original };
        // SAFETY: F_SETFL takes an integer flag set, with all original flags preserved.
        if unsafe { libc::fcntl(fd, libc::F_SETFL, original | libc::O_NONBLOCK) } == -1 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(stream)
    }
}
impl Drop for Nonblocking {
    fn drop(&mut self) {
        // SAFETY: this borrows a standard stream; it has not been closed or replaced.
        unsafe { libc::fcntl(self.fd, libc::F_SETFL, self.original) };
    }
}

#[derive(Clone)]
pub(super) struct Output {
    send: mpsc::SyncSender<Vec<u8>>,
    failed: Arc<AtomicBool>,
}
impl Output {
    pub(super) fn send(&self, frame: Value) {
        let mut bytes = frame.to_string().into_bytes();
        bytes.push(b'\n');
        if self.send.try_send(bytes).is_err() {
            self.failed.store(true, Ordering::Release);
        }
    }
}
struct Writing {
    bytes: Vec<u8>,
    offset: usize,
    deadline: Instant,
}
pub(super) struct Transport {
    input: Nonblocking,
    output: Nonblocking,
    receive: mpsc::Receiver<Vec<u8>>,
    failed: Arc<AtomicBool>,
    writing: Option<Writing>,
    buffer: Vec<u8>,
    scanned: usize,
    eof: bool,
}
impl Transport {
    pub(super) fn new() -> Result<(Self, Output), String> {
        let input = Nonblocking::new(libc::STDIN_FILENO)?;
        let output = Nonblocking::new(libc::STDOUT_FILENO)?;
        let (send, receive) = mpsc::sync_channel(128);
        let failed = Arc::new(AtomicBool::new(false));
        Ok((
            Self {
                input,
                output,
                receive,
                failed: failed.clone(),
                writing: None,
                buffer: vec![],
                scanned: 0,
                eof: false,
            },
            Output { send, failed },
        ))
    }
    fn begin_write(&mut self) {
        if self.writing.is_none() {
            self.writing = self.receive.try_recv().ok().map(|bytes| Writing {
                bytes,
                offset: 0,
                deadline: Instant::now() + WRITE_TIMEOUT,
            });
        }
    }
    fn pump(&mut self) -> Result<(), String> {
        if self.failed.load(Ordering::Acquire) {
            return Err("Preview output queue is full or disconnected".into());
        }
        self.begin_write();
        let Some(frame) = &mut self.writing else {
            return Ok(());
        };
        if Instant::now() >= frame.deadline {
            return Err("Preview output timed out".into());
        }
        let bytes = &frame.bytes[frame.offset..frame.bytes.len().min(frame.offset + 65536)];
        // SAFETY: the slice is readable for its length and the descriptor is open. The
        // Rust executable ignores SIGPIPE; a disconnected pipe is an ordinary I/O error.
        let written = unsafe { libc::write(self.output.fd, bytes.as_ptr().cast(), bytes.len()) };
        if written < 0 {
            let error = std::io::Error::last_os_error();
            if !matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted) {
                return Err(format!("Preview output failed: {error}"));
            }
        } else if written == 0 {
            return Err("Preview output disconnected".into());
        } else {
            frame.offset += written as usize;
            if frame.offset == frame.bytes.len() {
                self.writing = None;
            }
        }
        Ok(())
    }
    fn wait(&mut self, input: bool) -> Result<(), String> {
        // Queued frames should wake on a writable pipe, not wait for the next input
        // frame or the poll timeout after each completed output frame.
        self.begin_write();
        let mut fds = [
            libc::pollfd { fd: if input { self.input.fd } else { -1 }, events: libc::POLLIN, revents: 0 },
            libc::pollfd {
                fd: if self.writing.is_some() { self.output.fd } else { -1 },
                events: libc::POLLOUT,
                revents: 0,
            },
        ];
        // SAFETY: fds is an initialized array of exactly the length passed to poll.
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, POLL_MILLIS) } < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error.to_string());
            }
        }
        Ok(())
    }
    pub(super) fn next(&mut self) -> Result<Option<Vec<u8>>, String> {
        loop {
            self.pump()?;
            let end = self.buffer[self.scanned..].iter().position(|b| *b == b'\n').map(|n| self.scanned + n + 1);
            if end.unwrap_or(self.buffer.len()) > hitslop_core::command::MAX_REQUEST_BYTES {
                return Err("Oversized preview frame".into());
            }
            if let Some(end) = end {
                self.scanned = 0;
                return Ok(Some(self.buffer.drain(..end).collect()));
            }
            self.scanned = self.buffer.len();
            if self.eof {
                self.scanned = 0;
                return Ok((!self.buffer.is_empty()).then(|| std::mem::take(&mut self.buffer)));
            }
            self.wait(true)?;
            let mut chunk = [0u8; 8192];
            // SAFETY: chunk is writable for its length; stdin remains open and nonblocking.
            let count = unsafe { libc::read(self.input.fd, chunk.as_mut_ptr().cast(), chunk.len()) };
            if count < 0 {
                let error = std::io::Error::last_os_error();
                if !matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted) {
                    return Err(error.to_string());
                }
            } else if count == 0 {
                self.eof = true;
            } else {
                self.buffer.extend_from_slice(&chunk[..count as usize]);
            }
        }
    }
    pub(super) fn finish(&mut self, deadline: Instant) -> Result<(), String> {
        loop {
            if Instant::now() >= deadline {
                return Err("Preview shutdown timed out".into());
            }
            self.pump()?;
            if self.writing.is_none() {
                // Close has completed; only already-queued frames can remain.
                self.begin_write();
                if self.writing.is_none() {
                    return Ok(());
                }
            }
            self.wait(false)?;
        }
    }
}
