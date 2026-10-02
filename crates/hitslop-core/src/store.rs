//! Durable storage for native hosts. A package's `state/document.sqlite` holds one
//! checkpoint (a Loro snapshot), the updates saved after it, and a small row with the
//! document's identity and theme overrides; the writer lock beside it names the one
//! process that may write. SQLite never sees anything but opaque Loro bytes.
//!
//! History is trimmed when nothing is editing: as a session closes, a document larger
//! than `TRIM_BYTES` keeps only the history since that session opened, or none when even
//! that is larger. While open, a checkpoint trims only past `SESSION_BYTES`. Compaction
//! keeps no history.
//!
//! A host keeps two serial queues: edits and `Store::job` on one, every other `Store`
//! call on the other, so a slow write never blocks edits.

use crate::{theme, Document};
use loro::{ExportMode, Frontiers, VersionVector};
use rusqlite::{ffi::ErrorCode, params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior};
use std::fs;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const MAX_BYTES: i64 = crate::STORAGE_BYTES as i64;
const MAX_ROWS: i64 = crate::STORAGE_ROWS as i64;
/// A save checkpoints instead of appending once the log reaches either.
const CHECKPOINT_ROWS: i64 = 256;
const CHECKPOINT_BYTES: i64 = 4 * 1024 * 1024;
/// A closing session that edited a document larger than this trims its history. Trimming
/// is not free: Loro re-encodes what it keeps instead of reusing its cached snapshot.
const TRIM_BYTES: i64 = 4 * 1024 * 1024;
/// A checkpoint larger than this trims history while the session is still open.
const SESSION_BYTES: usize = 16 * 1024 * 1024;
const MAX_KEY_BYTES: i64 = 1024 * 1024;
const APPLICATION_ID: i64 = 0x4853_4C50; // HSLP
const STORAGE_VERSION: i64 = 3;
/// `doc_id` names the logical document. It is minted with the database and renewed by
/// a duplicate; a plain filesystem copy keeps it, so it never authorizes synchronization.
/// `document.theme` holds the owner's theme overrides (JSON): presentation state outside
/// Loro, saved and backed up with the document. The checkpoint has its own row, absent
/// until the first save, so small writes never rewrite it.
const SCHEMA: &str = "CREATE TABLE document(id INTEGER PRIMARY KEY CHECK(id=1), doc_id TEXT NOT NULL, theme TEXT NOT NULL DEFAULT '{}'); INSERT INTO document(id,doc_id) VALUES(1,lower(hex(randomblob(16)))); CREATE TABLE checkpoint(id INTEGER PRIMARY KEY CHECK(id=1), schema_key TEXT NOT NULL, bytes BLOB NOT NULL); CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);";
const BOUNDS: &str = "SELECT (SELECT count(*) FROM updates),(SELECT COALESCE(sum(length(bytes)),0) FROM updates),COALESCE((SELECT length(bytes) FROM checkpoint),0),COALESCE((SELECT length(CAST(schema_key AS BLOB)) FROM checkpoint),0)";

/// Why a storage call failed. Every save failure keeps ownership, the live state and all
/// edits.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Another process holds the writer lock.
    #[error("document has a live writer")]
    Locked,
    /// Another connection held the database (for example a backup); retrying can succeed.
    #[error("document database is busy")]
    Busy,
    /// The write would exceed the storage limits; saved state is intact.
    #[error("document is full")]
    Full,
    /// The package directory was moved or replaced while open.
    #[error("document moved or replaced")]
    Moved,
    /// The store no longer owns the package.
    #[error("document is closed")]
    Closed,
    /// The request breaks a document rule (a theme value, for example); nothing changed.
    #[error("{0}")]
    Rejected(crate::Error),
    #[error("{0}")]
    Failed(String),
}
pub type Result<T> = std::result::Result<T, Error>;
fn failed(message: impl ToString) -> Error {
    Error::Failed(message.to_string())
}
fn sqlite(action: &str) -> impl FnOnce(rusqlite::Error) -> Error + '_ {
    move |e| match e.sqlite_error_code() {
        Some(ErrorCode::DiskFull | ErrorCode::TooBig) => Error::Full,
        // BUSY is a definite failure: nothing was written.
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Error::Busy,
        _ => failed(format!("{action}: {e}")),
    }
}

