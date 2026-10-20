//! Explicitly move a closed cloud document to local storage. No cloud SQLite writer is
//! opened. A failed cleanup leaves the durable local copy and the original intact.
use super::{Kind, copy, opened, reader};
use crate::error::{Error, Result, failed, invalid, sqlite};
use crate::registry::Lease;
use std::ffi::{CStr, CString, c_char, c_void};
use std::fs;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Path, PathBuf};

pub struct RelocatedDocument {
    pub path: PathBuf,
    /// The destination is usable even when removing the original failed.
    pub cleanup_error: Option<String>,
}

trait Access {
    fn run(&self, path: &Path, deleting: bool, body: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()>;
    fn trash(&self, path: &Path) -> Result<()>;
}

/// Holds both registry leases until the source is trashed or explicitly retained.
pub fn relocate_document(source: &Path, folder: &Path) -> Result<RelocatedDocument> {
    relocate(source, folder, &Foundation)
}

#[derive(Debug, PartialEq, Eq)]
struct Stamp(u64, u64, u64, i64, i64, i64, i64);
fn stamp(path: &Path) -> Result<Stamp> {
    let m = crate::registry::regular(path)?;
    Ok(Stamp(m.dev() as u64, m.ino(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()))
}

fn relocate(source: &Path, folder: &Path, access: &dyn Access) -> Result<RelocatedDocument> {
    let folder = fs::canonicalize(folder).map_err(failed)?;
    let name = source.file_stem().ok_or_else(|| invalid("Invalid document name"))?;
    let mut prepared = None;
    access.run(source, false, &mut |resolved| {
        let lease = Lease::acquire(resolved)?;
        let base = stamp(resolved)?;
        let connection = reader(resolved)?;
        let read = connection.unchecked_transaction().map_err(sqlite("read"))?;
        let accepted = opened(&read, resolved, true)?;
        if accepted.kind != Kind::Document {
            return Err(invalid("Templates create editable copies; they are never moved"));
        }
        crate::store::validate_saved(&read, accepted.app.spec())?;
        for suffix in 1..=10_000 {
            let mut filename = name.to_os_string();
            if suffix != 1 {
                filename.push(format!(" {suffix}"));
            }
            filename.push(".slop");
            let destination = folder.join(filename);
            super::places::document_destination(&destination)?;
            match copy(&read, &destination, false, true, None) {
                Err(Error::Rejected(error)) if error.code == crate::Code::Exists => continue,
                other => other?,
            }
            let destination_lease = Lease::acquire(&destination)?;
            let saved = stamp(&destination)?;
            prepared = Some((lease, destination_lease, base, saved, destination));
            return Ok(());
        }
        Err(failed("Cannot find an unused local document name"))
    })?;
    let (source_lease, destination_lease, base, saved, destination) =
        prepared.ok_or_else(|| failed("The provider did not open the document"))?;
    let cleanup = access.run(source, true, &mut |resolved| {
        source_lease.check()?;
        destination_lease.check()?;
        if stamp(resolved)? != base || stamp(&destination)? != saved {
            return Err(Error::Moved);
        }
        access.trash(resolved)
    });
    Ok(RelocatedDocument { path: destination, cleanup_error: cleanup.err().map(|error| error.to_string()) })
}

struct Foundation;
unsafe extern "C" {
    fn hitslop_relocation_access(
        path: *const c_char,
        deleting: bool,
        accessor: unsafe extern "C" fn(*const c_char, *mut c_void) -> bool,
        context: *mut c_void,
        message: *mut *mut c_char,
    ) -> bool;
    fn hitslop_relocation_trash(path: *const c_char, message: *mut *mut c_char) -> bool;
}
fn path_string(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| invalid("Invalid document path"))
}
fn os_result(ok: bool, message: *mut c_char) -> Result<()> {
    if !message.is_null() {
        // SAFETY: Foundation returns a strdup allocation; consume and free it once.
        let detail = unsafe { CStr::from_ptr(message) }.to_string_lossy().into_owned();
        // SAFETY: `message` is the same strdup allocation, no longer borrowed.
        unsafe { libc::free(message.cast()) };
        return Err(failed(detail));
    }
    if ok { Ok(()) } else { Err(failed("The provider did not complete the file operation")) }
}
impl Access for Foundation {
    fn run(&self, path: &Path, deleting: bool, body: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()> {
        struct Context<'a> {
            body: &'a mut dyn FnMut(&Path) -> Result<()>,
            result: Option<Result<()>>,
        }
        unsafe extern "C" fn call(path: *const c_char, context: *mut c_void) -> bool {
            // SAFETY: the synchronous Foundation accessor borrows these for this call only.
            let context = unsafe { &mut *context.cast::<Context<'_>>() };
            // SAFETY: Foundation supplies a NUL-terminated filesystem representation.
            let path = Path::new(std::ffi::OsStr::from_bytes(unsafe { CStr::from_ptr(path) }.to_bytes()));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (context.body)(path)))
                .unwrap_or_else(|_| Err(failed("File relocation failed")));
            let ok = result.is_ok();
            context.result = Some(result);
            ok
        }
        let path = path_string(path)?;
        let mut context = Context { body, result: None };
        let mut message = std::ptr::null_mut();
        // SAFETY: the C function invokes the accessor synchronously before returning.
        let ok = unsafe {
            hitslop_relocation_access(
                path.as_ptr(),
                deleting,
                call,
                (&mut context as *mut Context<'_>).cast(),
                &mut message,
            )
        };
        let provider = os_result(ok, message);
        context.result.unwrap_or(Ok(()))?;
        provider
    }
    fn trash(&self, path: &Path) -> Result<()> {
        let path = path_string(path)?;
        let mut message = std::ptr::null_mut();
        // SAFETY: valid path and output pointer for the duration of the synchronous call.
        let ok = unsafe { hitslop_relocation_trash(path.as_ptr(), &mut message) };
        os_result(ok, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Local {
        fail_cleanup: bool,
        change_source: bool,
    }
    impl Access for Local {
        fn run(&self, path: &Path, deleting: bool, body: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()> {
            if deleting && self.change_source {
                fs::write(path, b"replaced by another provider version").unwrap();
            }
            body(path)
        }
        fn trash(&self, path: &Path) -> Result<()> {
            if self.fail_cleanup {
                Err(failed("Trash unavailable"))
            } else {
                fs::rename(path, path.with_extension("trashed")).map_err(failed)
            }
        }
    }

    #[test]
    fn relocation_checks_before_trashing_and_keeps_a_usable_copy_on_cleanup_failure() {
        let dir = tempfile::tempdir().unwrap();
        crate::registry::use_folder(&dir.path().join("registry")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/checklist/document");
        let template = dir.path().join("template.slop");
        super::super::pack(&fs::read_to_string(fixture.join("input.json")).unwrap(), &fixture, &template, None)
            .unwrap();
        let source = dir.path().join("source.slop");
        let folder = dir.path().join("local");
        fs::create_dir(&folder).unwrap();
        for (fail_cleanup, change_source) in [(false, false), (true, false), (false, true)] {
            super::super::create_document(&template, &source).unwrap();
            let uuid = super::super::open(&source, true).unwrap().document_uuid;
            let held = Lease::acquire(&source).unwrap();
            assert!(matches!(relocate(&source, &folder, &Local { fail_cleanup, change_source }), Err(Error::Locked)));
            drop(held);
            let result = relocate(&source, &folder, &Local { fail_cleanup, change_source }).unwrap();
            assert_eq!(super::super::open(&result.path, true).unwrap().document_uuid, uuid);
            assert_eq!(result.cleanup_error.is_some(), fail_cleanup || change_source);
            assert_eq!(source.exists(), fail_cleanup || change_source);
            if source.exists() {
                fs::remove_file(&source).unwrap();
            }
        }
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 3, "collisions retain earlier copies");
        let bytes = fs::read(&template).unwrap();
        assert!(relocate(&template, &folder, &Local { fail_cleanup: false, change_source: false }).is_err());
        assert_eq!(fs::read(&template).unwrap(), bytes);
    }
}
