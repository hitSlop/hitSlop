//! Native process driver for the shared evaluator.
use super::*;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
const TIME: Duration = Duration::from_millis(WATCHDOG_MS);
/// A host-configured child, never an in-process evaluator. Every call starts a
/// fresh process and QuickJS realm; the host retains all document authority.
#[derive(Clone, Debug)]
pub struct Evaluator {
    executable: PathBuf,
    arguments: Vec<String>,
}
/// Owns the process from spawn, including partially initialized pipe workers. Killing
/// before joining releases a worker blocked on a pipe when setup fails or unwinds.
struct Running {
    child: Child,
    writer: Option<std::thread::JoinHandle<std::io::Result<()>>>,
    reader: Option<std::thread::JoinHandle<()>>,
    reaped: bool,
}
impl Running {
    fn new(child: Child) -> Self {
        Self { child, writer: None, reader: None, reaped: false }
    }
    fn stop(&mut self) -> Result<(), String> {
        if !self.reaped {
            let _ = self.child.kill();
            self.child.wait().map_err(|e| format!("Cannot reap command evaluator: {e}"))?;
            self.reaped = true;
        }
        let written = self
            .writer
            .take()
            .map(|writer| {
                writer
                    .join()
                    .map_err(|_| "Command input worker panicked".to_owned())?
                    .map_err(|e| format!("Command input was not fully written: {e}"))
            })
            .unwrap_or(Ok(()));
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        written
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
impl Evaluator {
    pub fn new(executable: PathBuf, arguments: Vec<String>) -> Result<Self, String> {
        if !executable.is_absolute() {
            return Err("Evaluator path must be absolute".into());
        }
        Ok(Self { executable, arguments })
    }
    pub fn run(&self, runtime_abi: u64, bundle: &str, request: &str) -> Result<String, String> {
        let input = command_input(runtime_abi, bundle, request)?.into_bytes();
        let child = Command::new(&self.executable)
            .args(&self.arguments)
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut running = Running::new(child);
        let mut stdin = running.child.stdin.take().expect("piped input");
        let stdout = running.child.stdout.take().expect("piped output");
        no_broken_pipe_signal(&stdin).map_err(|e| format!("Cannot protect command input pipe: {e}"))?;
        running.writer = Some(
            std::thread::Builder::new()
                .name("hitslop.command.input".into())
                .spawn(move || {
                    #[cfg(not(target_os = "macos"))]
                    block_broken_pipe_signal()?;
                    stdin.write_all(&input)
                })
                .map_err(|e| format!("Cannot start command input worker: {e}"))?,
        );
        let (sender, receiver) = std::sync::mpsc::channel();
        running.reader = Some(
            std::thread::Builder::new()
                .name("hitslop.command.output".into())
                .spawn(move || {
                    let mut output = Vec::new();
                    let result = stdout.take(OUTPUT as u64 + 1).read_to_end(&mut output);
                    let _ = sender.send((result, output));
                })
                .map_err(|e| format!("Cannot start command output worker: {e}"))?,
        );
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
        let written = running.stop();
        // A reply to input it never fully read is no reply.
        if result.is_ok() {
            written?;
        }
        result
    }
}

/// The app process keeps SIGPIPE's default action, which ends it when a child exits before
/// reading its input. macOS refuses the signal for this pipe alone; the write fails instead.
#[cfg(target_os = "macos")]
fn no_broken_pipe_signal(stdin: &std::process::ChildStdin) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    const F_SETNOSIGPIPE: libc::c_int = 73; // <sys/fcntl.h>; the libc crate omits it.
    // SAFETY: sets a flag on a descriptor this function borrows.
    if unsafe { libc::fcntl(stdin.as_raw_fd(), F_SETNOSIGPIPE, 1) } == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(target_os = "macos"))]
fn no_broken_pipe_signal(_: &std::process::ChildStdin) -> std::io::Result<()> {
    Ok(())
}
/// Elsewhere a write's SIGPIPE goes to the writing thread: blocked there, it stays pending
/// until this short-lived thread exits and the write fails with EPIPE.
#[cfg(not(target_os = "macos"))]
fn block_broken_pipe_signal() -> std::io::Result<()> {
    // SAFETY: changes only the calling thread's signal mask.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        if libc::sigemptyset(&mut set) == -1 || libc::sigaddset(&mut set, libc::SIGPIPE) == -1 {
            return Err(std::io::Error::last_os_error());
        }
        let status = libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status));
        }
    }
    Ok(())
}

#[cfg(test)]
mod process_tests {
    use super::*;
    #[test]
    fn an_abandoned_or_unwinding_guard_kills_and_reaps_its_child() {
        for panic in [false, true] {
            let child = Command::new("/bin/cat").stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
            let pid = child.id() as libc::pid_t;
            let result = std::panic::catch_unwind(|| {
                let _running = Running::new(child);
                assert!(!panic, "unwind after spawn");
            });
            assert_eq!(result.is_err(), panic);
            let mut status = 0;
            // SAFETY: status is writable and pid names only the child this test spawned.
            let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
            assert_eq!(waited, -1, "the guard must reap, not just signal, its child");
            assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ECHILD));
        }
    }
}
