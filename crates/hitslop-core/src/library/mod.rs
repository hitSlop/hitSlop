//! Account-scoped library metadata and whole-file replication through the existing owner.
pub mod snapshot;
use crate::{
    Code, file,
    owner::{Failure, FailureKind, Owner, Request},
    store::{Mode, Share, Store},
};
use rusqlite::{Connection, params};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    Busy,
    RequiresUpdate,
    AppMismatch,
    TooLarge,
    AccountChanged,
    Failed,
}
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub issue: Issue,
    pub message: String,
}
pub type Result<T> = std::result::Result<T, Error>;
fn failed(e: impl std::fmt::Display) -> Error {
    Error { issue: Issue::Failed, message: e.to_string() }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        failed(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        failed(e)
    }
}
impl From<Failure> for Error {
    fn from(e: Failure) -> Self {
        let issue = if e.reason == Some(Code::RequiresUpdate) {
            Issue::RequiresUpdate
        } else if matches!(e.kind, FailureKind::Busy | FailureKind::Locked | FailureKind::Closing) {
            Issue::Busy
        } else {
            Issue::Failed
        };
        Self { issue, message: e.message }
    }
}
impl From<crate::store::Error> for Error {
    fn from(e: crate::store::Error) -> Self {
        Failure::from(e).into()
    }
}
impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Failure::from(e).into()
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub template: String,
    pub path: String,
    pub version: String,
    pub asset_revision: String,
    pub artwork_version: String,
    pub dirty: bool,
    pub deleted: bool,
    pub local_only: bool,
    pub problem: Option<String>,
}
pub struct Upload {
    pub entry: Entry,
    pub asset_path: Option<String>,
    pub fields: Option<Vec<u8>>,
}
struct Inner {
    db: Connection,
    live: HashMap<String, Weak<Owner>>,
    // Failed temporary owners keep their writer lease and unsaved state for retry.
    retained: HashMap<String, Arc<Owner>>,
    artwork_cache: RefCell<HashMap<String, (u64, std::time::SystemTime, String, String)>>,
    relay_checkpoint: HashMap<String, std::time::Instant>,
}
pub struct Library {
    root: PathBuf,
    inner: Mutex<Inner>,
}
impl Library {
    pub fn open(root: &Path, account: &str) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root.join("documents"))?;
        std::fs::create_dir_all(root.join("uploads"))?;
        let root = std::fs::canonicalize(root)?;
        let db = Connection::open(root.join("library.sqlite"))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value BLOB NOT NULL);
          CREATE TABLE IF NOT EXISTS documents(id TEXT PRIMARY KEY, title TEXT NOT NULL, template TEXT NOT NULL,
            confirmed TEXT NOT NULL DEFAULT '', fields BLOB, deleted INTEGER NOT NULL DEFAULT 0,
            confirmed_deleted INTEGER NOT NULL DEFAULT 0, remote_delete INTEGER NOT NULL DEFAULT 0,
            local_only INTEGER NOT NULL DEFAULT 0, problem TEXT);",
        )?;
        let columns = db
            .prepare("PRAGMA table_info(documents)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for name in ["confirmed_asset", "artwork_version"] {
            if !columns.iter().any(|column| column == name) {
                db.execute_batch(&format!("ALTER TABLE documents ADD COLUMN {name} TEXT NOT NULL DEFAULT '';"))?;
            }
        }
        let saved: Option<String> =
            db.query_row("SELECT CAST(value AS TEXT) FROM meta WHERE key='account'", [], |r| r.get(0)).ok();
        if saved.is_some_and(|saved| saved != account) {
            return Err(Error {
                issue: Issue::AccountChanged,
                message: "Not syncing: this library belongs to another iCloud account".into(),
            });
        }
        db.execute("INSERT OR IGNORE INTO meta VALUES('account', ?1)", [account])?;
        for entry in std::fs::read_dir(root.join("uploads"))? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(Arc::new(Self {
            root,
            inner: Mutex::new(Inner {
                db,
                live: HashMap::new(),
                retained: HashMap::new(),
                artwork_cache: RefCell::new(HashMap::new()),
                relay_checkpoint: HashMap::new(),
            }),
        }))
    }
    fn valid_id(id: &str) -> Result<()> {
        if id.len() != 36 || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            return Err(failed("Invalid library document ID"));
        }
        Ok(())
    }
    fn path(&self, id: &str) -> PathBuf {
        self.root.join("documents").join(format!("{id}.slop"))
    }
    pub fn register(&self, owner: &Arc<Owner>) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(id) = owner.app().document_uuid.as_ref()
            && owner.path() == self.path(id)
        {
            inner.live.insert(id.clone(), Arc::downgrade(owner));
        }
        Ok(())
    }
    fn entry(&self, inner: &Inner, id: &str) -> Result<Entry> {
        let (title, template, confirmed, deleted, confirmed_deleted, local_only): (
            String,
            String,
            String,
            bool,
            bool,
            bool,
        ) = inner.db.query_row(
            "SELECT title,template,confirmed,deleted,confirmed_deleted,local_only FROM documents WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )?;
        let problem: Option<String> =
            inner.db.query_row("SELECT problem FROM documents WHERE id=?1", [id], |r| r.get(0))?;
        let (confirmed_asset, source_version): (String, String) =
            inner.db.query_row("SELECT confirmed_asset,artwork_version FROM documents WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        let (version, asset_revision) = if deleted || problem.is_some() {
            (String::new(), String::new())
        } else {
            let path = self.path(id);
            let stat = std::fs::metadata(&path)?;
            let stamp = (stat.len(), stat.modified()?);
            let mut cache = inner.artwork_cache.borrow_mut();
            let (version, digest) = match cache.get(id) {
                Some((size, time, version, digest)) if (*size, *time) == stamp => (version.clone(), digest.clone()),
                _ => {
                    let store = Store::open(&path, Mode::Snapshot)?;
                    let version = store.document()?.reading()?.version;
                    let digest = snapshot::artwork_digest(&store).map_err(failed)?;
                    cache.insert(id.into(), (stamp.0, stamp.1, version.clone(), digest.clone()));
                    (version, digest)
                }
            };
            let revision = snapshot::asset_revision(&version, &source_version, &digest);
            (version, revision)
        };
        Ok(Entry {
            id: id.into(),
            title,
            template,
            path: self.path(id).to_string_lossy().into_owned(),
            dirty: problem.is_none()
                && !local_only
                && if deleted {
                    !confirmed_deleted
                } else if confirmed_asset.is_empty() {
                    version != confirmed || source_version == version
                } else {
                    asset_revision != confirmed_asset
                },
            artwork_version: if source_version == version { source_version } else { String::new() },
            version,
            asset_revision,
            deleted,
            local_only,
            problem,
        })
    }
    pub fn entries(&self) -> Result<Vec<Entry>> {
        let inner = self.inner.lock().unwrap();
        let ids = inner
            .db
            .prepare("SELECT id FROM documents ORDER BY rowid DESC")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter().map(|id| self.entry(&inner, id)).collect()
    }
    fn with_owner<T>(&self, inner: &mut Inner, id: &str, work: impl FnOnce(&Owner) -> Result<T>) -> Result<T> {
        let existing = inner.retained.get(id).cloned().or_else(|| inner.live.get(id).and_then(Weak::upgrade));
        if let Some(owner) = existing
            && snapshot::request(&owner, Request::State).is_ok()
        {
            return work(&owner);
        }
        let owner = Arc::new(Owner::open(&self.path(id), Mode::Document, Arc::new(|_| {}))?);
        let result = work(&owner);
        match snapshot::request(&owner, Request::Close { preview: None, icon: None }) {
            Ok(_) => {
                inner.retained.remove(id);
                result
            }
            Err(e) => {
                inner.retained.insert(id.into(), owner);
                Err(failed(e))
            }
        }
    }
    pub fn create(&self, template: &Path, title: &str) -> Result<Entry> {
        let title = title.trim();
        if title.is_empty() {
            return Err(failed("Enter a name for this slop"));
        }
        let mut inner = self.inner.lock().unwrap();
        let temporary = self.root.join("documents").join(format!("new-{}.slop", crate::random_hex(12)));
        file::create_document(template, &temporary)?;
        let opened = file::open(&temporary, true)?;
        let id = opened.document_uuid.as_ref().ok_or_else(|| failed("Missing document identity"))?;
        std::fs::rename(&temporary, self.path(id))?;
        self.with_owner(&mut inner, id, |owner| {
            snapshot::request(
                owner,
                Request::Share { share: Share { room: id.clone(), endpoint: crate::LIBRARY_SHARE_ENDPOINT.into() } },
            )
            .map_err(failed)?;
            Ok(())
        })?;
        inner.db.execute(
            "INSERT INTO documents(id,title,template) VALUES(?1,?2,?3)",
            params![id, title, opened.app.metadata().slug],
        )?;
        let entry = self.entry(&inner, id)?;
        inner.db.execute("UPDATE documents SET artwork_version=?2 WHERE id=?1", params![id, entry.version])?;
        self.entry(&inner, id)
    }
    /// The same owner handles live edits and derived images. Local files are not library entries.
    pub fn publish_artwork(
        &self,
        owner: &Arc<Owner>,
        source_version: &str,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    ) -> Result<bool> {
        let Some(id) = owner.app().document_uuid.as_ref() else { return Ok(false) };
        if owner.path() != self.path(id) {
            return Ok(false);
        }
        let inner = self.inner.lock().unwrap();
        snapshot::request_typed(owner, Request::SetArtwork { source_version: source_version.into(), preview, icon })
            .map_err(Error::from)?;
        inner.db.execute("UPDATE documents SET artwork_version=?2 WHERE id=?1", params![id, source_version])?;
        Ok(true)
    }
    /// Automatic CloudKit snapshots are checkpoints for relay documents. Manual saves,
    /// background saves and ordinary library documents keep their existing behavior.
    pub fn stage_allowed(&self, id: &str, explicit: bool) -> Result<bool> {
        Self::valid_id(id)?;
        let inner = self.inner.lock().unwrap();
        self.checkpoint_allowed(&inner, id, explicit)
    }
    fn checkpoint_allowed(&self, inner: &Inner, id: &str, explicit: bool) -> Result<bool> {
        if explicit || inner.relay_checkpoint.get(id).is_none_or(|at| at.elapsed().as_secs() >= 60) {
            return Ok(true);
        }
        let entry = self.entry(inner, id)?;
        if entry.deleted {
            return Ok(true);
        }
        let share = if let Some(owner) = inner.live.get(id).and_then(Weak::upgrade) {
            owner.share().map_err(failed)?
        } else {
            Store::open(Path::new(&entry.path), Mode::Snapshot).map_err(failed)?.share().map_err(failed)?
        };
        Ok(share.is_none_or(|s| s.endpoint == crate::LIBRARY_SHARE_ENDPOINT))
    }
    pub fn stage_sync(&self, id: &str, explicit: bool) -> Result<Option<Upload>> {
        Self::valid_id(id)?;
        {
            let mut inner = self.inner.lock().unwrap();
            if !self.checkpoint_allowed(&inner, id, explicit)? {
                return Ok(None);
            }
            inner.relay_checkpoint.insert(id.into(), std::time::Instant::now());
        }
        self.stage(id).map(Some)
    }
    pub fn stage(&self, id: &str) -> Result<Upload> {
        Self::valid_id(id)?;
        let mut inner = self.inner.lock().unwrap();
        let mut entry = self.entry(&inner, id)?;
        let fields = inner.db.query_row("SELECT fields FROM documents WHERE id=?1", [id], |r| r.get(0))?;
        let asset_path = if entry.deleted {
            None
        } else {
            let path = self.root.join("uploads").join(format!("{}-{}.slop", id, crate::random_hex(12)));
            self.with_owner(&mut inner, id, |owner| {
                snapshot::request(owner, Request::Backup { destination: path.clone() }).map_err(failed)?;
                Ok(())
            })?;
            let info = match self.inspect(&path) {
                Ok(info) => info,
                Err(error) => {
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }
            };
            entry.asset_revision =
                snapshot::asset_revision(&info.version, &entry.artwork_version, &info.artwork_digest);
            if entry.artwork_version != info.version {
                entry.artwork_version.clear();
            }
            entry.version = info.version;
            Some(path.to_string_lossy().into_owned())
        };
        Ok(Upload { entry, asset_path, fields })
    }
    fn inspect(&self, path: &Path) -> Result<snapshot::Info> {
        if std::fs::metadata(path)?.len() > snapshot::MAX_FILE_BYTES {
            return Err(Error {
                issue: Issue::TooLarge,
                message: "Saved locally. This preview syncs slops up to 16 MiB.".into(),
            });
        }
        // Preserve the existing typed compatibility refusal before inspecting history.
        file::open(path, true)?;
        snapshot::inspect(path).map_err(failed)
    }
    #[allow(clippy::too_many_arguments, reason = "A single complete cloud record enters at this boundary")]
    pub fn receive(
        &self,
        id: &str,
        title: &str,
        template: &str,
        source: Option<&Path>,
        version: &str,
        fields: &[u8],
        deleted: bool,
    ) -> Result<()> {
        self.receive_with_artwork(id, title, template, source, version, fields, deleted, None, None)
    }
    #[allow(clippy::too_many_arguments, reason = "One complete cloud snapshot enters this boundary")]
    pub fn receive_with_artwork(
        &self,
        id: &str,
        title: &str,
        template: &str,
        source: Option<&Path>,
        version: &str,
        fields: &[u8],
        deleted: bool,
        artwork_version: Option<&str>,
        asset_revision: Option<&str>,
    ) -> Result<()> {
        Self::valid_id(id)?;
        let mut inner = self.inner.lock().unwrap();
        let mut adopted_artwork: Option<String> = None;
        let mut confirmed_asset = String::new();
        let exists: bool =
            inner.db.query_row("SELECT EXISTS(SELECT 1 FROM documents WHERE id=?1)", [id], |r| r.get(0))?;
        let tombstoned = exists && self.entry(&inner, id)?.deleted;
        if !deleted && tombstoned {
            inner.db.execute("UPDATE documents SET fields=?2,confirmed_deleted=0 WHERE id=?1", params![id, fields])?;
            return Ok(());
        } // A stale device cannot resurrect a deletion.
        if !deleted {
            let source = source.ok_or_else(|| failed("Missing document download"))?;
            let incoming = self.inspect(source)?;
            if incoming.id != id || incoming.version != version {
                return Err(failed("Cloud record does not match its document"));
            }
            let source_version = artwork_version.unwrap_or_default();
            confirmed_asset = snapshot::asset_revision(version, source_version, &incoming.artwork_digest);
            if asset_revision.is_some_and(|revision| revision != confirmed_asset) {
                return Err(failed("Cloud artwork revision does not match its asset"));
            }
            if exists && self.path(id).exists() {
                let local = Store::open(&self.path(id), Mode::Snapshot)?;
                let remote = Store::open(source, Mode::Snapshot)?;
                if !snapshot::compatible(&local.app().app, &remote.app().app) {
                    return Err(Error {
                        issue: Issue::AppMismatch,
                        message: "Sync paused: these copies use different data schemas or theme palettes. Your local edits are saved.".into(),
                    });
                }
                self.with_owner(&mut inner, id, |owner| {
                    snapshot::merge(owner, source).map_err(failed)?;
                    if source_version == version {
                        let store = Store::open(source, Mode::Snapshot)?;
                        let result = snapshot::request_typed(
                            owner,
                            Request::SetArtwork {
                                source_version: version.into(),
                                preview: store.artwork(file::Artwork::Preview)?,
                                icon: store.artwork(file::Artwork::Icon)?,
                            },
                        );
                        match result {
                            Ok(_) => adopted_artwork = Some(source_version.into()),
                            Err(error) if error.kind == FailureKind::Replaced => {}
                            Err(error) => return Err(failed(error)),
                        }
                    }
                    Ok(())
                })?;
            } else {
                snapshot::install(source, &self.path(id), id).map_err(failed)?;
                adopted_artwork = Some(source_version.into());
            }
            self.with_owner(&mut inner, id, |owner| {
                // The placeholder enables history retention; an existing local relay
                // membership is already shared and must survive a snapshot download.
                if owner.share()?.is_none() {
                    snapshot::request(
                        owner,
                        Request::Share {
                            share: Share { room: id.into(), endpoint: crate::LIBRARY_SHARE_ENDPOINT.into() },
                        },
                    )
                    .map_err(failed)?;
                }
                Ok(())
            })?;
        }
        let tx = inner.db.transaction()?;
        tx.execute("INSERT INTO documents(id,title,template,confirmed,fields,deleted,confirmed_deleted,remote_delete)
            VALUES(?1,?2,?3,?4,?5,?6,?6,?6) ON CONFLICT(id) DO UPDATE SET title=excluded.title,
            template=excluded.template, confirmed=CASE WHEN excluded.deleted THEN documents.confirmed ELSE excluded.confirmed END,
            fields=excluded.fields, deleted=excluded.deleted,confirmed_deleted=excluded.deleted,remote_delete=excluded.deleted,problem=NULL",
            params![id,title,template,version,fields,deleted])?;
        tx.execute(
            "UPDATE documents SET confirmed_asset=?2, artwork_version=COALESCE(?3,artwork_version) WHERE id=?1",
            params![id, confirmed_asset, adopted_artwork],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn block(&self, id: &str, title: &str, template: &str, message: &str) -> Result<()> {
        Self::valid_id(id)?;
        self.inner.lock().unwrap().db.execute(
            "INSERT INTO documents(id,title,template,problem) VALUES(?1,?2,?3,?4)
            ON CONFLICT(id) DO UPDATE SET problem=excluded.problem",
            params![id, title, template, message],
        )?;
        Ok(())
    }
    pub fn acknowledge(&self, id: &str, version: &str, fields: &[u8], deleted: bool) -> Result<()> {
        self.acknowledge_asset(id, version, None, fields, deleted)
    }
    pub fn acknowledge_asset(
        &self,
        id: &str,
        version: &str,
        asset_revision: Option<&str>,
        fields: &[u8],
        deleted: bool,
    ) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let entry = self.entry(&inner, id)?;
        let revision = asset_revision
            .map(String::from)
            .unwrap_or_else(|| if entry.version == version { entry.asset_revision } else { String::new() });
        let tx = inner.db.transaction()?;
        tx.execute(
            "UPDATE documents SET confirmed=?2, fields=?3, confirmed_deleted=?4 WHERE id=?1",
            params![id, version, fields, deleted],
        )?;
        tx.execute("UPDATE documents SET confirmed_asset=?2 WHERE id=?1", params![id, revision])?;
        tx.commit()?;
        Ok(())
    }
    pub fn delete(&self, id: &str) -> Result<()> {
        Self::valid_id(id)?;
        self.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE documents SET deleted=1,confirmed_deleted=0,remote_delete=0 WHERE id=?1", [id])?;
        Ok(())
    }
    /// Finalize only after an editor releases ownership. Remote dirty edits become a
    /// new local identity; a tombstone always remains for the canonical cloud identity.
    pub fn cleanup(&self) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        let ids = inner
            .db
            .prepare("SELECT id FROM documents WHERE deleted=1 AND confirmed_deleted=1")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for id in ids {
            if !self.path(&id).exists() {
                continue;
            }
            if inner
                .live
                .get(&id)
                .and_then(Weak::upgrade)
                .is_some_and(|o| snapshot::request(&o, Request::State).is_ok())
            {
                continue;
            }
            let owner = Owner::open(&self.path(&id), Mode::Document, Arc::new(|_| {}))?;
            let (remote, confirmed, title, template): (bool, String, String, String) = inner.db.query_row(
                "SELECT remote_delete,confirmed,title,template FROM documents WHERE id=?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
            if remote && snapshot::inspect(&self.path(&id)).map_err(failed)?.version != confirmed {
                let temporary = self.root.join("documents").join(format!("recovered-{}.slop", crate::random_hex(12)));
                Store::import_copy(&self.path(&id), &temporary)?;
                let rescued = snapshot::inspect(&temporary).map_err(failed)?.id;
                std::fs::rename(&temporary, self.path(&rescued))?;
                inner.db.execute(
                    "INSERT INTO documents(id,title,template,local_only) VALUES(?1,?2,?3,1)",
                    params![rescued, format!("{title} — Deleted on another device"), template],
                )?;
            }
            snapshot::request(&owner, Request::Close { preview: None, icon: None }).map_err(failed)?;
            std::fs::remove_file(self.path(&id))?;
        }
        Ok(())
    }
    pub fn engine_state(&self) -> Result<Option<Vec<u8>>> {
        Ok(self.inner.lock().unwrap().db.query_row("SELECT value FROM meta WHERE key='engine'", [], |r| r.get(0)).ok())
    }
    pub fn save_engine_state(&self, bytes: &[u8]) -> Result<()> {
        self.inner.lock().unwrap().db.execute(
            "INSERT INTO meta VALUES('engine',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [bytes],
        )?;
        Ok(())
    }
    pub fn release_upload(&self, path: &Path) -> Result<()> {
        if path.parent() != Some(self.root.join("uploads").as_path()) {
            return Err(failed("Not a library upload"));
        }
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
