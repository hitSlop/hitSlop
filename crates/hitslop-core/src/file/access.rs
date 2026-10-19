//! Thin platform operations; policy and session records belong to `publication`.
use crate::error::{Result, failed};
use std::{fs, path::Path};

#[cfg(target_os = "macos")]
mod apple {
    use super::*;
    use std::{
        ffi::{CStr, CString},
        os::unix::ffi::OsStrExt,
        panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    };
    unsafe extern "C" {
        fn publication_claim(
            path: *const libc::c_char,
            writing: bool,
            context: *mut libc::c_void,
            access: extern "C" fn(*mut libc::c_void, *const libc::c_char),
        ) -> *mut libc::c_char;
        fn publication_clone(source: *const libc::c_char, destination: *const libc::c_char) -> libc::c_int;
        fn publication_replace(
            original: *const libc::c_char,
            candidate: *const libc::c_char,
            result: *mut *mut libc::c_char,
        ) -> *mut libc::c_char;
        fn publication_metadata(source: *const libc::c_char, destination: *const libc::c_char) -> *mut libc::c_char;
        fn publication_ubiquitous(path: *const libc::c_char) -> bool;
    }
    fn path(path: &Path) -> Result<CString> {
        CString::new(path.as_os_str().as_bytes()).map_err(|_| failed("Invalid document path"))
    }
    fn take(value: *mut libc::c_char) -> String {
        // SAFETY: the adapter returns a strdup allocation, consumed exactly once.
        let text = unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned();
        // SAFETY: `value` is that same strdup allocation and is no longer used.
        unsafe { libc::free(value.cast()) };
        text
    }
    fn checked(error: *mut libc::c_char) -> Result<()> {
        if error.is_null() { Ok(()) } else { Err(failed(take(error))) }
    }
    pub(super) fn claim<T, F: FnOnce(&Path) -> Result<T>>(name: &Path, writing: bool, work: F) -> Result<T> {
        struct Context<F, T> {
            work: Option<F>,
            result: Option<std::thread::Result<Result<T>>>,
        }
        extern "C" fn access<F: FnOnce(&Path) -> Result<T>, T>(context: *mut libc::c_void, name: *const libc::c_char) {
            // SAFETY: the synchronous coordinator holds this stack context until return.
            let context = unsafe { &mut *context.cast::<Context<F, T>>() };
            // SAFETY: Foundation keeps this resolved, NUL-terminated path alive for the callback.
            let name = unsafe { CStr::from_ptr(name) };
            let path = Path::new(std::ffi::OsStr::from_bytes(name.to_bytes()));
            context.result = Some(catch_unwind(AssertUnwindSafe(|| context.work.take().expect("one accessor")(path))));
        }
        let mut context = Context { work: Some(work), result: None };
        let name = path(name)?;
        // SAFETY: valid strings and a stack callback context; no unwind crosses ObjC.
        let error = unsafe {
            publication_claim(name.as_ptr(), writing, (&mut context as *mut Context<F, T>).cast(), access::<F, T>)
        };
        checked(error)?;
        match context.result.ok_or_else(|| failed("File coordination did not run"))? {
            Ok(result) => result,
            Err(panic) => resume_unwind(panic),
        }
    }
    pub(super) fn clone(source: &Path, dest: &Path) -> Result<bool> {
        let (a, b) = (path(source)?, path(dest)?);
        // SAFETY: valid NUL-terminated paths.
        match unsafe { publication_clone(a.as_ptr(), b.as_ptr()) } {
            0 => Ok(true),
            libc::EXDEV | libc::ENOTSUP => Ok(false),
            error => Err(failed(std::io::Error::from_raw_os_error(error))),
        }
    }
    pub(super) fn replace(source: &Path, dest: &Path) -> Result<()> {
        let (a, b) = (path(dest)?, path(source)?);
        let mut actual = std::ptr::null_mut();
        // SAFETY: valid paths and an owned result string.
        checked(unsafe { publication_replace(a.as_ptr(), b.as_ptr(), &mut actual) })?;
        if actual.is_null() {
            return Err(failed("Replacement returned no destination"));
        }
        let actual = take(actual);
        if super::super::resolve(Path::new(&actual))? != super::super::resolve(dest)? {
            return Err(failed("Publication moved; the private copy is retained"));
        }
        Ok(())
    }
    pub(super) fn metadata(source: &Path, dest: &Path) -> Result<()> {
        let (a, b) = (path(source)?, path(dest)?);
        // SAFETY: valid paths; the adapter returns an owned error or null.
        checked(unsafe { publication_metadata(a.as_ptr(), b.as_ptr()) })
    }
    pub(super) fn ubiquitous(name: &Path) -> Result<bool> {
        let name = path(name)?;
        // SAFETY: valid path; no side effects or downloads.
        Ok(unsafe { publication_ubiquitous(name.as_ptr()) })
    }
}

pub(crate) fn claim<T>(path: &Path, writing: bool, work: impl FnOnce(&Path) -> Result<T>) -> Result<T> {
    #[cfg(target_os = "macos")]
    {
        apple::claim(path, writing, work)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = writing;
        work(path)
    }
}
pub(crate) fn clone_file(source: &Path, dest: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    if apple::clone(source, dest)? {
        return Ok(());
    }
    let mut input = fs::File::open(source).map_err(failed)?;
    let mut output = fs::OpenOptions::new().write(true).create_new(true).open(dest).map_err(failed)?;
    std::io::copy(&mut input, &mut output).map_err(failed)?;
    Ok(())
}
pub(crate) fn replace(source: &Path, dest: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        apple::replace(source, dest)
    }
    #[cfg(not(target_os = "macos"))]
    {
        fs::rename(source, dest).map_err(failed)
    }
}
pub(crate) fn metadata(source: &Path, dest: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    apple::metadata(source, dest)?;
    fs::set_permissions(dest, fs::metadata(source).map_err(failed)?.permissions()).map_err(failed)
}
pub(crate) fn ubiquitous(path: &Path) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        apple::ubiquitous(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Ok(false)
    }
}
pub(crate) fn sync_file(path: &Path) -> Result<()> {
    let file = fs::File::open(path).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a valid open descriptor; this does not close it.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } != 0 {
            return Err(failed(std::io::Error::last_os_error()));
        }
    }
    Ok(())
}
pub(crate) fn sync_dir(path: &Path) -> Result<()> {
    fs::File::open(path).and_then(|file| file.sync_all()).map_err(failed)
}

/// Explicit opens/template reads may materialize a cloud-only item. Catalog display
/// reads deliberately do not use this helper.
pub(crate) fn reading<T>(path: &Path, work: impl FnOnce(&Path) -> Result<T>) -> Result<T> {
    if super::is_icloud(path)? { claim(path, false, work) } else { work(path) }
}
