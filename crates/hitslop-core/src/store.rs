//! Durable storage for native hosts. A document is one SQLite file (`package` owns its
//! format); this module saves its state: one checkpoint (a Loro snapshot), the updates saved
//! after it, a small row with the document's identity and theme overrides, its attachments
//! and its artwork. The writer lock lives in the registry, outside the file. SQLite never
//! sees anything but opaque Loro bytes.
//!
//! History is trimmed when nothing is editing: a session that edited a document larger
//! than `TRIM_BYTES` closes with no history. While open, a checkpoint trims only past
//! `SESSION_BYTES`, keeping the session's history when that fits, so a concurrent text
//! edit can still branch from where the session opened. Compaction keeps no history.
//!
//! A host keeps two serial queues: edits, `Store::theme` and `Store::job` on one, every
//! other `Store` call on the other, so a slow write never blocks edits. A theme change is
//! held in memory like an edit and saved by the next job, in the same transaction.

use crate::file::{self, Kind, OpenedApp};
use crate::registry::Lease;
use crate::{theme, Document};
use loro::{ExportMode, Frontiers, VersionVector};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

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
/// The saved checkpoint, and the updates saved since, in order.
const CHECKPOINT: &str = "SELECT schema_key,bytes FROM checkpoint WHERE id=1";
const UPDATES: &str = "SELECT bytes FROM updates ORDER BY seq";
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
    /// The document file was moved or replaced while open.
    #[error("document moved or replaced")]
    Moved,
    /// The store no longer owns the document.
    #[error("document is closed")]
    Closed,
    /// The request breaks a document rule (a theme value, for example); nothing changed.
    #[error("{0}")]
    Rejected(crate::Error),
    #[error("{0}")]
    Failed(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn failed(message: impl ToString) -> Error {
    Error::Failed(message.to_string())
}
/// A refusal under a core error code; nothing changed.
pub(crate) fn rejected(code: crate::Code, message: impl ToString) -> Error {
    Error::Rejected(crate::err(code, message))
}
pub(crate) fn invalid(message: impl ToString) -> Error {
    rejected(crate::Code::InvalidRequest, message)
}
pub(crate) fn requires_update(message: impl ToString) -> Error {
    rejected(crate::Code::RequiresUpdate, message)
}
/// A SQLite failure as a storage outcome: full, busy, moved, or failed doing `action`.
pub(crate) fn sqlite(action: &str) -> impl FnOnce(rusqlite::Error) -> Error + '_ {
    use rusqlite::ffi::ErrorCode;
    move |e| match e.sqlite_error_code() {
        Some(ErrorCode::DiskFull | ErrorCode::TooBig) => Error::Full,
        // BUSY is a definite failure: nothing was written.
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Error::Busy,
        // Not a SQLite database at all: a file this app refuses, not a storage failure.
        Some(ErrorCode::NotADatabase) => invalid("This is not a hitSlop document"),
        // Apple's SQLite: the file was renamed, or gained and lost a hard link, while open.
        _ if e.sqlite_error().is_some_and(|e| e.extended_code == rusqlite::ffi::SQLITE_IOERR_VNODE) => Error::Moved,
        _ => failed(format!("{action}: {e}")),
    }
}
/// A core failure while loading: a document this build is too old for is a refusal the
/// host names; anything else is a storage failure.
fn load_failure(e: crate::Error) -> Error {
    if e.code == crate::Code::RequiresUpdate { Error::Rejected(e) } else { failed(e) }
}

