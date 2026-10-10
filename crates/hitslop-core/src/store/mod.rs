//! Durable storage for native and browser hosts. A document is one SQLite file (`file` owns its
//! format); this module saves its state: a history (a Loro snapshot, then the updates saved
//! after it), its attachments and its artwork. The writer lock lives in the registry, outside the file. SQLite never
//! sees anything but opaque Loro bytes.
//!
//! History is trimmed when nothing is editing: a session that edited a document larger
//! than `TRIM_BYTES` closes with no history. While open, a checkpoint keeps the
//! whole history when it fits the storage limits, else only the current state; the live
//! document keeps its own history either way. Compaction keeps no history. A shared
//! document never trims and never reclaims an attachment: another replica's edits can depend
//! on any of its history and still reference any blob.
//!
//! A host keeps two serial queues: edits and `Store::job` on one, every other
//! `Store` call on the other, so a slow write never blocks edits.

mod attachments;
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod copy;
mod save;
pub use crate::error::{Error, Result};
use crate::error::{failed, rejected, sqlite};
use crate::file::{self, Artwork, Kind, OpenedApp, rows};
#[cfg(not(target_arch = "wasm32"))]
use crate::registry::Lease;
use crate::{Document, lock};
pub use attachments::Attachment;
#[cfg(not(target_arch = "wasm32"))]
use attachments::delete_unreferenced;
use loro::{ExportMode, Frontiers, VersionVector};
use rusqlite::{Connection, Transaction, TransactionBehavior};
pub use save::SaveJob;
use save::{Metadata, Rows, bounds, checked_bounds, checkpoint_row, within};
use sha2::{Digest, Sha256};
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// A core failure while loading: a document this build is too old for is a refusal the
/// host names; anything else is a storage failure.
fn load_failure(e: crate::Error) -> Error {
    if e.code == crate::Code::RequiresUpdate { Error::Rejected(e) } else { failed(e) }
}

/// The store's connection and, for the writer, its lock. Field order matters: the
/// connection closes before the lock is released.
struct Backing {
    conn: Option<Connection>,
    #[cfg(not(target_arch = "wasm32"))]
    lease: Option<Lease>,
}
struct Account {
    meta: Metadata,
    /// The version the durable state covers.
    saved: VersionVector,
    /// Whether this session saved an edit or stored an attachment: only then can a blob
    /// have lost its last reference, and only then does close trim history.
    changed: bool,
    /// Whether the document syncs through a room. A shared document keeps its whole
    /// history (another replica's edits can depend on any of it) and every attachment
    /// (another replica can still reference it).
    shared: bool,
}
impl Account {
    fn new() -> Self {
        Self { meta: Metadata::default(), saved: VersionVector::default(), changed: false, shared: false }
    }
}
/// The room a shared document syncs through: its ID and the relay's endpoint. The room's
/// key is a credential the host keeps outside the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Share {
    pub room: String,
    pub endpoint: String,
}

/// One document's storage. `Document` mode owns the file: it holds the writer lock and
/// persists writes. `Snapshot` mode reads the saved state without the lock and writes
/// nothing, so a render never locks the file or changes what it holds (it may finish
/// rolling back a crashed write, as any reader does).
pub struct Store {
    #[cfg(target_arch = "wasm32")]
    browser_file: (String, String),
    #[cfg(target_arch = "wasm32")]
    resource_cache: std::sync::Arc<Mutex<file::ResourceCache>>,
    #[cfg(not(target_arch = "wasm32"))]
    path: PathBuf,
    /// The file's device and inode when opened; a rename or replacement is `Moved`.
    #[cfg(not(target_arch = "wasm32"))]
    inode: (u64, u64),
    /// The app the document was built with, checked once when it opened.
    app: OpenedApp,
    backing: Mutex<Backing>,
    /// Whether this store holds the writer lock. Read without `backing`, which a save holds
    /// for its whole transaction, so checking ownership never waits for a save.
    owned: AtomicBool,
    account: Mutex<Account>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Document,
    Snapshot,
}

/// Opens the saved document under the descriptor of the app it is stored with. The
/// history's snapshot and every update after it are imported straight from SQLite's
/// buffers, without copying them.
fn load(conn: &Connection, app: &crate::AppSpec) -> Result<(Document, Metadata)> {
    let meta = checked_bounds(conn)?;
    let mut saved = conn.prepare_cached(rows::SNAPSHOT).map_err(sqlite("read"))?;
    let mut saved = saved.query([]).map_err(sqlite("read"))?;
    // Every open checked the file holds a history (`file::check`).
    let row = saved
        .next()
        .map_err(sqlite("read"))?
        .ok_or_else(|| failed("The file has no saved state; keep it for recovery"))?;
    let checkpoint =
        row.get_ref(0).ok().and_then(|bytes| bytes.as_blob().ok()).ok_or_else(|| failed("Invalid checkpoint bytes"))?;
    let mut updates = conn.prepare_cached(rows::UPDATES).map_err(sqlite("read updates"))?;
    let mut updates = updates.query([]).map_err(sqlite("read updates"))?;
    let doc = Document::open_with(app, checkpoint, load_failure, |import| {
        loop {
            match updates.next().map_err(sqlite("read updates"))? {
                None => return Ok(()),
                Some(row) => import(
                    row.get_ref(0).ok().and_then(|v| v.as_blob().ok()).ok_or_else(|| failed("Invalid update bytes"))?,
                )?,
            }
        }
    })?;
    Ok((doc, meta))
}

