//! Durable storage for native hosts. A document is one SQLite file (`file` owns its
//! format); this module saves its state: one checkpoint (a Loro snapshot), the updates saved
//! after it, a small row with the document's theme overrides, its attachments
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
use std::sync::{Mutex, MutexGuard};

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
const MAX_DESCRIPTOR_BYTES: i64 = 1024 * 1024;
/// The overrides of a document that has none.
const NO_OVERRIDES: &str = "{}";
/// The saved checkpoint, and the updates saved since, in order.
const CHECKPOINT: &str = "SELECT descriptor,bytes FROM checkpoint WHERE id=1";
const UPDATES: &str = "SELECT bytes FROM updates ORDER BY seq";
const BOUNDS: &str = "SELECT (SELECT count(*) FROM updates),(SELECT COALESCE(sum(length(bytes)),0) FROM updates),COALESCE((SELECT length(bytes) FROM checkpoint),0),COALESCE((SELECT length(CAST(descriptor AS BLOB)) FROM checkpoint),0)";

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

/// Stored sizes, refreshed by every write, so choosing append or checkpoint needs no
/// database read.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Metadata {
    rows: i64,
    update_bytes: i64,
    checkpoint_bytes: i64,
}
impl Metadata {
    /// The checkpoint and update bytes the document holds.
    fn stored(&self) -> i64 {
        self.checkpoint_bytes + self.update_bytes
    }
}
/// The storage limits every size decision checks: at most `MAX_ROWS` saved updates and
/// `MAX_BYTES` of checkpoint and updates.
fn within(rows: i64, bytes: i64) -> bool {
    rows <= MAX_ROWS && bytes <= MAX_BYTES
}
/// A checkpoint row's size: SQLite bounds the whole row, its descriptor and header too.
fn checkpoint_row(bytes: usize, descriptor: usize) -> i64 {
    (bytes + descriptor) as i64 + 512
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
/// The stored sizes, checked against the limits. Read before any blob is.
fn checked_bounds(conn: &Connection) -> Result<Metadata> {
    let (meta, descriptor) = bounds(conn)?;
    if meta.rows > 0 && meta.checkpoint_bytes == 0 {
        return Err(failed("Saved updates have no checkpoint; keep the file for recovery"));
    }
    if descriptor > MAX_DESCRIPTOR_BYTES || !within(meta.rows, meta.stored()) {
        return Err(failed(format!(
            "Document exceeds storage limits ({} MiB or {MAX_ROWS} updates); keep the file for recovery",
            MAX_BYTES >> 20
        )));
    }
    Ok(meta)
}

/// The store's connection and, for the writer, its lock. Field order matters: the
/// connection closes before the lock is released.
struct Backing {
    conn: Option<Connection>,
    lease: Option<Lease>,
}
struct Account {
    meta: Metadata,
    /// The version the durable state covers.
    saved: VersionVector,
    /// The version this session opened at: where a checkpoint trimmed while open keeps
    /// history from, and how close tells whether the session edited.
    opened: Frontiers,
}
/// The document's palette, held in memory from `document` on. `revision` counts accepted
/// changes; `saved` is the revision the durable state covers.
struct ThemeSlot {
    theme: theme::Theme,
    revision: u64,
    saved: u64,
}

/// One document's storage. `Document` mode owns the file: it holds the writer lock and
/// persists writes. `Snapshot` mode reads the saved state without the lock and writes
/// nothing, so a render never locks the file or changes what it holds (it may finish
/// rolling back a crashed write, as any reader does).
pub struct Store {
    path: PathBuf,
    /// The file's device and inode when opened; a rename or replacement is `Moved`.
    inode: (u64, u64),
    /// The app the document was built with, checked once when it opened.
    app: OpenedApp,
    backing: Mutex<Backing>,
    /// Whether this store holds the writer lock. Read without `backing`, which a save holds
    /// for its whole transaction, so checking ownership never waits for a save.
    owned: AtomicBool,
    account: Mutex<Account>,
    theme: Mutex<Option<ThemeSlot>>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Document,
    Snapshot,
}

/// What a save writes, exported on the edit queue and written on the storage queue.
pub struct SaveJob {
    rows: Rows,
    version: VersionVector,
    /// The overrides to save, and the theme revision they cover.
    theme: Option<(String, u64)>,
}
/// A save's Loro bytes: none (it saves only the theme), the updates since the last save,
/// or a checkpoint that replaces the log.
enum Rows {
    None,
    Append(Vec<u8>),
    Checkpoint(Vec<u8>),
}
impl SaveJob {
    pub fn is_checkpoint(&self) -> bool {
        matches!(self.rows, Rows::Checkpoint(_))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The updates since `saved`, when appending them keeps the log within the limits.
fn append(doc: &Document, saved: &VersionVector, meta: Metadata) -> Result<Option<Vec<u8>>> {
    let bytes = doc.doc.export(ExportMode::updates(saved)).map_err(failed)?;
    Ok(within(meta.rows + 1, meta.stored() + bytes.len() as i64).then_some(bytes))
}
/// A checkpoint that fits: the whole history while it is small, else the session's own
/// history, else none. Compaction keeps no history.
fn checkpoint(doc: &mut Document, opened: &Frontiers, descriptor: usize, compact: bool) -> Result<Option<Vec<u8>>> {
    let fits = |bytes: &[u8]| within(0, checkpoint_row(bytes.len(), descriptor));
    let latest = doc.doc.oplog_frontiers();
    if compact {
        return trimmed(doc, &latest, fits);
    }
    let bytes = doc.doc.export(ExportMode::Snapshot).map_err(failed)?;
    if bytes.len() <= SESSION_BYTES && fits(&bytes) {
        return Ok(Some(bytes));
    }
    match trimmed(doc, opened, |b| b.len() <= SESSION_BYTES && fits(b))? {
        Some(bytes) => Ok(Some(bytes)),
        None => trimmed(doc, &latest, fits),
    }
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
/// update are imported straight from SQLite's buffers, without copying them. The saved
/// descriptor is compared by meaning, whatever its spelling.
fn load(conn: &Connection, schema: &crate::Node) -> Result<Option<(Document, Metadata)>> {
    let meta = checked_bounds(conn)?;
    let mut saved = conn.prepare_cached(CHECKPOINT).map_err(sqlite("read"))?;
    let mut saved = saved.query([]).map_err(sqlite("read"))?;
    let Some(row) = saved.next().map_err(sqlite("read"))? else {
        return Ok(None);
    };
    if !row.get_ref(0).ok().and_then(|key| key.as_str().ok()).is_some_and(|saved| crate::descriptor(saved).is_ok_and(|saved| saved == *schema)) {
        return Err(failed("Document schema differs from saved state"));
    }
    let checkpoint = row.get_ref(1).ok().and_then(|bytes| bytes.as_blob().ok()).ok_or_else(|| failed("Invalid checkpoint bytes"))?;
    let mut rows = conn.prepare_cached(UPDATES).map_err(sqlite("read updates"))?;
    let mut rows = rows.query([]).map_err(sqlite("read updates"))?;
    let doc = Document::open_with(schema.clone(), checkpoint, load_failure, |import| loop {
        match rows.next().map_err(sqlite("read updates"))? {
            None => return Ok(()),
            Some(row) => import(row.get_ref(0).ok().and_then(|v| v.as_blob().ok()).ok_or_else(|| failed("Invalid update bytes"))?)?,
        }
    })?;
    Ok(Some((doc, meta)))
}

impl Store {
    /// Opens the document file at `path`. `Document` mode takes the writer lock and checks
    /// the file, including SQLite's quick check; a template is never opened as a document
    /// (`file::create_document` makes one from it). `Snapshot` mode reads a document's
    /// saved state, or a template's initial values, once.
    pub fn open(path: &Path, mode: Mode) -> Result<Self> {
        let (lease, conn, kind) = match mode {
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
                (Some(lease), conn, kind)
            }
            Mode::Snapshot => {
                let conn = file::reader(path)?;
                let kind = file::check(&conn, false)?;
                (None, conn, kind)
            }
        };
        let inode = match &lease {
            Some(lease) => lease.file(),
            None => crate::registry::identity(path).map(|(dev, ino, _)| (dev, ino)).map_err(|_| failed("Cannot find the document"))?,
        };
        Ok(Self {
            path: path.to_owned(),
            inode,
            app: file::open_app(&conn, kind, path)?,
            owned: AtomicBool::new(lease.is_some()),
            backing: Mutex::new(Backing { conn: Some(conn), lease }),
            account: Mutex::new(Account { meta: Metadata::default(), saved: VersionVector::default(), opened: Frontiers::default() }),
            theme: Mutex::new(None),
        })
    }

    /// The app this store's file holds, as its open checked it.
    pub fn app(&self) -> &OpenedApp {
        &self.app
    }
    /// Fails once the file was moved or replaced (or, for the writer, gained a hard link),
    /// or, with `writable`, once the store no longer owns it.
    pub fn check(&self, writable: bool) -> Result<()> {
        // A stat, never the backing mutex a save holds: a theme change never waits for one.
        let owned = self.owned.load(Ordering::Acquire);
        match crate::registry::identity(&self.path) {
            Ok((dev, ino, links)) if (dev, ino) == self.inode && (links == 1 || !owned) => {}
            _ => return Err(Error::Moved),
        }
        if writable && !owned {
            return Err(Error::Closed);
        }
        Ok(())
    }

    /// The saved document and its palette over the app's theme defaults, read in one read
    /// transaction so a render sees one saved state. A document without a checkpoint starts
    /// from the app's initial values, which the writer saves as its first checkpoint. Also
    /// the reload after discarding unsaved edits, which discards unsaved theme changes too.
    pub fn document(&self) -> Result<Document> {
        self.check(false)?;
        let (theme, loaded) = self.connected(&mut lock(&self.backing).conn, |conn| {
            let read = Transaction::new_unchecked(conn, TransactionBehavior::Deferred).map_err(sqlite("read"))?;
            let stored = read
                .query_row("SELECT theme FROM document WHERE id=1", [], |r| r.get::<_, String>(0))
                .optional()
                .map_err(sqlite("read theme"))?;
            let theme = theme::Theme::new(&self.app.slug, &self.app.theme_tokens, stored.as_deref().unwrap_or(NO_OVERRIDES))
                .map_err(Error::Rejected)?;
            Ok((theme, load(&read, &self.app.schema)?))
        })?;
        *lock(&self.theme) = Some(ThemeSlot { theme, revision: 0, saved: 0 });
        let (doc, meta) = match loaded {
            Some(loaded) => loaded,
            None => {
                let doc = Document::create_with(self.app.schema.clone(), &self.app.app.initial).map_err(load_failure)?;
                lock(&self.account).opened = doc.doc.oplog_frontiers();
                if self.owned.load(Ordering::Acquire) {
                    let bytes = doc.checkpoint().map_err(load_failure)?;
                    self.write(&SaveJob { rows: Rows::Checkpoint(bytes), version: doc.doc.oplog_vv(), theme: None })?;
                }
                return Ok(doc);
            }
        };
        *lock(&self.account) = Account { meta, saved: doc.doc.oplog_vv(), opened: doc.doc.oplog_frontiers() };
        Ok(doc)
    }

    /// The next write for `doc`, or none when the durable state already covers it and no
    /// checkpoint is requested. Exports only what it writes: the updates since the last
    /// save, or a checkpoint once the log is long, full or a checkpoint is requested.
    pub fn job(&self, doc: &mut Document, force_checkpoint: bool) -> Result<Option<SaveJob>> {
        let (meta, saved, opened) = {
            let account = lock(&self.account);
            (account.meta, account.saved.clone(), account.opened.clone())
        };
        let version = doc.doc.oplog_vv();
        let theme = lock(&self.theme).as_ref().filter(|slot| slot.revision > slot.saved).map(|slot| (slot.theme.overrides(), slot.revision));
        if version == saved && !force_checkpoint {
            return Ok(theme.map(|theme| SaveJob { rows: Rows::None, version, theme: Some(theme) }));
        }
        // A checkpoint first when one is requested or the log is due for one; optional
        // maintenance never prevents an append that still fits.
        let due = meta.rows >= CHECKPOINT_ROWS || meta.update_bytes >= CHECKPOINT_BYTES;
        let order: &[bool] = if force_checkpoint { &[true] } else if due { &[true, false] } else { &[false, true] };
        for &as_checkpoint in order {
            let bytes = if as_checkpoint {
                checkpoint(doc, &opened, self.app.canonical_descriptor.len(), force_checkpoint)?
            } else {
                append(doc, &saved, meta)?
            };
            if let Some(bytes) = bytes {
                let rows = if as_checkpoint { Rows::Checkpoint(bytes) } else { Rows::Append(bytes) };
                return Ok(Some(SaveJob { rows, version, theme }));
            }
        }
        Err(Error::Full)
    }

    /// The checkpoint to write as the owner closes, after its last save: a session that
    /// edited a document larger than `TRIM_BYTES` leaves no history. Undo covers the open
    /// session only, so nothing reads it later, and a cut before the latest version would
    /// keep, in its starting state, everything deleted before it (Loro 1.16.2). None when
    /// nothing would shrink.
    pub fn close_job(&self, doc: &mut Document) -> Result<Option<SaveJob>> {
        let (meta, opened) = {
            let account = lock(&self.account);
            (account.meta, account.opened.clone())
        };
        let stored = meta.stored();
        let latest = doc.doc.oplog_frontiers();
        if latest == opened || stored <= TRIM_BYTES {
            return Ok(None);
        }
        let descriptor = self.app.canonical_descriptor.len();
        let smaller = |bytes: &[u8]| (bytes.len() as i64) < stored && within(0, checkpoint_row(bytes.len(), descriptor));
        let version = doc.doc.oplog_vv();
        let bytes = trimmed(doc, &latest, smaller)?;
        Ok(bytes.map(|bytes| SaveJob { rows: Rows::Checkpoint(bytes), version, theme: None }))
    }

    /// Writes a job in one transaction. An error may follow the commit, so the durable
    /// version advances only on success; sizes are re-read either way.
    pub fn write(&self, job: &SaveJob) -> Result<()> {
        self.check(true)?;
        let mut backing = lock(&self.backing);
        let meta = match self.connected(&mut backing.conn, |conn| self.transaction(conn, job)) {
            Ok(meta) => meta,
            Err(e) => {
                if let Some(Ok((meta, _))) = backing.conn.as_ref().map(bounds) {
                    lock(&self.account).meta = meta;
                }
                return Err(e);
            }
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
    /// Runs `work` on the store's connection. Apple's SQLite stops a connection for good
    /// once its file is renamed, or gains and loses a hard link, even after the file is back
    /// (`Moved`). When the path again names this store's file, the store reconnects and the
    /// work runs once more; a writer's lease never left the file.
    fn connected<T>(&self, conn: &mut Option<Connection>, work: impl Fn(&Connection) -> Result<T>) -> Result<T> {
        match work(conn.as_ref().ok_or(Error::Closed)?) {
            Err(Error::Moved) if self.check(false).is_ok() => {
                let fresh = if self.owned.load(Ordering::Acquire) {
                    let writer = file::writer(&self.path, false)?;
                    file::configure_writer(&writer)?;
                    writer
                } else {
                    file::reader(&self.path)?
                };
                // Replacing closes the stale connection, which holds no transaction.
                work(conn.insert(fresh))
            }
            other => other,
        }
    }
    /// Dropping the guard rolls back: on any error, after a failed COMMIT, or in a panic.
    fn transaction(&self, conn: &Connection, job: &SaveJob) -> Result<Metadata> {
        let descriptor = &self.app.canonical_descriptor;
        let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("begin"))?;
        let meta = match &job.rows {
            Rows::None => lock(&self.account).meta,
            Rows::Checkpoint(bytes) => {
                if !within(0, checkpoint_row(bytes.len(), descriptor.len())) || descriptor.len() as i64 > MAX_DESCRIPTOR_BYTES {
                    return Err(Error::Full);
                }
                tx.prepare_cached("INSERT INTO checkpoint VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET descriptor=excluded.descriptor,bytes=excluded.bytes")
                    .and_then(|mut s| s.execute(params![descriptor, bytes]))
                    .map_err(sqlite("checkpoint"))?;
                tx.execute_batch("DELETE FROM updates").map_err(sqlite("checkpoint"))?;
                // Each step frees one page.
                tx.prepare_cached("PRAGMA incremental_vacuum")
                    .and_then(|mut s| s.query([])?.mapped(|_| Ok(())).collect::<rusqlite::Result<()>>())
                    .map_err(sqlite("checkpoint"))?;
                Metadata { rows: 0, update_bytes: 0, checkpoint_bytes: bytes.len() as i64 }
            }
            Rows::Append(bytes) => {
                // A write that would cross the limits leaves saved state intact.
                let meta = checked_bounds(&tx)?;
                let size = bytes.len() as i64;
                if !within(meta.rows + 1, meta.stored() + size) {
                    return Err(Error::Full);
                }
                tx.prepare_cached("INSERT INTO updates(bytes) VALUES(?)")
                    .and_then(|mut s| s.execute([bytes]))
                    .map_err(sqlite("append"))?;
                Metadata { rows: meta.rows + 1, update_bytes: meta.update_bytes + size, ..meta }
            }
        };
        if let Some((theme, _)) = &job.theme {
            tx.prepare_cached("UPDATE document SET theme=? WHERE id=1")
                .and_then(|mut s| s.execute([theme]))
                .map_err(sqlite("save theme"))?;
        }
        tx.commit().map_err(sqlite("commit"))?;
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
        Ok((slot.theme.state(), changed))
    }
    /// The palette as a theme file for this document's template (see `theme::Theme::export`).
    pub fn export_theme(&self) -> Result<String> {
        Ok(lock(&self.theme).as_ref().ok_or(Error::Closed)?.theme.export())
    }

    /// Releases the database, withdraws discovery, then releases the writer lock. A failed
    /// close keeps ownership.
    pub fn close(&self) -> Result<()> {
        let mut backing = lock(&self.backing);
        if let Some(open) = backing.conn.take() {
            if let Err((open, e)) = open.close() {
                backing.conn = Some(open);
                return Err(failed(format!("close; retaining document ownership: {e}")));
            }
        }
        self.owned.store(false, Ordering::Release);
        if let Some(lease) = backing.lease.take() {
            lease.withdraw();
        }
        Ok(())
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
        self.connected(&mut lock(&self.backing).conn, |conn| self.store_attachment(conn, &id, bytes))?;
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
            if !file::attachments_fit(count + 1, bytes.len() as i64, total + bytes.len() as i64) {
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
        if !crate::wire::valid_attachment_id(id) {
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
    /// Writes the document's artwork (`preview`, `icon`) through the writer's connection: a
    /// window renders it from the open document as it closes. Checked as `pack` checks it.
    pub fn set_artwork(&self, artwork: &[(&str, &[u8])]) -> Result<()> {
        self.check(true)?;
        for (name, png) in artwork {
            if !matches!(*name, "preview" | "icon") {
                return Err(invalid(format!("Unknown artwork {name}")));
            }
            file::check_artwork(&format!("The {name} artwork"), png)?;
        }
        self.connected(&mut lock(&self.backing).conn, |conn| {
            let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("write artwork"))?;
            for (name, png) in artwork {
                tx.execute("INSERT INTO artwork VALUES(?,?) ON CONFLICT(name) DO UPDATE SET png=excluded.png", params![name, png])
                    .map_err(sqlite("write artwork"))?;
            }
            tx.commit().map_err(sqlite("write artwork"))
        })
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
    /// Runs a read on the store's connection: the writer's, or a snapshot's reader.
    fn read<T>(&self, f: impl Fn(&Connection) -> Result<T>) -> Result<T> {
        self.connected(&mut lock(&self.backing).conn, f)
    }

    /// Names the live owner for clients: a writer publishes its socket in the registry.
    pub fn publish_discovery(&self, json: &str) -> Result<()> {
        lock(&self.backing).lease.as_ref().ok_or(Error::Closed)?.publish(json)
    }
    pub fn withdraw_discovery(&self) {
        if let Some(lease) = &lock(&self.backing).lease {
            lease.withdraw();
        }
    }

    /// Copies the open document to `dest` from the writer's own connection, so saves queue
    /// behind the copy instead of timing out. Duplicate and Share a Copy use it after
    /// flushing.
    pub fn copy_to(&self, dest: &Path) -> Result<()> {
        self.check(true)?;
        self.read(|conn| file::copy(conn, dest, false))
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
