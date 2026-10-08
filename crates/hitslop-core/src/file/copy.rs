//! New files beside their destination: staged, checked and published without replacing
//! anything; documents created from templates and copied from documents.

use super::{Kind, check, checked, configure_writer, opened, reader, resolve, rows, writer};
use crate::Code;
use crate::error::{Error, Result, failed, invalid, rejected, sqlite};
use rusqlite::Connection;
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// A private temporary file beside `dest`, removed if it is never published.
pub(crate) struct Staged {
    path: PathBuf,
    published: bool,
}
impl Staged {
    pub(crate) fn beside(dest: &Path) -> Result<Self> {
        let dest = resolve(dest)?;
        let name = dest.file_name().expect("a resolved path names its file").to_string_lossy();
        Ok(Self { path: dest.with_file_name(format!(".{name}.{}.tmp", crate::random_hex(8))), published: false })
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    /// Publishes the finished file without replacing anything at `dest`; a durable one's
    /// folder entry is synced too.
    pub(crate) fn publish_new(mut self, dest: &Path, durable: bool) -> Result<()> {
        let (from, to) = (cstring(&self.path)?, cstring(&resolve(dest)?)?);
        #[cfg(target_os = "macos")]
        // SAFETY: valid C strings.
        let status = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(target_os = "linux")]
        // Rust's self-contained musl may not export the renameat2 libc wrapper.
        // SAFETY: the Linux syscall receives valid C strings and directory descriptors.
        let status = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if status != 0 {
            let error = std::io::Error::last_os_error();
            return Err(if error.raw_os_error() == Some(libc::EEXIST) {
                rejected(Code::Exists, "A file already exists there")
            } else {
                failed(format!("Cannot save the document: {error}"))
            });
        }
        self.published = true;
        if durable {
            sync_folder(dest)?;
        }
        Ok(())
    }
    /// Publishes over a template, never over a document.
    pub(super) fn publish_template(mut self, dest: &Path) -> Result<()> {
        if let Ok(meta) = fs::symlink_metadata(dest) {
            if !meta.is_file() {
                return Err(rejected(
                    Code::Exists,
                    format!("Refusing to replace {}: it is not a template", dest.display()),
                ));
            }
            if checked(dest)?.1 != Kind::Template {
                return Err(rejected(Code::Exists, "Refusing to replace a document with a template"));
            }
            fs::rename(&self.path, resolve(dest)?).map_err(|e| failed(format!("Cannot save the template: {e}")))?;
            self.published = true;
            sync_folder(dest)?;
            return Ok(());
        }
        self.publish_new(dest, true)
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(self.path.with_extension("tmp-journal"));
        }
    }
}
fn cstring(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| failed("Invalid path"))
}
fn sync_folder(path: &Path) -> Result<()> {
    // The rename already published the file. Report an uncertain durability outcome,
    // retaining that destination rather than deleting or retrying it.
    let sync = || -> Result<()> {
        let resolved = resolve(path)?;
        let folder = resolved.parent().ok_or_else(|| failed("Invalid destination folder"))?;
        fs::File::open(folder).and_then(|file| file.sync_all()).map_err(failed)
    };
    sync().map_err(|error| {
        failed(format!(
            "The destination exists, but its durability could not be confirmed: {error}. Inspect it before retrying"
        ))
    })
}

/// A change a copy makes to its staged file, before its checks.
pub(crate) type Clean<'a> = &'a dyn Fn(&Connection) -> Result<()>;
/// Copies `source` to `dest` through SQLite's online backup into a temporary file beside
/// `dest`, then checks it and publishes it without replacing anything; `create` adds the
/// document row a template lacks.
/// A `durable` copy is a document a person keeps; a capture's source, read once and then
/// deleted, skips the syncs. `clean` changes the staged copy before its checks, which then
/// include SQLite's quick check.
pub(crate) fn copy(source: &Connection, dest: &Path, create: bool, durable: bool, clean: Option<Clean>) -> Result<()> {
    let staged = Staged::beside(dest)?;
    let mut output = writer(staged.path(), true)?;
    if !durable {
        output.execute_batch("PRAGMA synchronous=OFF; PRAGMA journal_mode=OFF;").map_err(sqlite("configure"))?;
    }
    {
        let backup = rusqlite::backup::Backup::new(source, &mut output).map_err(sqlite("Cannot copy the document"))?;
        match backup.step(-1).map_err(sqlite("Cannot copy the document"))? {
            rusqlite::backup::StepResult::Done => {}
            rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => return Err(Error::Busy),
            _ => return Err(failed("The copy did not complete")),
        }
    }
    if durable {
        configure_writer(&output)?;
    }
    if create {
        let tx = super::begin_write(&output, "create document")?;
        rows::add_document(&tx)?;
        tx.commit()?;
    }
    if let Some(clean) = clean {
        clean(&output)?;
    }
    check(&output, clean.is_some())?;
    output.close().map_err(|(_, e)| sqlite("close")(e))?;
    staged.publish_new(dest, durable)
}
/// A new document from a template: a copy of it, its initial state included, with the
/// document row added. Missing folders above `dest` are made once it may go there.
pub fn create_document(template: &Path, dest: &Path) -> Result<()> {
    super::places::document_destination(dest)?;
    if let Some(folder) = dest.parent().filter(|folder| !folder.as_os_str().is_empty()) {
        fs::create_dir_all(folder).map_err(|e| failed(format!("Cannot make the document's folder: {e}")))?;
    }
    let source = reader(template)?;
    // The app and its complete saved state are checked in the same read as the copy:
    // a template the owner would refuse to open publishes nothing.
    let read = source.unchecked_transaction().map_err(sqlite("read"))?;
    let accepted = opened(&read, template, true)?;
    if accepted.kind != Kind::Template {
        return Err(invalid("Documents are created from a template"));
    }
    // The one call from `file` into `store`: loading saved state is the store's, and this
    // check must be exactly what opening the new document will run.
    crate::store::validate_saved(&read, accepted.app.spec())?;
    copy(&read, dest, true, true, None)
}
