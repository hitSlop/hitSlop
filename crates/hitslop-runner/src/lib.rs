//! The evaluator runs in a new process with no inherited environment or owner handles.
//! The parent alone opens documents and routes mutations. A failed/oversized evaluation
//! cannot produce a partial edit, and the OS sandbox also contains interpreter bugs.
//!
//! The spawning client (`Evaluator`) is all a host links. The `child` feature adds the
//! evaluator itself: QuickJS, the OS sandbox and the per-ABI preludes.
#[cfg(feature = "child")]
mod child;
#[cfg(feature = "child")]
mod sandbox;
#[cfg(feature = "child")]
pub use child::child;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) const INPUT: usize = 64 << 20;
pub(crate) const OUTPUT: usize = 4 << 20;
const TIME: Duration = Duration::from_secs(3);
pub const RUNTIME_ABI: u64 = 1;
/// What the parent sends: `Input`, borrowed, so a large program is not copied per run.
#[derive(Serialize)]
struct Sent<'a> {
    #[serde(rename = "runtimeABI")]
    runtime_abi: u64,
    bundle: &'a str,
    request: &'a str,
    mode: Mode,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Mode {
    #[default]
    Command,
    Definition,
}

/// A host-configured child, never an in-process evaluator. Every call starts a
/// fresh process and QuickJS realm; the host retains all document authority.
#[derive(Clone, Debug)]
pub struct Evaluator {
    executable: PathBuf,
    arguments: Vec<String>,
}
impl Evaluator {
    pub fn new(executable: PathBuf, arguments: Vec<String>) -> Result<Self, String> {
        if !executable.is_absolute() {
            return Err("Evaluator path must be absolute".into());
        }
        Ok(Self { executable, arguments })
    }
    pub fn run(&self, runtime_abi: u64, bundle: &str, request: &str) -> Result<String, String> {
        let input = serde_json::to_vec(&Sent { runtime_abi, bundle, request, mode: Mode::Command })
            .map_err(|e| e.to_string())?;
        if input.len() > INPUT {
            return Err("Command input is too large".into());
        }
        let mut child = Command::new(&self.executable)
            .args(&self.arguments)
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut stdin = child.stdin.take().expect("piped input");
        let stdout = child.stdout.take().expect("piped output");
        no_broken_pipe_signal(&stdin);
        let writer = std::thread::spawn(move || {
            #[cfg(not(target_os = "macos"))]
            block_broken_pipe_signal();
            stdin.write_all(&input)
        });
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            let result = stdout.take(OUTPUT as u64 + 1).read_to_end(&mut output);
            let _ = sender.send((result, output));
        });
        let deadline = Instant::now() + TIME;
        let result = match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok((result, bytes)) => result.map_err(|e| e.to_string()).and_then(|_| {
                if bytes.len() > OUTPUT {
                    Err("Command output is too large".into())
                } else {
                    String::from_utf8(bytes).map_err(|e| e.to_string())
                }
            }),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("Command runner stopped without a reply".into())
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err("Command execution timed out".into()),
        };
        // Even an evaluator that wrote a valid reply cannot stay running.
        let _ = child.kill();
        let _ = child.wait();
        let written = writer.join().is_ok_and(|w| w.is_ok());
        let _ = reader.join();
        // A reply to input it never fully read is no reply.
        if !written && result.is_ok() {
            return Err("Command runner stopped before reading its input".into());
        }
        result
    }
}

/// The app process keeps SIGPIPE's default action, which ends it when a child exits before
/// reading its input. macOS refuses the signal for this pipe alone; the write fails instead.
#[cfg(target_os = "macos")]
fn no_broken_pipe_signal(stdin: &std::process::ChildStdin) {
    use std::os::fd::AsRawFd;
    const F_SETNOSIGPIPE: libc::c_int = 73; // <sys/fcntl.h>; the libc crate omits it.
    // SAFETY: sets a flag on a descriptor this function borrows.
    unsafe { libc::fcntl(stdin.as_raw_fd(), F_SETNOSIGPIPE, 1) };
}
#[cfg(not(target_os = "macos"))]
fn no_broken_pipe_signal(_: &std::process::ChildStdin) {}
/// Elsewhere a write's SIGPIPE goes to the writing thread: blocked there, it stays pending
/// until this short-lived thread exits and the write fails with EPIPE.
#[cfg(not(target_os = "macos"))]
fn block_broken_pipe_signal() {
    // SAFETY: changes only the calling thread's signal mask.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGPIPE);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}
