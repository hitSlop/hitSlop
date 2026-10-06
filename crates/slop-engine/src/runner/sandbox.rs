//! An allowlist, installed before any authored JavaScript is evaluated. No filesystem,
//! network, process creation or owner socket operations are available in the child.

pub fn restrict() -> Result<(), String> {
    let directory = if cfg!(target_os = "macos") { "/dev/fd" } else { "/proc/self/fd" };
    let descriptors = std::fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .map(|entry| {
            entry
                .map_err(|e| e.to_string())
                .and_then(|entry| entry.file_name().to_string_lossy().parse::<i32>().map_err(|e| e.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    // SAFETY: these process-local calls operate on valid rlimit values. This process is
    // single-threaded; only stdin/stdout/stderr may survive into authored evaluation.
    unsafe {
        for fd in descriptors.into_iter().filter(|fd| *fd > 2) {
            libc::close(fd);
        }
        for (resource, value) in [(libc::RLIMIT_CPU, 3), (libc::RLIMIT_NOFILE, 3), (libc::RLIMIT_CORE, 0)] {
            let limit = libc::rlimit { rlim_cur: value, rlim_max: value };
            if libc::setrlimit(resource, &limit) != 0 {
                return Err("Cannot limit command process".into());
            }
        }
    }
    platform()
}

#[cfg(target_os = "macos")]
fn platform() -> Result<(), String> {
    use std::ffi::CStr;
    unsafe extern "C" {
        fn sandbox_init(profile: *const libc::c_char, flags: u64, error: *mut *mut libc::c_char) -> libc::c_int;
        fn sandbox_free_error(error: *mut libc::c_char);
    }
    let profile = c"(version 1)(deny default)(allow sysctl-read)";
    let mut error = std::ptr::null_mut();
    // SAFETY: profile is NUL terminated and error is writable; Apple's API owns the
    // returned error buffer until sandbox_free_error. Failure is never ignored.
    unsafe {
        if sandbox_init(profile.as_ptr(), 0, &mut error) != 0 {
            let message = if error.is_null() {
                "Cannot sandbox command".into()
            } else {
                let message = CStr::from_ptr(error).to_string_lossy().into_owned();
                sandbox_free_error(error);
                message
            };
            return Err(message);
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform() -> Result<(), String> {
    use libc::*;
    // seccomp_data: syscall number at 0, audit architecture at 4. Verify architecture
    // before interpreting numbers; alternate ABIs cannot bypass the allowlist.
    #[cfg(target_arch = "x86_64")]
    const ARCH: u32 = 0xc000003e;
    #[cfg(target_arch = "aarch64")]
    const ARCH: u32 = 0xc00000b7;
    let instruction = |code, jt, jf, k| sock_filter { code, jt, jf, k };
    let mut filter = vec![
        instruction(0x20, 0, 0, 4),
        instruction(0x15, 1, 0, ARCH),
        instruction(0x06, 0, 0, SECCOMP_RET_KILL_PROCESS),
        instruction(0x20, 0, 0, 0),
    ];
    for call in [
        SYS_read,
        SYS_write,
        SYS_close,
        SYS_fstat,
        SYS_lseek,
        SYS_brk,
        SYS_mmap,
        SYS_munmap,
        SYS_mprotect,
        SYS_mremap,
        SYS_madvise,
        SYS_rt_sigaction,
        SYS_rt_sigprocmask,
        SYS_rt_sigreturn,
        SYS_sigaltstack,
        SYS_futex,
        SYS_clock_gettime,
        SYS_gettimeofday,
        SYS_getrandom,
        SYS_getpid,
        SYS_gettid,
        SYS_sched_yield,
        SYS_exit,
        SYS_exit_group,
    ] {
        filter.push(instruction(0x15, 0, 1, call as u32));
        filter.push(instruction(0x06, 0, 0, SECCOMP_RET_ALLOW));
    }
    filter.push(instruction(0x06, 0, 0, SECCOMP_RET_ERRNO | EPERM as u32));
    let program = sock_fprog { len: filter.len() as u16, filter: filter.as_mut_ptr() };
    // SAFETY: the BPF program points into filter for both calls. The kernel copies it;
    // no_new_privs prevents privilege acquisition, and failure refuses evaluation.
    unsafe {
        if prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 || prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &program) != 0 {
            return Err("Cannot sandbox command".into());
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform() -> Result<(), String> {
    Err("Command isolation is unavailable on this platform".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn sandbox_denies_file_and_socket_access() {
        const MARKER: &str = "HITSLOP_SANDBOX_PROBE";
        if std::env::var_os(MARKER).is_some() {
            let path = std::env::var_os(MARKER).unwrap();
            super::restrict().unwrap();
            let file = std::fs::read(path);
            // SAFETY: socket takes numeric constants, returning an owned descriptor or
            // -1. If the sandbox failed to deny it, close it before reporting failure.
            let socket = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
            if socket >= 0 {
                // SAFETY: socket returned a live descriptor owned here.
                unsafe {
                    libc::close(socket);
                }
            }
            std::process::exit(if file.is_err() && socket < 0 { 0 } else { 1 });
        }
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "must not be readable").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "runner::sandbox::tests::sandbox_denies_file_and_socket_access", "--nocapture"])
            .env(MARKER, file.path())
            .status()
            .unwrap();
        assert!(status.success());
    }
}
