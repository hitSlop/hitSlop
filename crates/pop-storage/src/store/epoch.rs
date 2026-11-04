//! Staged authority snapshots preserve document identity. Canonical replacement writes
//! through the existing connection and lease; it never replaces a live owner's file.
use super::*;

/// A staged SQLite document already owned by the native lease or browser Web Lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EpochLocation {
    #[cfg(not(target_arch = "wasm32"))]
    Path(std::path::PathBuf),
    #[cfg(target_arch = "wasm32")]
    Vfs { name: String, vfs: String },
}
#[cfg(not(target_arch = "wasm32"))]
impl From<std::path::PathBuf> for EpochLocation {
    fn from(path: std::path::PathBuf) -> Self {
        Self::Path(path)
    }
}
impl EpochLocation {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native_path(&self) -> &Path {
        let Self::Path(path) = self;
        path
    }
    fn inspection_path(&self) -> &Path {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Path(path) => path,
            #[cfg(target_arch = "wasm32")]
            Self::Vfs { name, .. } => Path::new(name),
        }
    }
    fn reader(&self) -> Result<Connection> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Path(path) => file::reader(path),
            #[cfg(target_arch = "wasm32")]
            Self::Vfs { name, vfs } => {
                let connection = Connection::open_with_flags_and_vfs(
                    name,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    vfs.as_str(),
                )
                .map_err(sqlite("open epoch VFS"))?;
                file::configure_connection(&connection)?;
                Ok(connection)
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn destination(&self) -> Result<Connection> {
        let Self::Vfs { name, vfs } = self;
        let connection = Connection::open_with_flags_and_vfs(
            name,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            vfs.as_str(),
        )
        .map_err(sqlite("open epoch destination"))?;
        file::configure_connection(&connection)?;
        Ok(connection)
    }
    fn exists(&self) -> Result<bool> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Path(path) => Ok(std::fs::symlink_metadata(path).is_ok()),
            #[cfg(target_arch = "wasm32")]
            Self::Vfs { .. } => {
                let connection = self.destination()?;
                connection
                    .query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))
                    .map(|pages| pages != 0)
                    .map_err(sqlite("inspect epoch VFS"))
            }
        }
    }
    fn copy_from(&self, source: &Connection, sanitize: &dyn Fn(&Connection) -> Result<()>) -> Result<()> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Path(path) => {
                file::document_destination(path)?;
                file::copy(source, path, false, true, Some(sanitize))
            }
            #[cfg(target_arch = "wasm32")]
            Self::Vfs { .. } => {
                // The driver installed a fresh private destination VFS and holds its
                // Web Lock. Never overwrite a previously populated candidate/recovery.
                let mut output = self.destination()?;
                let pages: i64 = output
                    .query_row("PRAGMA page_count", [], |row| row.get(0))
                    .map_err(sqlite("inspect epoch destination"))?;
                if pages != 0 {
                    return Err(rejected(crate::Code::Exists, "The epoch destination already contains a document"));
                }
                let Self::Vfs { vfs, .. } = self;
                let scratch_location = Self::Vfs { name: "epoch-staging.pop".into(), vfs: vfs.clone() };
                let mut scratch = scratch_location.destination()?;
                backup_into(source, &mut scratch)?;
                file::configure_writer(&scratch)?;
                sanitize(&scratch)?;
                file::check(&scratch, true)?;
                // Backup commits the already sanitized database atomically. A failed
                // backup rolls the fresh destination back; the driver removes scratch
                // under the same Web Lock before retrying this reserved location.
                backup_into(&scratch, &mut output)?;
                scratch.close().map_err(|(_, error)| sqlite("close epoch scratch")(error))?;
                output.close().map_err(|(_, error)| sqlite("close epoch destination")(error))
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn backup_into(source: &Connection, target: &mut Connection) -> Result<()> {
    let backup = rusqlite::backup::Backup::new(source, target).map_err(sqlite("copy epoch"))?;
    match backup.step(-1).map_err(sqlite("copy epoch"))? {
        rusqlite::backup::StepResult::Done => Ok(()),
        rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => Err(Error::Busy),
        _ => Err(failed("The epoch copy did not complete")),
    }
}

impl Store {
    /// The public app resource inventory from the same accepted SQLite snapshot.
    /// Both native and browser sharing hash decoded bytes through the same bounded reader.
    pub fn sharing_manifest(&self) -> Result<pop_sharing_protocol::resources::Manifest> {
        use pop_sharing_protocol::resources::{self, Manifest, Resource};
        self.check(false)?;
        self.read(|connection| {
            let read = connection.unchecked_transaction().map_err(sqlite("read sharing resources"))?;
            let connection: &Connection = &read;
            let reader = file::ResourceReader::with_cache(connection, Default::default());
            let mut inventory = Vec::new();
            for key in rows::asset_keys(&read)? {
                if !resources::asset_key(&key) {
                    continue;
                }
                let info = reader
                    .info(file::ResourceRoute::App, &key)?
                    .ok_or_else(|| failed("Missing public app resource"))?;
                let mut hash = Sha256::new();
                let mut offset = 0;
                while offset < info.size {
                    let length = (info.size - offset).min(1 << 20);
                    let bytes = reader
                        .read_range(file::ResourceRoute::App, &key, offset, length)?
                        .ok_or_else(|| failed("Missing app resource bytes"))?;
                    if bytes.len() as u64 != length {
                        return Err(failed("Incomplete app resource bytes"));
                    }
                    hash.update(&bytes);
                    offset += length;
                }
                inventory.push(Resource {
                    key,
                    hash: crate::hex(&hash.finalize()),
                    size: info.size,
                    media_type: info.media_type,
                });
            }
            resources::validate(&inventory, false).map_err(failed)?;
            Ok(Manifest { app: crate::hex(&rows::app_digest(&read)?), resources: inventory })
        })
    }

    /// Stages an accepted clean epoch under the original document UUID, keeping only
    /// referenced blobs and clearing potentially stale artwork. The live file is untouched.
    pub fn stage_epoch(&self, destination: &EpochLocation, checkpoint: &[u8]) -> Result<String> {
        self.check(true)?;
        let document = Document::open(self.app.app.spec(), checkpoint, &[]).map_err(load_failure)?;
        self.copy_epoch(destination, checkpoint, &document, false)?;
        Ok(document.version())
    }

    fn copy_epoch(
        &self,
        destination: &EpochLocation,
        checkpoint: &[u8],
        document: &Document,
        independent: bool,
    ) -> Result<()> {
        if !within(0, checkpoint_row(checkpoint.len())) {
            return Err(Error::Full);
        }
        self.read(|source| {
            // Copying a candidate must preserve the source's persisted layout.
            // Core performs the same exact check used by committed adoption.
            if !independent {
                let mut current = load(source, self.app.app.spec())?.0;
                current.adopt_checkpoint(checkpoint).map_err(load_failure)?;
            }
            destination.copy_from(source, &|staged| {
                let tx = file::begin_write(staged, "stage document epoch")?;
                if independent {
                    rows::renew_document(&tx)?;
                }
                rows::replace_history(&tx, checkpoint)?;
                rows::clear_artwork(&tx)?;
                attachments::delete_unreferenced(&tx, Some(document), self.app.app.spec())?;
                for (id, _, _) in rows::attachment_list(&tx)? {
                    attachments::stored_attachment(&tx, &id)?.ok_or_else(|| failed("Staged attachment disappeared"))?;
                }
                tx.commit()?;
                // Compact the private copy after logical deletion, including sources
                // whose accepted SQLite settings do not reclaim free pages themselves.
                staged.execute_batch("VACUUM").map_err(sqlite("compact staged epoch"))
            })
        })
    }

    /// Reuses only an exact, independent recovery copy after a lost acknowledgment.
    /// An unrelated destination is never overwritten or treated as a successful recovery.
    pub fn ensure_recovery(&self, destination: &EpochLocation, checkpoint: &[u8]) -> Result<()> {
        self.check(true)?;
        if !destination.exists()? {
            let expected = Document::open(self.app.app.spec(), checkpoint, &[]).map_err(load_failure)?;
            let clean = expected.clean_checkpoint().map_err(load_failure)?;
            return self.copy_epoch(destination, &clean, &expected, true);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let lease = {
            file::document_location(destination.native_path())?;
            Lease::acquire(destination.native_path())?
        };
        let recovery = destination.reader()?;
        let expected = Document::open(self.app.app.spec(), checkpoint, &[]).map_err(load_failure)?;
        self.read(|source| {
            let source_read = source.unchecked_transaction().map_err(sqlite("read recovery source"))?;
            {
                let copy_read = recovery.unchecked_transaction().map_err(sqlite("read recovery copy"))?;
                let accepted = file::opened(&copy_read, destination.inspection_path(), true)?;
                let actual = load(&copy_read, accepted.app.spec())?.0;
                let same = accepted.kind == Kind::Document
                    && accepted.document_uuid.is_some()
                    && accepted.document_uuid != self.app.document_uuid
                    && rows::app_digest(&copy_read)? == rows::app_digest(&source_read)?
                    && rows::read_share(&copy_read)?.is_none()
                    && actual.reading().map_err(load_failure)?.value == expected.reading().map_err(load_failure)?.value
                    && actual.theme_state().map_err(load_failure)?.overrides
                        == expected.theme_state().map_err(load_failure)?.overrides;
                if !same {
                    return Err(rejected(
                        crate::Code::Exists,
                        "The recovery destination contains a different document",
                    ));
                }
                let stored: Vec<_> = rows::attachment_list(&source_read)?.into_iter().map(|(id, _, _)| id).collect();
                let referenced = expected.attachment_references(&stored);
                let actual_ids: std::collections::HashSet<_> =
                    rows::attachment_list(&copy_read)?.into_iter().map(|(id, _, _)| id).collect();
                if referenced != actual_ids {
                    return Err(rejected(crate::Code::Exists, "The recovery destination has different attachments"));
                }
                for id in referenced {
                    let expected = attachments::stored_attachment(&source_read, &id)?
                        .ok_or_else(|| failed("Recovery source attachment disappeared"))?;
                    let actual = attachments::stored_attachment(&copy_read, &id)?
                        .ok_or_else(|| failed("Recovery attachment disappeared"))?;
                    if expected != actual {
                        return Err(rejected(
                            crate::Code::Exists,
                            "The recovery destination has different attachment bytes",
                        ));
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                lease.check()?;
                Ok(())
            }
        })
    }

    /// Installs a validated canonical epoch only over the exact flushed source version.
    /// Old blobs remain until the owner acknowledges its new in-memory state, so an
    /// uncertain transaction never removes data required by the old session's undo.
    pub fn adopt_epoch(&self, source: &EpochLocation, expected_version: &str) -> Result<Document> {
        self.check(true)?;
        let incoming = source.reader()?;
        // Keep app identity, accepted history and every copied blob in the same source
        // transaction. A display reader never upgrades or repairs that source file.
        let result = (|| {
            let source_read = incoming.unchecked_transaction().map_err(sqlite("read epoch source"))?;
            let accepted = file::opened(&source_read, source.inspection_path(), true)?;
            let document = load(&source_read, accepted.app.spec())?.0;
            if accepted.document_uuid != self.app.document_uuid {
                return Err(rejected(crate::Code::InvalidRequest, "Epoch belongs to a different document"));
            }
            let checkpoint = document.checkpoint().map_err(load_failure)?;
            if !within(0, checkpoint_row(checkpoint.len())) {
                return Err(Error::Full);
            }
            // `read`/`connected` may retry a moved Apple connection: each invocation
            // obtains its own target guard instead of capturing a mutable outer borrow.
            let mut backing = lock(&self.backing);
            let (meta, shared) = self.connected(&mut backing.conn, |target| {
                let tx = file::begin_write(target, "install document epoch")?;
                // Authority adoption installs only history and referenced blobs;
                // each replica retains its embedded UI bundle. Require the same
                // accepted state contract, as ordinary document synchronization does.
                if !accepted.app.replica_compatible(&self.app.app) {
                    return Err(rejected(crate::Code::InvalidRequest, "Epoch belongs to a different state contract"));
                }
                let mut current = load(&tx, self.app.app.spec())?.0;
                if current.version() != expected_version {
                    return Err(rejected(
                        crate::Code::StaleBase,
                        "The saved document changed before epoch installation",
                    ));
                }
                // Core validates exact persisted-layout compatibility before any
                // target row/blob changes, including identity-preserving layout 1.
                current.adopt_checkpoint(&checkpoint).map_err(load_failure)?;
                for (id, _, _) in rows::attachment_list(&source_read)? {
                    let bytes = attachments::stored_attachment(&source_read, &id)?
                        .ok_or_else(|| failed("Incoming attachment disappeared"))?;
                    if attachments::stored_attachment(&tx, &id)?.is_none() {
                        let (count, _, total) = rows::attachment_sizes(&tx)?;
                        if !file::attachments_fit(count + 1, bytes.len() as i64, total + bytes.len() as i64) {
                            return Err(Error::Full);
                        }
                        rows::put_attachment(&tx, &id, &bytes)?;
                    }
                }
                rows::replace_history(&tx, &checkpoint)?;
                rows::clear_artwork(&tx)?;
                let shared = rows::read_share(&tx)?.is_some();
                tx.commit()?;
                Ok((bounds(target)?, shared))
            })?;
            Ok((document, meta, shared))
        })();
        match result {
            Ok((document, meta, shared)) => {
                let mut account = lock(&self.account);
                account.meta = meta;
                account.saved = document.history_version();
                account.changed = true;
                account.shared = shared;
                account.checkpoint_required = false;
                Ok(document)
            }
            Err(error) => {
                // Gather connection metadata before taking account: all storage paths
                // use backing-before-account lock order. An unknown commit fences append.
                let meta = {
                    let backing = lock(&self.backing);
                    backing.conn.as_ref().and_then(|connection| bounds(connection).ok())
                };
                let mut account = lock(&self.account);
                account.checkpoint_required |= matches!(error, Error::Failed(_) | Error::Full | Error::Moved);
                if let Some(meta) = meta {
                    account.meta = meta;
                }
                Err(error)
            }
        }
    }

    /// Best-effort cleanup after the owner has installed and published this epoch.
    /// A queued later edit or replacement makes this a no-op rather than deleting blobs.
    pub fn reclaim_epoch(&self, expected_version: &str) -> Result<()> {
        self.check(true)?;
        self.read(|connection| {
            let tx = file::begin_write(connection, "reclaim adopted epoch")?;
            let document = load(&tx, self.app.app.spec())?.0;
            if document.version() != expected_version {
                return Ok(());
            }
            attachments::delete_unreferenced(&tx, Some(&document), self.app.app.spec())?;
            tx.commit()
        })
    }
}
