//! The writer lock and live-owner discovery, outside the document: `~/.hitslop/live`, one
//! lock file and one discovery file per document file, named by its device and inode. The
//! folder comes from the account's home, not `$HOME`, so the app, the helper and the engine
//! agree even under `sudo`. The lock is an `flock` on the registry file and never on the
//! database: closing any second descriptor on a SQLite file drops SQLite's own locks. Lock
//! files are never unlinked; only the lock holder removes a discovery file, and a dropped
//! lease withdraws its own. `sweep` clears a crashed owner's discovery.

use crate::error::{Error, Result, failed, invalid};
use std::ffi::CStr;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static FOLDER: OnceLock<PathBuf> = OnceLock::new();
/// Uses `path` as this process's registry instead of `~/.hitslop/live`, so test runs never
/// fill a person's registry. Every process sharing documents must use the same folder:
/// only debug hosts call this (`HITSLOP_TEST_REGISTRY`), and the engine for reads.
pub fn use_folder(path: &Path) -> Result<()> {
    FOLDER.set(path.to_owned()).map_err(|_| failed("The registry folder was already chosen"))
}
/// `~/.hitslop/live` for the account running this process.
fn folder() -> Result<PathBuf> {
    if let Some(folder) = FOLDER.get() {
        return Ok(folder.clone());
    }
    let home = home().ok_or_else(|| failed("Cannot find this account's home folder"))?;
    Ok(home.join(".hitslop/live"))
}
/// This account's home folder, not `$HOME`, so every process of the account agrees.
pub(crate) fn home() -> Option<&'static Path> {
    static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    HOME.get_or_init(lookup_home).as_deref()
}
/// This account's home folder from the password database. `getpwuid_r`, because
/// `getpwuid`'s result lives in storage every other thread's lookup may overwrite.
fn lookup_home() -> Option<PathBuf> {
    let mut buffer = vec![0 as libc::c_char; 4096];
    loop {
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut found = std::ptr::null_mut();
        // SAFETY: every pointer is valid for the call and `buffer.len()` is its length; the
        // entry's strings point into `buffer`, which outlives their use below.
        let status = unsafe {
            libc::getpwuid_r(libc::getuid(), entry.as_mut_ptr(), buffer.as_mut_ptr(), buffer.len(), &mut found)
        };
        if status == libc::ERANGE && buffer.len() < 1 << 20 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != 0 || found.is_null() {
            return None;
        }
        // SAFETY: a found entry was written in full.
        let directory = unsafe { entry.assume_init_ref() }.pw_dir;
        if directory.is_null() {
            return None;
        }
        // SAFETY: a non-null `pw_dir` is a NUL-terminated string inside `buffer`.
        let directory = unsafe { CStr::from_ptr(directory) };
        return Some(PathBuf::from(std::ffi::OsStr::from_bytes(directory.to_bytes())));
    }
}
/// The device, inode and link count of the regular file at `path`; `Moved` for anything
/// else. The device and inode name the file in the registry.
pub(crate) fn identity(path: &Path) -> Result<(u64, u64, u64)> {
    let meta = fs::symlink_metadata(path).map_err(|_| Error::Moved)?;
    if !meta.is_file() {
        return Err(Error::Moved);
    }
    Ok((meta.dev() as u64, meta.ino(), meta.nlink() as u64))
}
fn name(dev: u64, ino: u64) -> String {
    format!("{dev:x}-{ino:x}")
}

/// The one writer of a document file. Released when dropped.
pub struct Lease {
    _lock: fs::File,
    path: PathBuf,
    dev: u64,
    ino: u64,
    discovery: PathBuf,
}
impl Lease {
    /// Takes the writer lock, or `Locked` when another writer holds it. Refuses a file with
    /// a second hard link: SQLite names its journal after the path it opened.
    pub fn acquire(path: &Path) -> Result<Self> {
        // A stat, never a descriptor on the database: POSIX record locks belong to the
        // process, so closing any descriptor on the file would release every lock SQLite
        // holds on it through this process's other connections.
        let meta = fs::symlink_metadata(path).map_err(|e| failed(format!("Cannot open the document: {e}")))?;
        if !meta.is_file() {
            return Err(failed("A document must be a regular file"));
        }
        if meta.nlink() > 1 {
            return Err(invalid("This document has another hard link; duplicate it to edit a copy"));
        }
        let (dev, ino) = (meta.dev() as u64, meta.ino());
        let folder = folder()?;
        fs::create_dir_all(&folder).map_err(|e| failed(format!("Cannot create {}: {e}", folder.display())))?;
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).map_err(failed)?;
        let key = name(dev, ino);
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(folder.join(format!("{key}.lock")))
            .map_err(|_| failed("Cannot open the writer lock"))?;
        lock.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => Error::Locked,
            fs::TryLockError::Error(_) => failed("Cannot acquire the writer lock"),
        })?;
        let lease =
            Self { _lock: lock, path: path.to_owned(), dev, ino, discovery: folder.join(format!("{key}.json")) };
        // The path must still name the file this lock is for.
        lease.check()?;
        // A crashed owner's discovery file is stale; only the lock holder removes it.
        lease.withdraw();
        Ok(lease)
    }
    /// The locked file's device and inode.
    pub fn file(&self) -> (u64, u64) {
        (self.dev, self.ino)
    }
    /// `Moved` once the path names another file, or the file gained a hard link.
    pub(crate) fn check(&self) -> Result<()> {
        match identity(&self.path)? {
            (dev, ino, 1) if (dev, ino) == (self.dev, self.ino) => Ok(()),
            _ => Err(Error::Moved),
        }
    }
    /// Names the live owner's socket for clients, atomically.
    pub fn publish(&self, json: &str) -> Result<()> {
        let staged = self.discovery.with_extension("json.tmp");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&staged)
            .map_err(failed)?;
        file.write_all(json.as_bytes()).map_err(failed)?;
        fs::rename(&staged, &self.discovery).map_err(failed)
    }
    pub fn withdraw(&self) {
        let _ = fs::remove_file(&self.discovery);
        let _ = fs::remove_file(self.discovery.with_extension("json.tmp"));
    }
}
impl Drop for Lease {
    /// Runs before the lock is released, so the holder is still the one removing it.
    fn drop(&mut self) {
        self.withdraw();
    }
}

/// Removes discovery files whose owner is gone (a crashed one's): each whose lock can be
/// taken for a moment. Never creates or removes a lock file. Returns how many it removed.
/// Run at app launch; a writer acquiring meanwhile sees the lock busy for that moment.
pub fn sweep() -> Result<usize> {
    let folder = folder()?;
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(failed(e)),
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(key) = name.strip_suffix(".json.tmp").or_else(|| name.strip_suffix(".json")) else { continue };
        // Held across the removal, and released when it drops at the end of this turn.
        let held = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(folder.join(format!("{key}.lock")));
        let free = match &held {
            Ok(lock) => lock.try_lock().is_ok(),
            // No lock file: no owner ever held this key.
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        };
        if free && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
        drop(held);
    }
    Ok(removed)
}

/// The live owner's discovery for the file at `path`, when one is published. A client
/// still verifies the owner it reaches.
pub fn discovery(path: &Path) -> Result<Option<String>> {
    let (dev, ino, _) = identity(path)?;
    match fs::read_to_string(folder()?.join(format!("{}.json", name(dev, ino)))) {
        Ok(json) => Ok(Some(json)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(failed(e)),
    }
}