/// Fault injection for tests at the real I/O boundary: `load`, `append:uncommitted`,
/// `append:committed`, `checkpoint:uncommitted`, `checkpoint:committed`,
/// `theme:uncommitted`, `theme:committed` and `close`. A
/// hook may block or fail the call.
pub trait Phases: Send + Sync {
    fn reached(&self, phase: &str) -> Result<()>;
}

/// The package's exclusive writer lock (`state/writer.lock`). Released when dropped.
pub struct WriterLock { _fd: OwnedFd }
impl WriterLock {
    pub fn acquire(root: &Path) -> Result<Self> {
        let state = state_directory(root)?;
        let path = std::ffi::CString::new(state.join("writer.lock").as_os_str().as_bytes())
            .map_err(|_| failed("Invalid document path"))?;
        // SAFETY: a valid C string; the descriptor is owned from here on.
        let fd = unsafe {
            libc::open(path.as_ptr(), libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC, 0o600)
        };
        if fd < 0 {
            return Err(failed("Cannot open writer lock"));
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(fd.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let code = std::io::Error::last_os_error().raw_os_error();
            return Err(if code == Some(libc::EWOULDBLOCK) { Error::Locked } else { failed("Cannot acquire writer lock") });
        }
        Ok(Self { _fd: fd })
    }
}
/// `state/`, created when missing; never a symbolic link.
fn state_directory(root: &Path) -> Result<PathBuf> {
    let state = root.join("state");
    match fs::create_dir(&state) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Err(failed(format!("Cannot create state: {e}"))),
        _ => {}
    }
    safe_directory(&state)?;
    Ok(state)
}
fn safe_directory(path: &Path) -> Result<()> {
    let info = fs::symlink_metadata(path).map_err(|e| failed(format!("Unsafe directory: state: {e}")))?;
    if !info.is_dir() {
        return Err(failed("Unsafe directory: state"));
    }
    Ok(())
}
/// The database path inside a resolved `state/`. The file itself is opened with NOFOLLOW,
/// so a replaced database is still refused.
fn database(state: &Path) -> Result<PathBuf> {
    let resolved = fs::canonicalize(state).map_err(|e| failed(format!("Cannot resolve storage directory: {e}")))?;
    let path = resolved.join("document.sqlite");
    match fs::symlink_metadata(&path) {
        Ok(info) if !info.is_file() => Err(failed("Unsafe file: document.sqlite")),
        _ => Ok(path),
    }
}
fn inode(root: &Path) -> Result<u64> {
    fs::symlink_metadata(root).map(|m| m.ino()).map_err(|_| failed("Cannot identify document directory"))
}
fn connect(path: &Path, flags: OpenFlags, busy: Duration) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW)
        .map_err(sqlite("open"))?;
    conn.busy_timeout(busy).map_err(sqlite("open"))?;
    conn.execute_batch("PRAGMA trusted_schema=OFF").map_err(sqlite("open"))?;
    conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, MAX_BYTES as i32).map_err(sqlite("open"))?;
    Ok(conn)
}
fn is_new(conn: &Connection) -> Result<bool> {
    let tables: i64 = conn
        .query_row("SELECT count(*) FROM sqlite_master WHERE type='table'", [], |r| r.get(0))
        .map_err(sqlite("read schema"))?;
    if tables > 0 {
        let id: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0)).map_err(sqlite("read identity"))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(sqlite("read identity"))?;
        if id != APPLICATION_ID || version != STORAGE_VERSION {
            return Err(failed("Unsupported document storage"));
        }
    }
    Ok(tables == 0)
}