/// A new file may be published only after its saved state has passed the same acceptance
/// as opening it. The caller keeps the source read transaction through its copy.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn validate_saved(conn: &Connection, app: &crate::AppSpec) -> Result<()> {
    load(conn, app).map(|_| ())
}

impl Store {
    /// A digest of the app this file holds: its whole `app` row and assets, typed and
    /// length-delimited. Two files hold the same app exactly when their digests match;
    /// document state and artwork are not part of it. A shared document's relay admits only
    /// replicas whose digest matches its room's.
    pub fn app_digest(&self) -> Result<[u8; 32]> {
        self.check(false)?;
        self.read(rows::app_digest)
    }

    /// Opens the document file at `path`. `Document` mode takes the writer lock and checks
    /// the file, including SQLite's quick check; a template is never opened as a document
    /// (`file::create_document` makes one from it). `Snapshot` mode reads a document's
    /// saved state, or a template's initial values, once.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path, mode: Mode) -> Result<Self> {
        let (lease, conn, app) = match mode {
            Mode::Document => {
                file::document_location(path)?;
                let lease = Lease::acquire(path)?;
                let conn = file::writer(path, false)?;
                // Checked before anything is configured: a file this build refuses is never
                // written, not even its journal mode.
                let app = file::opened(&conn, path, true)?;
                if app.kind != Kind::Document {
                    return Err(rejected(crate::Code::IsTemplate, "A template opens by creating a document from it"));
                }
                file::configure_writer(&conn)?;
                (Some(lease), conn, app)
            }
            Mode::Snapshot => {
                let conn = file::reader(path)?;
                let app = file::opened(&conn, path, false)?;
                (None, conn, app)
            }
        };
        let inode = match &lease {
            Some(lease) => lease.file(),
            None => crate::registry::identity(path)
                .map(|(dev, ino, _)| (dev, ino))
                .map_err(|_| failed("Cannot find the document"))?,
        };
        Ok(Self {
            path: path.to_owned(),
            inode,
            app,
            owned: AtomicBool::new(lease.is_some()),
            backing: Mutex::new(Backing { conn: Some(conn), lease }),
            account: Mutex::new(Account::new()),
        })
    }

    /// The app this store's file holds, as its open checked it.
    pub fn app(&self) -> &OpenedApp {
        &self.app
    }
    /// A reader of the app's assets on its own connection, which pages read from while this
    /// store saves. The file is the one this store's open checked, or the reader fails with
    /// `Moved`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn resource_reader(&self) -> Result<file::ResourceReader> {
        let conn = file::reader(&self.path)?;
        self.check(false)?;
        Ok(file::ResourceReader::new(conn))
    }
    /// Fails once the file was moved or replaced (or, for the writer, gained a hard link),
    /// or, with `writable`, once the store no longer owns it.
    pub fn check(&self, writable: bool) -> Result<()> {
        // A stat, never the backing mutex a save holds: a theme change never waits for one.
        let owned = self.owned.load(Ordering::Acquire);
        #[cfg(not(target_arch = "wasm32"))]
        match crate::registry::identity(&self.path) {
            Ok((dev, ino, links)) if (dev, ino) == self.inode && (links == 1 || !owned) => {}
            _ => return Err(Error::Moved),
        }
        if writable && !owned {
            return Err(Error::Closed);
        }
        Ok(())
    }

    /// The saved data and theme, imported from one read transaction: a template's initial
    /// state, or a document's checkpoint and updates. Reloading after discard drops unsaved
    /// data and theme together.
    pub fn document(&self) -> Result<Document> {
        self.check(false)?;
        let (doc, meta, shared) = self.connected(&mut lock(&self.backing).conn, |conn| {
            let read = Transaction::new_unchecked(conn, TransactionBehavior::Deferred).map_err(sqlite("read"))?;
            let (doc, meta) = load(&read, self.app.app.spec())?;
            Ok((doc, meta, rows::read_share(&read)?.is_some()))
        })?;
        let mut account = lock(&self.account);
        *account = Account { meta, saved: doc.doc.oplog_vv(), changed: account.changed, shared };
        Ok(doc)
    }

    /// The room this document syncs through, when it is shared.
    pub fn share(&self) -> Result<Option<Share>> {
        self.check(false)?;
        Ok(self.read(rows::read_share)?.map(|(room, endpoint)| Share { room, endpoint }))
    }
    /// Shares the document through `share`'s room: from now on it keeps its whole history
    /// and every attachment.
    pub fn set_share(&self, share: &Share) -> Result<()> {
        self.check(true)?;
        if !(1..=128).contains(&share.room.len()) || !(1..=2048).contains(&share.endpoint.len()) {
            return Err(rejected(crate::Code::InvalidRequest, "Invalid room or endpoint"));
        }
        self.connected(&mut lock(&self.backing).conn, |conn| {
            let tx = file::begin_write(conn, "share")?;
            rows::put_share(&tx, &share.room, &share.endpoint)?;
            tx.commit()
        })?;
        lock(&self.account).shared = true;
        Ok(())
    }
    pub(crate) fn is_shared(&self) -> bool {
        lock(&self.account).shared
    }

    /// Writes a job in one transaction. An error may follow the commit, so the durable
    /// version advances only on success; sizes are re-read either way.
    pub fn write(&self, job: &SaveJob) -> Result<()> {
        self.check(true)?;
        let mut backing = lock(&self.backing);
        let meta = match self.connected(&mut backing.conn, |conn| self.transaction(conn, job)) {
            Ok(meta) => meta,
            Err(e) => {
                if let Some(Ok(meta)) = backing.conn.as_ref().map(bounds) {
                    lock(&self.account).meta = meta;
                }
                return Err(e);
            }
        };
        let mut account = lock(&self.account);
        account.meta = meta;
        account.saved = job.version.clone();
        account.changed = true;
        Ok(())
    }
    /// Runs `work` on the store's connection. Apple's SQLite stops a connection for good
    /// once its file is renamed, or gains and loses a hard link, even after the file is back
    /// (`Moved`). When the path again names this store's file, the store reconnects and the
    /// work runs once more; a writer's lease never left the file.
    fn connected<T>(&self, conn: &mut Option<Connection>, work: impl Fn(&Connection) -> Result<T>) -> Result<T> {
        #[cfg(target_arch = "wasm32")]
        if conn.is_none() {
            *lock(&self.resource_cache) = Default::default();
            self.check(false)?;
            let (name, vfs) = &self.browser_file;
            let (fresh, app) = browser::open_browser(name, vfs, "reopen")?;
            load(&fresh, app.app.spec())?;
            file::configure_writer(&fresh)?;
            *conn = Some(fresh);
        }
        match work(conn.as_ref().ok_or(Error::Closed)?) {
            #[cfg(not(target_arch = "wasm32"))]
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
            // An OPFS I/O failure can poison SQLite's pager even after space is
            // available again. Keep the Web Lock and unsaved owner state, but close
            // this connection. The next explicit attempt reopens through the same
            // VFS and recovers its journal before retrying the save.
            #[cfg(target_arch = "wasm32")]
            Err(error @ Error::Failed(_)) => {
                drop(conn.take());
                Err(error)
            }
            other => other,
        }
    }
    /// Dropping the guard rolls back: on any error, after a failed COMMIT, or in a panic.
    fn transaction(&self, conn: &Connection, job: &SaveJob) -> Result<Metadata> {
        let tx = file::begin_write(conn, "begin")?;
        let meta = match &job.rows {
            Rows::Checkpoint(bytes) => {
                if !within(0, checkpoint_row(bytes.len())) {
                    return Err(Error::Full);
                }
                rows::replace_history(&tx, bytes)?;
                Metadata { rows: 0, update_bytes: 0, checkpoint_bytes: bytes.len() as i64 }
            }
            Rows::Append(bytes) => {
                // A write that would cross the limits leaves saved state intact.
                let meta = checked_bounds(&tx)?;
                let size = bytes.len() as i64;
                if !within(meta.rows + 1, meta.stored() + size) {
                    return Err(Error::Full);
                }
                rows::append_history(&tx, bytes)?;
                Metadata { rows: meta.rows + 1, update_bytes: meta.update_bytes + size, ..meta }
            }
        };
        tx.commit()?;
        Ok(meta)
    }

    /// Releases the database, withdraws discovery, then releases the writer lock. A failed
    /// close keeps ownership.
    pub fn close(&self) -> Result<()> {
        let mut backing = lock(&self.backing);
        if let Some(open) = backing.conn.take()
            && let Err((open, e)) = open.close()
        {
            backing.conn = Some(open);
            return Err(failed(format!("close; retaining document ownership: {e}")));
        }
        self.owned.store(false, Ordering::Release);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(lease) = backing.lease.take() {
            lease.withdraw();
        }
        Ok(())
    }

    /// One artwork image, through this store's own connection.
    pub fn artwork(&self, name: Artwork) -> Result<Option<Vec<u8>>> {
        self.check(false)?;
        self.read(|conn| rows::read_artwork(conn, name))
    }
    /// Runs a read on the store's connection: the writer's, or a snapshot's reader.
    fn read<T>(&self, f: impl Fn(&Connection) -> Result<T>) -> Result<T> {
        self.connected(&mut lock(&self.backing).conn, f)
    }

    /// Names the live owner for clients: a writer publishes its socket in the registry.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn publish_discovery(&self, json: &str) -> Result<()> {
        lock(&self.backing).lease.as_ref().ok_or(Error::Closed)?.publish(json)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn withdraw_discovery(&self) {
        if let Some(lease) = &lock(&self.backing).lease {
            lease.withdraw();
        }
    }
}