/// Fault injection for tests at the real I/O boundary: `load`, `append:uncommitted`,
/// `append:committed`, `checkpoint:uncommitted`, `checkpoint:committed`,
/// `theme:uncommitted` and `theme:committed` (a job that saves only the theme), and
/// `close`. A hook may block or fail the call.
pub trait Phases: Send + Sync {
    fn reached(&self, phase: &str) -> Result<()>;
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
pub(crate) fn checked_bounds(conn: &Connection, rows: i64, bytes: i64) -> Result<Metadata> {
    let (meta, key) = bounds(conn)?;
    if meta.rows > 0 && meta.checkpoint_bytes == 0 {
        return Err(failed("Saved updates have no checkpoint; keep the file for recovery"));
    }
    if key > MAX_KEY_BYTES || meta.rows + rows > MAX_ROWS || meta.update_bytes + meta.checkpoint_bytes + bytes > MAX_BYTES {
        // A write that would cross the limits leaves saved state intact.
        if rows > 0 || bytes > 0 {
            return Err(Error::Full);
        }
        return Err(failed(format!(
            "Document exceeds storage limits ({} MiB or {MAX_ROWS} updates); keep the file for recovery",
            MAX_BYTES >> 20
        )));
    }
    Ok(meta)
}

/// Saved rows and theme overrides held in memory: a snapshot store's copy of the document,
/// and the saved-state marker it was read at.
struct Saved {
    checkpoint: Option<Vec<u8>>,
    schema_key: Option<String>,
    updates: Vec<Vec<u8>>,
    theme: String,
    marker: String,
}
impl Default for Saved {
    fn default() -> Self {
        Self { checkpoint: None, schema_key: None, updates: vec![], theme: NO_OVERRIDES.into(), marker: String::new() }
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
    Disk { conn: Option<Connection>, lease: Option<Lease> },
    Memory(Saved),
}
struct Account {
    meta: Metadata,
    /// The version the durable state covers.
    saved: VersionVector,
    /// The version this session opened at: where a checkpoint trimmed while open keeps
    /// history from, and how close tells whether the session edited.
    opened: Frontiers,
    schema_key: String,
}
/// The document's palette, held in memory from `document` on. `revision` counts accepted
/// changes; `saved` is the revision the durable state covers.
struct ThemeSlot {
    theme: theme::Theme,
    revision: u64,
    saved: u64,
}

/// One document's storage. `Document` mode owns the file: it holds the writer lock and
/// persists writes. `Snapshot` mode reads the saved rows once, without the lock; its
/// writes stay in memory, so a render never locks the file or changes what it holds (it
/// may finish rolling back a crashed write, as any reader does).
pub struct Store {
    path: PathBuf,
    /// The file's device and inode when opened; a rename or replacement is `Moved`.
    inode: (u64, u64),
    /// Whether this store opened as the writer (`Document` mode).
    writer: bool,
    doc_id: String,
    /// The app the document was built with, checked once when it opened.
    app: OpenedApp,
    backing: Mutex<Backing>,
    /// Whether this store holds the writer lock. Read without `backing`, which a save holds
    /// for its whole transaction, so checking ownership never waits for a save.
    owned: AtomicBool,
    account: Mutex<Account>,
    theme: Mutex<Option<ThemeSlot>>,
    phases: Mutex<Option<Arc<dyn Phases>>>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Document,
    Snapshot,
}

/// Bytes to persist, exported on the edit queue and written on the storage queue. A job
/// with no bytes saves only the theme.
pub struct SaveJob {
    checkpoint: bool,
    bytes: Vec<u8>,
    version: VersionVector,
    /// The overrides to save, and the theme revision they cover.
    theme: Option<(String, u64)>,
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
    let mut saved = conn.prepare_cached(CHECKPOINT).map_err(sqlite("read"))?;
    let mut saved = saved.query([]).map_err(sqlite("read"))?;
    let Some(row) = saved.next().map_err(sqlite("read"))? else {
        return Ok(None);
    };
    if !row.get_ref(0).ok().and_then(|key| key.as_str().ok()).is_some_and(|saved| crate::same_schema(saved, schema_key)) {
        return Err(failed("Document schema differs from saved state"));
    }
    let checkpoint = row.get_ref(1).ok().and_then(|bytes| bytes.as_blob().ok()).ok_or_else(|| failed("Invalid checkpoint bytes"))?;
    let mut rows = conn.prepare_cached(UPDATES).map_err(sqlite("read updates"))?;
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
        Err(e) => Err(fault.unwrap_or_else(|| load_failure(e))),
    }
}

impl Store {
    /// Opens the document file at `path`. `Document` mode takes the writer lock and checks
    /// the file, including SQLite's quick check; a template is never opened as a document
    /// (`file::create_document` makes one from it). `Snapshot` mode reads a document's
    /// saved state, or a template's initial values, once.
    pub fn open(path: &Path, mode: Mode) -> Result<Self> {
        let (backing, inode, doc_id, app) = match mode {
            Mode::Document => {
                let lease = Lease::acquire(path)?;
                let conn = file::writer(path, false)?;
                // Checked before anything is configured: a file this build refuses is never
                // written, not even its journal mode.
                let kind = file::check(&conn, true)?;
                if kind != Kind::Document {
                    return Err(rejected(crate::Code::IsTemplate, "A template opens by creating a document from it"));
                }
                file::configure_writer(&conn)?;
                let app = file::open_app(&conn, kind, path)?;
                let doc_id = conn
                    .query_row("SELECT doc_id FROM document WHERE id=1", [], |r| r.get(0))
                    .map_err(sqlite("read identity"))?;
                let inode = lease.file();
                (Backing::Disk { conn: Some(conn), lease: Some(lease) }, inode, doc_id, app)
            }
            Mode::Snapshot => {
                let (dev, ino, _) = crate::registry::identity(path).map_err(|_| failed("Cannot find the document"))?;
                let (saved, doc_id, app) = Self::read_snapshot(path)?;
                (Backing::Memory(saved), (dev, ino), doc_id, app)
            }
        };
        Ok(Self {
            path: path.to_owned(),
            inode,
            writer: mode == Mode::Document,
            doc_id,
            app,
            owned: AtomicBool::new(matches!(backing, Backing::Disk { .. })),
            backing: Mutex::new(backing),
            account: Mutex::new(Account {
                meta: Metadata::default(),
                saved: VersionVector::default(),
                opened: Frontiers::default(),
                schema_key: String::new(),
            }),
            theme: Mutex::new(None),
            phases: Mutex::new(None),
        })
    }
    /// The saved rows, read in one read transaction so validation and the copy see the
    /// same saved state. Limits are checked before any blob is read. A template has none.
    fn read_snapshot(path: &Path) -> Result<(Saved, String, OpenedApp)> {
        let conn = file::reader(path)?;
        let read = Transaction::new_unchecked(&conn, TransactionBehavior::Deferred).map_err(sqlite("read"))?;
        let kind = file::check(&read, false)?;
        let app = file::open_app(&read, kind, path)?;
        if kind == Kind::Template {
            return Ok((Saved::default(), file::new_doc_id(), app));
        }
        checked_bounds(&read, 0, 0)?;
        let marker = marker(&read)?;
        // The theme is read with the rows, so a render sees one saved state.
        let (doc_id, theme) = read
            .query_row("SELECT doc_id,theme FROM document WHERE id=1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(sqlite("read"))?;
        let (schema_key, checkpoint) = read
            .query_row(CHECKPOINT, [], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(sqlite("read"))?
            .unzip();
        let mut updates = vec![];
        let mut rows = read.prepare(UPDATES).map_err(sqlite("read updates"))?;
        let mut rows = rows.query([]).map_err(sqlite("read updates"))?;
        while let Some(row) = rows.next().map_err(sqlite("read updates"))? {
            updates.push(row.get(0).map_err(sqlite("read updates"))?);
        }
        Ok((Saved { checkpoint, schema_key, updates, theme, marker }, doc_id, app))
    }

    pub fn doc_id(&self) -> &str {
        &self.doc_id
    }
    /// The app this store's file holds, as its open checked it.
    pub fn app(&self) -> &OpenedApp {
        &self.app
    }
    pub fn set_phases(&self, phases: Option<Arc<dyn Phases>>) {
        *lock(&self.phases) = phases;
    }
    fn phase(&self, name: &str) -> Result<()> {
        let hook = lock(&self.phases).clone();
        hook.map_or(Ok(()), |hook| hook.reached(name))
    }
    /// Fails once the file was moved or replaced (or, for the writer, gained a hard link),
    /// or, with `writable`, once the store no longer owns it.
    pub fn check(&self, writable: bool) -> Result<()> {
        // A stat, never the backing mutex a save holds: a theme change never waits for one.
        match crate::registry::identity(&self.path) {
            Ok((dev, ino, links)) if (dev, ino) == self.inode && (links == 1 || !self.writer) => {}
            _ => return Err(Error::Moved),
        }
        if writable && !self.owned.load(Ordering::Acquire) {
            return Err(Error::Closed);
        }
        Ok(())
    }

    /// The saved document and its palette over the app's theme defaults. A document without
    /// a checkpoint starts from the app's initial values and saves its first checkpoint.
    /// Also the reload after discarding unsaved edits, which discards unsaved theme changes
    /// too.
    pub fn document(&self) -> Result<Document> {
        self.phase("load")?;
        self.check(false)?;
        let core = load_failure;
        let schema_key = &self.app.schema_key;
        let (initial, theme_defaults) = (&self.app.app.initial, &self.app.app.theme);
        let mut backing = lock(&self.backing);
        let stored = match &*backing {
            Backing::Memory(saved) => saved.theme.clone(),
            Backing::Disk { conn, .. } => conn
                .as_ref()
                .ok_or(Error::Closed)?
                .query_row("SELECT theme FROM document WHERE id=1", [], |r| r.get(0))
                .map_err(sqlite("read theme"))?,
        };
        let theme = theme::Theme::new(theme_defaults, &stored).map_err(Error::Rejected)?;
        *lock(&self.theme) = Some(ThemeSlot { theme, revision: 0, saved: 0 });
        let (doc, meta) = match &mut *backing {
            Backing::Memory(saved) => {
                if saved.checkpoint.is_some() && !saved.schema_key.as_deref().is_some_and(|saved| crate::same_schema(saved, schema_key)) {
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
                        saved.checkpoint = Some(checkpoint);
                        saved.schema_key = Some(schema_key.into());
                        saved.updates.clear();
                        (doc, meta)
                    }
                }
            }
            Backing::Disk { conn, .. } => match load(conn.as_ref().ok_or(Error::Closed)?, schema_key)? {
                Some(loaded) => loaded,
                None => {
                    drop(backing);
                    let doc = Document::create(schema_key, initial).map_err(core)?;
                    let job = SaveJob { checkpoint: true, bytes: doc.checkpoint().map_err(core)?, version: doc.doc.oplog_vv(), theme: None };
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
            Account { meta, saved: doc.doc.oplog_vv(), opened: doc.doc.oplog_frontiers(), schema_key: schema_key.clone() };
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
        let theme = lock(&self.theme)
            .as_ref()
            .filter(|slot| slot.revision > slot.saved)
            .map(|slot| slot.theme.overrides().map(|json| (json, slot.revision)))
            .transpose()
            .map_err(Error::Rejected)?;
        if version == saved && !force_checkpoint {
            return Ok(theme.map(|theme| SaveJob { checkpoint: false, bytes: vec![], version, theme: Some(theme) }));
        }
        let job = |checkpoint, bytes| SaveJob { checkpoint, bytes, version: version.clone(), theme: theme.clone() };
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

    /// The checkpoint to write as the owner closes, after its last save: a session that
    /// edited a document larger than `TRIM_BYTES` leaves no history. Undo covers the open
    /// session only, so nothing reads it later, and a cut before the latest version would
    /// keep, in its starting state, everything deleted before it (Loro 1.16.2). None when
    /// nothing would shrink.
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
        let bytes = trimmed(doc, &latest, smaller)?;
        Ok(bytes.map(|bytes| SaveJob { checkpoint: true, bytes, version, theme: None }))
    }

    /// Writes a job in one transaction. An error may follow the commit, so the durable
    /// version advances only on success; sizes are re-read either way.
    pub fn write(&self, job: &SaveJob) -> Result<()> {
        self.check(false).map_err(|_| Error::Moved)?;
        if job.bytes.is_empty() && job.theme.is_none() {
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
                } else if !job.bytes.is_empty() {
                    saved.updates.push(job.bytes.clone());
                }
                if let Some((theme, _)) = &job.theme {
                    saved.theme = theme.clone();
                }
                saved.metadata()
            }
            Backing::Disk { conn, .. } => match self.on_writer(conn, |conn| self.transaction(conn, job, &schema_key)) {
                Ok(meta) => meta,
                Err(e) => {
                    if let Some(Ok((meta, _))) = conn.as_ref().map(bounds) {
                        lock(&self.account).meta = meta;
                    }
                    return Err(e);
                }
            },
        };
        let mut account = lock(&self.account);
        account.meta = meta;
        account.saved = job.version.clone();
        drop(account);
        if let (Some((_, revision)), Some(slot)) = (&job.theme, lock(&self.theme).as_mut()) {
            slot.saved = slot.saved.max(*revision);
        }
        Ok(())
    }
    /// Runs a write on the writer's connection. Apple's SQLite stops a connection from
    /// writing for good once its file is renamed, or gains and loses a hard link, even after
    /// the file is back (`Moved`). When the path again names this store's file, the writer
    /// reconnects and the write runs once more; the lease never left the file.
    fn on_writer<T>(&self, conn: &mut Option<Connection>, work: impl Fn(&Connection) -> Result<T>) -> Result<T> {
        match work(conn.as_ref().ok_or(Error::Closed)?) {
            Err(Error::Moved) if self.check(false).is_ok() => {
                let fresh = file::writer(&self.path, false)?;
                file::configure_writer(&fresh)?;
                // Replacing closes the stale connection, which holds no transaction.
                work(conn.insert(fresh))
            }
            other => other,
        }
    }
    /// Dropping the guard rolls back: on any error, after a failed COMMIT, or in a panic.
    fn transaction(&self, conn: &Connection, job: &SaveJob, schema_key: &str) -> Result<Metadata> {
        let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("begin"))?;
        let size = job.bytes.len() as i64;
        let (method, meta) = if job.bytes.is_empty() {
            ("theme", lock(&self.account).meta)
        } else if job.checkpoint {
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
        if let Some((theme, _)) = &job.theme {
            tx.prepare_cached("UPDATE document SET theme=? WHERE id=1")
                .and_then(|mut s| s.execute([theme]))
                .map_err(sqlite("save theme"))?;
        }
        self.phase(&format!("{method}:uncommitted"))?;
        tx.commit().map_err(sqlite("commit"))?;
        self.phase(&format!("{method}:committed"))?;
        Ok(meta)
    }

    /// Runs a theme command against the palette held in memory, under the palette rules.
    /// A change is saved by the next job; a snapshot answers from what it read and refuses
    /// changes. Returns the theme and whether the command changed it.
    pub fn theme(&self, change: theme::Change) -> Result<(theme::ThemeState, bool)> {
        if !matches!(change, theme::Change::Get) {
            self.check(true)?;
        }
        let mut slot = lock(&self.theme);
        let slot = slot.as_mut().ok_or(Error::Closed)?;
        let changed = slot.theme.change(change).map_err(Error::Rejected)?;
        if changed {
            slot.revision += 1;
        }
        Ok((slot.theme.state().map_err(Error::Rejected)?, changed))
    }
    /// Whether an accepted theme change is not yet durable.
    pub fn theme_unsaved(&self) -> bool {
        lock(&self.theme).as_ref().is_some_and(|slot| slot.revision > slot.saved)
    }
    /// The palette as a theme file for `template` (see `theme::Theme::export`).
    pub fn export_theme(&self, template: &str) -> Result<String> {
        lock(&self.theme).as_ref().ok_or(Error::Closed)?.theme.export(template).map_err(Error::Rejected)
    }

    /// Releases the database, withdraws discovery, then releases the writer lock. A failed
    /// close keeps ownership.
    pub fn close(&self) -> Result<()> {
        self.phase("close")?;
        let mut backing = lock(&self.backing);
        if let Backing::Disk { conn, lease } = &mut *backing {
            if let Some(open) = conn.take() {
                if let Err((open, e)) = open.close() {
                    *conn = Some(open);
                    return Err(failed(format!("close; retaining document ownership: {e}")));
                }
            }
            self.owned.store(false, Ordering::Release);
            if let Some(lease) = lease.take() {
                lease.withdraw();
            }
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

    // Attachments: content-addressed immutable blobs in the document file, keyed by their
    // SHA-256. The writer stores them on the storage queue; a reader reads them through its
    // own connection.

    /// Stores `bytes` and returns its reference; storing the same bytes again is a no-op.
    /// The blob is committed before any edit can save a reference to it.
    pub fn put_attachment(&self, bytes: &[u8]) -> Result<Attachment> {
        self.check(true)?;
        if bytes.len() > crate::ATTACHMENT_FILE_BYTES {
            return Err(rejected(crate::Code::TooLarge, format!("Attachment exceeds {} MiB", crate::ATTACHMENT_FILE_BYTES >> 20)));
        }
        let id = attachment_id(bytes);
        let mut backing = lock(&self.backing);
        let Backing::Disk { conn, .. } = &mut *backing else { return Err(Error::Closed) };
        self.on_writer(conn, |conn| self.store_attachment(conn, &id, bytes))?;
        Ok(Attachment { id, bytes: bytes.len() as u64 })
    }
    fn store_attachment(&self, conn: &Connection, id: &str, bytes: &[u8]) -> Result<()> {
        let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("store attachment"))?;
        let stored: Option<Vec<u8>> =
            tx.query_row("SELECT bytes FROM attachments WHERE id=?", [&id], |r| r.get(0)).optional().map_err(sqlite("store attachment"))?;
        // The same bytes are already stored only if the stored copy is intact; damage is
        // refused, never repaired in passing, so a successful import is always readable.
        if stored.as_deref().is_some_and(|stored| attachment_id(stored) != id) {
            return Err(failed("Attachment checksum mismatch; keep the file for recovery"));
        }
        if stored.is_none() {
            let (count, total): (i64, i64) = tx
                .query_row("SELECT count(*), coalesce(sum(length(bytes)),0) FROM attachments", [], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(sqlite("store attachment"))?;
            if count + 1 > crate::ATTACHMENT_COUNT as i64 || total + bytes.len() as i64 > crate::ATTACHMENT_BYTES as i64 {
                return Err(rejected(
                    crate::Code::TooLarge,
                    format!("Document attachment limit reached ({} MiB or {} files)", crate::ATTACHMENT_BYTES >> 20, crate::ATTACHMENT_COUNT),
                ));
            }
            tx.execute("INSERT INTO attachments VALUES(?,?)", params![id, bytes]).map_err(sqlite("store attachment"))?;
        }
        tx.commit().map_err(sqlite("store attachment"))
    }
    /// An attachment's bytes, verified against its identity.
    pub fn attachment(&self, id: &str) -> Result<Vec<u8>> {
        self.check(false)?;
        if !valid_attachment_id(id) {
            return Err(rejected(crate::Code::InvalidId, "Invalid attachment ID"));
        }
        let bytes: Option<Vec<u8>> = self.read(|conn| {
            conn.query_row("SELECT bytes FROM attachments WHERE id=?", [id], |r| r.get(0)).optional().map_err(sqlite("read attachment"))
        })?;
        let bytes = bytes.ok_or_else(|| rejected(crate::Code::PathNotFound, "Attachment not found"))?;
        if attachment_id(&bytes) != id {
            return Err(failed("Attachment checksum mismatch; keep the file for recovery"));
        }
        Ok(bytes)
    }
    /// One artwork image (`preview` or `icon`), through this store's own connection.
    pub fn artwork(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.check(false)?;
        self.read(|conn| conn.query_row("SELECT png FROM artwork WHERE name=?", [name], |r| r.get(0)).optional().map_err(sqlite("read artwork")))
    }
    /// Every stored attachment, by identity.
    pub fn attachments(&self) -> Result<Vec<Attachment>> {
        self.check(false)?;
        self.read(|conn| {
            let mut statement = conn.prepare("SELECT id, length(bytes) FROM attachments ORDER BY id").map_err(sqlite("list attachments"))?;
            let rows = statement
                .query_map([], |r| Ok(Attachment { id: r.get(0)?, bytes: r.get::<_, i64>(1)? as u64 }))
                .map_err(sqlite("list attachments"))?;
            rows.collect::<rusqlite::Result<_>>().map_err(sqlite("list attachments"))
        })
    }
    /// Runs a read on the writer's connection, or, for a snapshot, on a reader of its own.
    fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let backing = lock(&self.backing);
        match &*backing {
            Backing::Disk { conn, .. } => f(conn.as_ref().ok_or(Error::Closed)?),
            Backing::Memory(_) => {
                drop(backing);
                f(&file::reader(&self.path)?)
            }
        }
    }

    /// What the saved state was when a snapshot read it. Artwork rendered from it is written
    /// only while the document still holds that state (`write_artwork`).
    pub fn saved_marker(&self) -> Result<String> {
        match &*lock(&self.backing) {
            Backing::Memory(saved) => Ok(saved.marker.clone()),
            Backing::Disk { conn, .. } => marker(conn.as_ref().ok_or(Error::Closed)?),
        }
    }

    /// Names the live owner for clients: a writer publishes its socket in the registry.
    pub fn publish_discovery(&self, json: &str) -> Result<()> {
        match &*lock(&self.backing) {
            Backing::Disk { lease: Some(lease), .. } => lease.publish(json),
            _ => Err(Error::Closed),
        }
    }
    pub fn withdraw_discovery(&self) {
        if let Backing::Disk { lease: Some(lease), .. } = &*lock(&self.backing) {
            lease.withdraw();
        }
    }

    /// Copies the open document to `dest` as a new logical document, from the writer's own
    /// connection, so saves queue behind the copy instead of timing out. Duplicate and
    /// Share a copy use it after flushing.
    pub fn copy_to(&self, dest: &Path) -> Result<()> {
        self.check(true)?;
        let backing = lock(&self.backing);
        let Backing::Disk { conn: Some(conn), .. } = &*backing else { return Err(Error::Closed) };
        file::copy(conn, dest, false)
    }
}

/// A stored attachment: its identity (the SHA-256 of its bytes) and size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub id: String,
    pub bytes: u64,
}
fn attachment_id(bytes: &[u8]) -> String {
    crate::hex(Sha256::digest(bytes).as_slice())
}
/// An attachment's identity: its SHA-256 in lowercase hex.
pub(crate) fn valid_attachment_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// The saved state, summarized: identity, theme, the log and the checkpoint's size and
/// ends. Any save changes it.
fn marker(conn: &Connection) -> Result<String> {
    conn.query_row(
        "SELECT (SELECT doc_id || ':' || theme FROM document WHERE id=1),
                (SELECT coalesce(max(seq),0) || ':' || count(*) || ':' || coalesce(sum(length(bytes)),0) FROM updates),
                (SELECT coalesce(length(bytes),0) || ':' || coalesce(hex(substr(bytes,1,32)),'') || ':' || coalesce(hex(substr(bytes,-32)),'') FROM checkpoint)",
        [],
        |r| Ok(format!("{}|{}|{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?.unwrap_or_default())),
    )
    .map_err(sqlite("read saved state"))
}

/// The first of `preferred` artwork (`preview`, `icon`) the file holds, by name: one read,
/// for a host displaying the file (Quick Look, the catalog, a window's icon). A file that
/// cannot be read now (busy, or mid-recovery) is an error, never "no artwork".
pub fn artwork(path: &Path, preferred: &[&str]) -> Result<Option<(String, Vec<u8>)>> {
    let conn = file::reader(path)?;
    file::check(&conn, false)?;
    for name in preferred {
        let png = conn.query_row("SELECT png FROM artwork WHERE name=?", [name], |r| r.get(0)).optional().map_err(sqlite("read artwork"))?;
        if let Some(png) = png {
            return Ok(Some((name.to_string(), png)));
        }
    }
    Ok(None)
}

/// Writes refreshed artwork into a closed document, but only while it still holds the
/// saved state the artwork was rendered from (`marker`). Returns false, writing nothing,
/// when the document is open elsewhere or has changed. Artwork never fails a save.
pub fn write_artwork(path: &Path, marker_at_render: &str, artwork: &[(&str, &[u8])]) -> Result<bool> {
    let _lease = match Lease::acquire(path) {
        Ok(lease) => lease,
        Err(Error::Locked) => return Ok(false),
        Err(e) => return Err(e),
    };
    for (name, png) in artwork {
        file::check_artwork(&format!("The {name} artwork"), png)?;
    }
    let conn = file::writer(path, false)?;
    if file::check(&conn, false)? != Kind::Document {
        return Ok(false);
    }
    file::configure_writer(&conn)?;
    let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate).map_err(sqlite("write artwork"))?;
    if marker(&tx)? != marker_at_render {
        return Ok(false);
    }
    for (name, png) in artwork {
        tx.execute("INSERT INTO artwork VALUES(?,?) ON CONFLICT(name) DO UPDATE SET png=excluded.png", params![name, png])
            .map_err(sqlite("write artwork"))?;
    }
    tx.commit().map_err(sqlite("write artwork"))?;
    Ok(true)
}