/// Stored sizes, refreshed by every write, so choosing append or checkpoint needs no
/// database read.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Metadata {
    pub rows: i64,
    pub update_bytes: i64,
    pub checkpoint_bytes: i64,
}
fn bounds(conn: &Connection) -> Result<(Metadata, i64)> {
    conn.prepare_cached(BOUNDS)
        .and_then(|mut s| {
            s.query_row([], |r| {
                Ok((Metadata { rows: r.get(0)?, update_bytes: r.get(1)?, checkpoint_bytes: r.get(2)? }, r.get(3)?))
            })
        })
        .map_err(sqlite("read metadata"))
}
/// The stored sizes, checked against the limits with room for `rows` and `bytes` more.
/// Read before any blob is.
fn checked_bounds(conn: &Connection, rows: i64, bytes: i64) -> Result<Metadata> {
    let (meta, key) = bounds(conn)?;
    if meta.rows > 0 && meta.checkpoint_bytes == 0 {
        return Err(failed("Saved updates have no checkpoint; preserve the package for recovery"));
    }
    if key > MAX_KEY_BYTES || meta.rows + rows > MAX_ROWS || meta.update_bytes + meta.checkpoint_bytes + bytes > MAX_BYTES {
        // A write that would cross the limits leaves saved state intact.
        if rows > 0 || bytes > 0 {
            return Err(Error::Full);
        }
        return Err(failed(format!(
            "Document exceeds storage limits ({} MiB or {MAX_ROWS} updates); preserve the package for recovery",
            MAX_BYTES >> 20
        )));
    }
    Ok(meta)
}

/// Saved rows and theme overrides held in memory: a snapshot store's copy of the package.
struct Saved {
    checkpoint: Option<Vec<u8>>,
    schema_key: Option<String>,
    updates: Vec<Vec<u8>>,
    theme: String,
}
impl Default for Saved {
    fn default() -> Self {
        Self { checkpoint: None, schema_key: None, updates: vec![], theme: NO_OVERRIDES.into() }
    }
}

const NO_OVERRIDES: &str = "{}";
impl Saved {
    fn metadata(&self) -> Metadata {
        Metadata {
            rows: self.updates.len() as i64,
            update_bytes: self.updates.iter().map(|u| u.len() as i64).sum(),
            checkpoint_bytes: self.checkpoint.as_ref().map_or(0, |c| c.len() as i64),
        }
    }
}
enum Backing {
    /// Field order matters: the connection closes before the lock is released.
    Disk { conn: Option<Connection>, lock: Option<WriterLock> },
    Memory(Saved),
}
struct Account {
    meta: Metadata,
    /// The version the durable state covers.
    saved: VersionVector,
    /// The version this session opened at: where a closing checkpoint's history starts.
    opened: Frontiers,
    schema_key: String,
}

/// One package's storage. `Document` mode owns the package: it holds the writer lock and
/// persists writes. `Snapshot` mode reads the saved rows once, without the lock; its
/// writes stay in memory, so a render never creates, locks or modifies package files.
pub struct Store {
    root: PathBuf,
    inode: u64,
    doc_id: String,
    backing: Mutex<Backing>,
    account: Mutex<Account>,
    phases: Mutex<Option<Arc<dyn Phases>>>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Document,
    Snapshot,
}

/// Bytes to persist, exported on the edit queue and written on the storage queue.
pub struct SaveJob {
    checkpoint: bool,
    bytes: Vec<u8>,
    version: VersionVector,
}
impl SaveJob {
    pub fn is_checkpoint(&self) -> bool {
        self.checkpoint
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// A checkpoint keeping only the history since `start`, when `accept` takes its bytes.
/// The document's edits may no longer branch from before `start`.
fn trimmed(doc: &mut Document, start: &Frontiers, accept: impl Fn(&[u8]) -> bool) -> Result<Option<Vec<u8>>> {
    let bytes = doc.doc.export(ExportMode::shallow_snapshot(start)).map_err(failed)?;
    if !accept(&bytes) {
        return Ok(None);
    }
    doc.retain_from(start);
    Ok(Some(bytes))
}

/// Opens the saved document, or none before the first save. The checkpoint and every
/// update are imported straight from SQLite's buffers, without copying them.
fn load(conn: &Connection, schema_key: &str) -> Result<Option<(Document, Metadata)>> {
    let meta = checked_bounds(conn, 0, 0)?;
    let mut saved = conn.prepare_cached("SELECT schema_key,bytes FROM checkpoint WHERE id=1").map_err(sqlite("read"))?;
    let mut saved = saved.query([]).map_err(sqlite("read"))?;
    let Some(row) = saved.next().map_err(sqlite("read"))? else {
        return Ok(None);
    };
    if row.get_ref(0).ok().and_then(|key| key.as_str().ok()) != Some(schema_key) {
        return Err(failed("Document schema differs from saved state"));
    }
    let checkpoint = row.get_ref(1).ok().and_then(|bytes| bytes.as_blob().ok()).ok_or_else(|| failed("Invalid checkpoint bytes"))?;
    let mut rows = conn.prepare_cached("SELECT bytes FROM updates ORDER BY seq").map_err(sqlite("read updates"))?;
    let mut rows = rows.query([]).map_err(sqlite("read updates"))?;
    let mut fault = None;
    let doc = Document::open_with(schema_key, checkpoint, |import| loop {
        let next = rows.next().and_then(|row| row.map(|r| r.get_ref(0)).transpose());
        match next {
            Ok(None) => return Ok(()),
            Ok(Some(value)) => match value.as_blob() {
                Ok(bytes) => import(bytes)?,
                Err(_) => {
                    fault = Some(failed("Invalid update bytes"));
                    return Err(crate::err(crate::Code::InvalidBytes, "Invalid update bytes"));
                }
            },
            Err(e) => {
                fault = Some(sqlite("read updates")(e));
                return Err(crate::err(crate::Code::InvalidBytes, "Unreadable update"));
            }
        }
    });
    match doc {
        Ok(doc) => Ok(Some((doc, meta))),
        Err(e) => Err(fault.unwrap_or_else(|| failed(e))),
    }
}

impl Store {
    pub fn open(root: &Path, mode: Mode) -> Result<Self> {
        let inode = inode(root)?;
        let (backing, doc_id) = match mode {
            Mode::Document => {
                let lock = WriterLock::acquire(root)?;
                let path = database(&root.join("state"))?;
                let conn = connect(&path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE, Duration::from_secs(2))?;
                let new = is_new(&conn)?;
                conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON;")
                    .map_err(sqlite("configure"))?;
                if new {
                    // Free pages are reclaimed at every checkpoint.
                    conn.execute_batch(&format!(
                        "PRAGMA auto_vacuum=INCREMENTAL; BEGIN IMMEDIATE; PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={STORAGE_VERSION}; {SCHEMA} COMMIT;"
                    ))
                    .map_err(sqlite("create"))?;
                }
                let doc_id = conn
                    .query_row("SELECT doc_id FROM document WHERE id=1", [], |r| r.get(0))
                    .map_err(sqlite("read identity"))?;
                (Backing::Disk { conn: Some(conn), lock: Some(lock) }, doc_id)
            }
            Mode::Snapshot => {
                let (saved, doc_id) = Self::read_snapshot(root)?;
                (Backing::Memory(saved), doc_id)
            }
        };
        Ok(Self {
            root: root.to_owned(),
            inode,
            doc_id,
            backing: Mutex::new(backing),
            account: Mutex::new(Account {
                meta: Metadata::default(),
                saved: VersionVector::default(),
                opened: Frontiers::default(),
                schema_key: String::new(),
            }),
            phases: Mutex::new(None),
        })
    }
    /// The saved rows, read in one read transaction so validation and the copy see the
    /// same saved state. Limits are checked before any blob is read.
    fn read_snapshot(root: &Path) -> Result<(Saved, String)> {
        let empty = || -> Result<(Saved, String)> {
            let mut bytes = [0u8; 16];
            getrandom::getrandom(&mut bytes).map_err(failed)?;
            Ok((Saved::default(), crate::hex(&bytes)))
        };
        let state = root.join("state");
        if fs::symlink_metadata(&state).is_err() {
            return empty();
        }
        safe_directory(&state)?;
        let path = database(&state)?;
        if fs::symlink_metadata(&path).is_err() {
            return empty();
        }
        let conn = connect(&path, OpenFlags::SQLITE_OPEN_READ_ONLY, Duration::from_secs(5))?;
        let read = Transaction::new_unchecked(&conn, TransactionBehavior::Deferred).map_err(sqlite("read"))?;
        if is_new(&read)? {
            return empty();
        }
        checked_bounds(&read, 0, 0)?;
        // The theme is read with the rows, so a render sees one saved state.
        let (doc_id, theme) = read
            .query_row("SELECT doc_id,theme FROM document WHERE id=1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(sqlite("read"))?;
        let (schema_key, checkpoint) = read
            .query_row("SELECT schema_key,bytes FROM checkpoint WHERE id=1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(sqlite("read"))?
            .unzip();
        let mut updates = vec![];
        let mut rows = read.prepare("SELECT bytes FROM updates ORDER BY seq").map_err(sqlite("read updates"))?;
        let mut rows = rows.query([]).map_err(sqlite("read updates"))?;
        while let Some(row) = rows.next().map_err(sqlite("read updates"))? {
            updates.push(row.get(0).map_err(sqlite("read updates"))?);
        }
        Ok((Saved { checkpoint, schema_key, updates, theme }, doc_id))
    }

    pub fn doc_id(&self) -> &str {
        &self.doc_id
    }
    pub fn set_phases(&self, phases: Option<Arc<dyn Phases>>) {
        *lock(&self.phases) = phases;
    }
    fn phase(&self, name: &str) -> Result<()> {
        let hook = lock(&self.phases).clone();
        hook.map_or(Ok(()), |hook| hook.reached(name))
    }
    /// Fails once the package directory was moved or replaced, or, with `writable`, once
    /// the store no longer owns the package.
    pub fn check(&self, writable: bool) -> Result<()> {
        if inode(&self.root).ok() != Some(self.inode) {
            return Err(Error::Moved);
        }
        if writable && !matches!(&*lock(&self.backing), Backing::Disk { lock: Some(_), .. }) {
            return Err(Error::Closed);
        }
        Ok(())
    }

    /// The saved document. A package without a checkpoint starts from `initial` and saves
    /// its first checkpoint. Also the reload after discarding unsaved edits.
    pub fn document(&self, schema_key: &str, initial: &str) -> Result<Document> {
        self.phase("load")?;
        self.check(false)?;
        let core = |e: crate::Error| failed(e);
        let mut backing = lock(&self.backing);
        let (doc, meta) = match &mut *backing {
            Backing::Memory(saved) => {
                if saved.checkpoint.is_some() && saved.schema_key.as_deref() != Some(schema_key) {
                    return Err(failed("Document schema differs from saved state"));
                }
                match &saved.checkpoint {
                    Some(checkpoint) => {
                        let doc = Document::open(schema_key, checkpoint, &saved.updates).map_err(core)?;
                        (doc, saved.metadata())
                    }
                    None => {
                        let doc = Document::create(schema_key, initial).map_err(core)?;
                        let checkpoint = doc.checkpoint().map_err(core)?;
                        let meta = Metadata { checkpoint_bytes: checkpoint.len() as i64, ..Default::default() };
                        *saved = Saved { checkpoint: Some(checkpoint), schema_key: Some(schema_key.into()), updates: vec![], theme: std::mem::take(&mut saved.theme) };
                        (doc, meta)
                    }
                }
            }
            Backing::Disk { conn, .. } => match load(conn.as_ref().ok_or(Error::Closed)?, schema_key)? {
                Some(loaded) => loaded,
                None => {
                    drop(backing);
                    let doc = Document::create(schema_key, initial).map_err(core)?;
                    let job = SaveJob { checkpoint: true, bytes: doc.checkpoint().map_err(core)?, version: doc.doc.oplog_vv() };
                    let mut account = lock(&self.account);
                    account.schema_key = schema_key.into();
                    account.opened = doc.doc.oplog_frontiers();
                    drop(account);
                    self.write(&job)?;
                    return Ok(doc);
                }
            },
        };
        *lock(&self.account) =
            Account { meta, saved: doc.doc.oplog_vv(), opened: doc.doc.oplog_frontiers(), schema_key: schema_key.into() };
        Ok(doc)
    }

    /// The next write for `doc`, or none when the durable state already covers it and no
    /// checkpoint is requested. Exports only what it writes: the updates since the last
    /// save, or a checkpoint once the log is long, full or a checkpoint is requested.
    pub fn job(&self, doc: &mut Document, force_checkpoint: bool) -> Result<Option<SaveJob>> {
        let (meta, key, saved, opened) = {
            let account = lock(&self.account);
            (account.meta, account.schema_key.len() as i64, account.saved.clone(), account.opened.clone())
        };
        let version = doc.doc.oplog_vv();
        if version == saved && !force_checkpoint {
            return Ok(None);
        }
        let job = |checkpoint, bytes| SaveJob { checkpoint, bytes, version: version.clone() };
        // SQLite also bounds the complete row, including its schema key.
        let fits = |bytes: &[u8]| bytes.len() as i64 + key + 512 <= MAX_BYTES;
        let snapshot = |doc: &mut Document| -> Result<Option<SaveJob>> {
            let latest = doc.doc.oplog_frontiers();
            if force_checkpoint {
                return Ok(trimmed(doc, &latest, fits)?.map(|bytes| job(true, bytes)));
            }
            let bytes = doc.doc.export(ExportMode::Snapshot).map_err(failed)?;
            if bytes.len() <= SESSION_BYTES && fits(&bytes) {
                return Ok(Some(job(true, bytes)));
            }
            // A session too large to keep everything keeps its own history, else none.
            let bytes = match trimmed(doc, &opened, |b| b.len() <= SESSION_BYTES && fits(b))? {
                Some(bytes) => Some(bytes),
                None => trimmed(doc, &latest, fits)?,
            };
            Ok(bytes.map(|bytes| job(true, bytes)))
        };
        let updates = |doc: &Document| -> Result<Option<SaveJob>> {
            let bytes = doc.doc.export(ExportMode::updates(&saved)).map_err(failed)?;
            Ok((meta.rows < MAX_ROWS && meta.checkpoint_bytes + meta.update_bytes + bytes.len() as i64 <= MAX_BYTES)
                .then(|| job(false, bytes)))
        };
        let maintenance = meta.rows >= CHECKPOINT_ROWS || meta.update_bytes >= CHECKPOINT_BYTES;
        if force_checkpoint || maintenance {
            if let Some(job) = snapshot(doc)? {
                return Ok(Some(job));
            }
            if force_checkpoint {
                return Err(Error::Full);
            }
            // Optional maintenance must not prevent an update that still fits the log.
            return updates(doc)?.map_or(Err(Error::Full), |job| Ok(Some(job)));
        }
        if let Some(job) = updates(doc)? {
            return Ok(Some(job));
        }
        snapshot(doc)?.map_or(Err(Error::Full), |job| Ok(Some(job)))
    }

    /// The checkpoint to write as the owner closes, after its last save. A session that
    /// edited a document larger than `TRIM_BYTES` leaves only its own history behind, so a
    /// later session can still read what this one changed, when that fits `TRIM_BYTES`;
    /// otherwise no history. A cut before the latest version keeps, in its starting
    /// state, everything deleted before it (Loro 1.16.2), so only a cut at the latest
    /// version reclaims a document that deletes a lot. None when nothing would shrink.
    pub fn close_job(&self, doc: &mut Document) -> Result<Option<SaveJob>> {
        let (meta, key, opened) = {
            let account = lock(&self.account);
            (account.meta, account.schema_key.len() as i64, account.opened.clone())
        };
        let stored = meta.checkpoint_bytes + meta.update_bytes;
        let latest = doc.doc.oplog_frontiers();
        if latest == opened || stored <= TRIM_BYTES {
            return Ok(None);
        }
        let smaller = |bytes: &[u8]| (bytes.len() as i64) < stored && bytes.len() as i64 + key + 512 <= MAX_BYTES;
        let version = doc.doc.oplog_vv();
        let bytes = match trimmed(doc, &opened, |b| b.len() as i64 <= TRIM_BYTES && smaller(b))? {
            Some(bytes) => Some(bytes),
            None => trimmed(doc, &latest, smaller)?,
        };
        Ok(bytes.map(|bytes| SaveJob { checkpoint: true, bytes, version }))
    }

    /// Writes a job in one transaction. An error may follow the commit, so the durable
    /// version advances only on success; sizes are re-read either way.
    pub fn write(&self, job: &SaveJob) -> Result<()> {
        self.check(false).map_err(|_| Error::Moved)?;
        if job.bytes.is_empty() {
            return Err(failed("Invalid save bytes"));
        }
        let mut backing = lock(&self.backing);
        let schema_key = lock(&self.account).schema_key.clone();
        let meta = match &mut *backing {
            Backing::Memory(saved) => {
                if job.checkpoint {
                    saved.checkpoint = Some(job.bytes.clone());
                    saved.schema_key = Some(schema_key);
                    saved.updates.clear();
                } else {
                    saved.updates.push(job.bytes.clone());
                }
                saved.metadata()
            }
            Backing::Disk { conn, .. } => {
                let conn = conn.as_ref().ok_or(Error::Closed)?;
                match self.transaction(conn, job, &schema_key) {
                    Ok(meta) => meta,
                    Err(e) => {
                        if let Ok((meta, _)) = bounds(conn) {
                            lock(&self.account).meta = meta;
                        }
                        return Err(e);
                    }
                }
            }
        };
        let mut account = lock(&self.account);
        account.meta = meta;
        account.saved = job.version.clone();
        Ok(())
    }
    /// Dropping the guard rolls back: on any error, after a failed COMMIT, or in a panic.
    fn transaction(&self, conn: &Connection, job: &SaveJob, schema_key: &str) -> Result<Metadata> {
        let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("begin"))?;
        let size = job.bytes.len() as i64;
        let (method, meta) = if job.checkpoint {
            if size > MAX_BYTES || schema_key.len() as i64 > MAX_KEY_BYTES {
                return Err(Error::Full);
            }
            tx.prepare_cached("INSERT INTO checkpoint VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET schema_key=excluded.schema_key,bytes=excluded.bytes")
                .and_then(|mut s| s.execute(params![schema_key, job.bytes]))
                .map_err(sqlite("checkpoint"))?;
            tx.execute_batch("DELETE FROM updates").map_err(sqlite("checkpoint"))?;
            // Each step frees one page.
            tx.prepare_cached("PRAGMA incremental_vacuum")
                .and_then(|mut s| s.query([])?.mapped(|_| Ok(())).collect::<rusqlite::Result<()>>())
                .map_err(sqlite("checkpoint"))?;
            ("checkpoint", Metadata { rows: 0, update_bytes: 0, checkpoint_bytes: size })
        } else {
            let meta = checked_bounds(&tx, 1, size)?;
            tx.prepare_cached("INSERT INTO updates(bytes) VALUES(?)")
                .and_then(|mut s| s.execute([&job.bytes]))
                .map_err(sqlite("append"))?;
            ("append", Metadata { rows: meta.rows + 1, update_bytes: meta.update_bytes + size, ..meta })
        };
        self.phase(&format!("{method}:uncommitted"))?;
        tx.commit().map_err(sqlite("commit"))?;
        self.phase(&format!("{method}:committed"))?;
        Ok(meta)
    }

    /// Runs a theme command against the stored overrides, under `theme::apply`'s rules.
    /// Changes are saved under ownership; a snapshot answers from what it read and
    /// refuses changes.
    pub fn theme(&self, defaults: &str, change: theme::Change) -> Result<theme::ThemeState> {
        let changes = !matches!(change, theme::Change::Get);
        self.check(changes)?;
        let backing = lock(&self.backing);
        let conn = match &*backing {
            Backing::Memory(saved) => return theme::apply(defaults, &saved.theme, change).map_err(Error::Rejected),
            Backing::Disk { conn, .. } => conn.as_ref().ok_or(Error::Closed)?,
        };
        let stored: String = conn
            .query_row("SELECT theme FROM document WHERE id=1", [], |r| r.get(0))
            .map_err(sqlite("read theme"))?;
        let applied = theme::apply(defaults, &stored, change).map_err(Error::Rejected)?;
        // A command that changes nothing writes nothing.
        if changes && applied.overrides != stored {
            if applied.overrides.len() > crate::wire::THEME_LIMIT {
                return Err(Error::Full);
            }
            let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("begin"))?;
            tx.execute("UPDATE document SET theme=? WHERE id=1", [&applied.overrides]).map_err(sqlite("save theme"))?;
            self.phase("theme:uncommitted")?;
            tx.commit().map_err(sqlite("commit"))?;
            self.phase("theme:committed")?;
        }
        Ok(applied)
    }

    /// Releases the database, then the writer lock. A failed close keeps ownership.
    pub fn close(&self) -> Result<()> {
        self.phase("close")?;
        let mut backing = lock(&self.backing);
        if let Backing::Disk { conn, lock } = &mut *backing {
            if let Some(open) = conn.take() {
                if let Err((open, e)) = open.close() {
                    *conn = Some(open);
                    return Err(failed(format!("close; retaining document ownership: {e}")));
                }
            }
            *lock = None;
        }
        Ok(())
    }
    /// The stored sizes; tests compare them with the file.
    pub fn metadata(&self) -> Result<Metadata> {
        match &*lock(&self.backing) {
            Backing::Memory(saved) => Ok(saved.metadata()),
            Backing::Disk { conn, .. } => Ok(bounds(conn.as_ref().ok_or(Error::Closed)?)?.0),
        }
    }
}

/// Copies a package's saved state into `destination_root/state/document.sqlite` as a new
/// logical document: the same Loro history and row identities, a new `doc_id`. The
/// source may be open in another process; the copy is SQLite's online backup.
pub fn duplicate(source_root: &Path, destination_root: &Path) -> Result<()> {
    // Resolve only the package root: NOFOLLOW still refuses replaced state entries.
    let source = fs::canonicalize(source_root)
        .map_err(|_| failed("Cannot resolve document for snapshot"))?
        .join("state/document.sqlite");
    let input = connect(&source, OpenFlags::SQLITE_OPEN_READ_ONLY, Duration::from_secs(5))?;
    // One read transaction: the backup copies exactly what was checked. Refuse what
    // opening would refuse, before anything is created.
    let read = Transaction::new_unchecked(&input, TransactionBehavior::Deferred).map_err(sqlite("read"))?;
    if is_new(&read)? {
        return Err(failed("Unsupported document storage"));
    }
    checked_bounds(&read, 0, 0)?;
    let state = state_directory(destination_root)?;
    let target = database(&state)?;
    let mut output = connect(&target, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE, Duration::from_secs(5))?;
    {
        let backup = rusqlite::backup::Backup::new(&read, &mut output).map_err(sqlite("Cannot start document snapshot"))?;
        match backup.step(-1).map_err(sqlite("Document snapshot failed"))? {
            rusqlite::backup::StepResult::Done => {}
            rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => return Err(Error::Busy),
            _ => return Err(failed("Document snapshot did not complete")),
        }
    }
    drop(read);
    let renewed = output
        .execute("UPDATE document SET doc_id=lower(hex(randomblob(16))) WHERE id=1", [])
        .map_err(sqlite("Cannot assign the duplicate a document identity"))?;
    if renewed != 1 {
        return Err(failed("Cannot assign the duplicate a document identity"));
    }
    output.close().map_err(|(_, e)| sqlite("close duplicate")(e))
}
