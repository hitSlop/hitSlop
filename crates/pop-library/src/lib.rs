//! Device-local document index and account-scoped cloud cache. Bytes synchronize
//! through the existing owner and persistent cloud protocol, never through catalog assets.
mod paths;
use pop_runtime::owner::{Failure, FailureKind, Owner, Request};
use pop_sharing_client::{Client, ClientStatus, local};
pub use pop_sharing_protocol::document::{AccessState, DocumentEntry, DocumentRole};
use pop_storage::{
    file,
    store::{Mode, Store},
};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
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
fn failed(error: impl std::fmt::Display) -> Error {
    Error { issue: Issue::Failed, message: error.to_string() }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        failed(error)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        failed(error)
    }
}
impl From<Failure> for Error {
    fn from(error: Failure) -> Self {
        let issue = if error.reason == Some(pop_core::Code::RequiresUpdate) {
            Issue::RequiresUpdate
        } else if matches!(error.kind, FailureKind::Busy | FailureKind::Locked | FailureKind::Closing) {
            Issue::Busy
        } else {
            Issue::Failed
        };
        Self { issue, message: error.message }
    }
}
impl From<pop_storage::store::Error> for Error {
    fn from(error: pop_storage::store::Error) -> Self {
        Failure::from(error).into()
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
    pub account_bound: bool,
    pub downloaded: bool,
    pub problem: Option<String>,
    pub role: Option<DocumentRole>,
    pub revision: u64,
    pub sharing_enabled: Option<bool>,
    pub inbox: bool,
}
struct Inner {
    db: Connection,
    live: HashMap<String, Weak<Owner>>,
    clients: HashMap<String, Arc<Client>>,
    retained: HashMap<String, Arc<Owner>>,
    token: Option<String>,
}
pub struct Library {
    root: PathBuf,
    account: String,
    scope: String,
    inner: Mutex<Inner>,
    active: AtomicBool,
    generation: AtomicU64,
    projections: Mutex<()>,
}
impl Library {
    pub fn open(root: &Path, account: &str) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root)?;
        let root = std::fs::canonicalize(root)?;
        let db = Connection::open(root.join("cloud-index.sqlite"))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS documents(account TEXT NOT NULL,id TEXT NOT NULL,title TEXT NOT NULL,template TEXT NOT NULL,path TEXT NOT NULL,version TEXT NOT NULL DEFAULT '',role TEXT,revision INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0,problem TEXT,pending_enable INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(account,id));")?;
        let columns = db
            .prepare("PRAGMA table_info(documents)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for column in ["artwork_version", "artwork_digest", "file_stamp"] {
            if !columns.iter().any(|name| name == column) {
                db.execute_batch(&format!("ALTER TABLE documents ADD COLUMN {column} TEXT NOT NULL DEFAULT ''"))?;
            }
        }
        if !columns.iter().any(|name| name == "catalog_seen") {
            db.execute_batch("ALTER TABLE documents ADD COLUMN catalog_seen INTEGER NOT NULL DEFAULT 0")?;
        }
        if !columns.iter().any(|name| name == "sharing_enabled") {
            db.execute_batch("ALTER TABLE documents ADD COLUMN sharing_enabled INTEGER")?;
        }
        db.execute_batch("CREATE TABLE IF NOT EXISTS legacy_sources(path TEXT PRIMARY KEY,id TEXT NOT NULL,fingerprint TEXT NOT NULL);")?;
        paths::migrate(&db, &root)?;
        if !columns.iter().any(|name| name == "opened") {
            let migration = db.unchecked_transaction()?;
            migration.execute_batch("ALTER TABLE documents ADD COLUMN opened INTEGER NOT NULL DEFAULT 1")?;
            // Earlier caches did not track opens. Only an absent remote file with
            // no saved version/stamp reliably identifies a never-downloaded entry.
            let unseen = migration.prepare("SELECT account,id,path FROM documents WHERE account<>'' AND role IS NOT NULL AND version='' AND file_stamp='' AND deleted=0")?
                .query_map([], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))?
                .collect::<std::result::Result<Vec<_>,_>>()?;
            for (account, id, path) in unseen {
                if !paths::resolve(&root, &path)?.is_file() {
                    migration
                        .execute("UPDATE documents SET opened=0 WHERE account=?1 AND id=?2", params![account, id])?;
                }
            }
            migration.commit()?;
        }

        db.execute_batch("CREATE TABLE IF NOT EXISTS catalog_writes(account TEXT NOT NULL,id TEXT NOT NULL,revision INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(account,id)); CREATE TABLE IF NOT EXISTS catalog_clock(account TEXT PRIMARY KEY,revision INTEGER NOT NULL);")?;
        let scope = crate::hex(&Sha256::digest(account.as_bytes()));
        std::fs::create_dir_all(root.join("documents"))?;
        std::fs::create_dir_all(root.join("accounts").join(&scope).join("documents"))?;
        Ok(Arc::new(Self {
            root,
            account: account.into(),
            scope,
            inner: Mutex::new(Inner {
                db,
                live: HashMap::new(),
                clients: HashMap::new(),
                retained: HashMap::new(),
                token: None,
            }),
            active: AtomicBool::new(true),
            generation: AtomicU64::new(1),
            projections: Mutex::new(()),
        }))
    }
    fn check(&self) -> Result<()> {
        if !self.active.load(Ordering::Acquire) {
            Err(Error { issue: Issue::AccountChanged, message: "The cloud account changed".into() })
        } else {
            Ok(())
        }
    }
    fn cloud(&self) -> Result<()> {
        self.check()?;
        if self.account.is_empty() {
            Err(Error { issue: Issue::AccountChanged, message: "Sign in to enable cloud sync".into() })
        } else {
            Ok(())
        }
    }
    fn valid(id: &str) -> Result<()> {
        if pop_sharing_protocol::valid_copy(id) { Ok(()) } else { Err(failed("Invalid document identity")) }
    }
    fn operation(&self) -> Result<u64> {
        self.cloud()?;
        Ok(self.generation.load(Ordering::Acquire))
    }
    fn current_operation(&self, generation: u64) -> Result<()> {
        self.check()?;
        if generation != self.generation.load(Ordering::Acquire) {
            return Err(Error { issue: Issue::AccountChanged, message: "The cloud account changed".into() });
        }
        Ok(())
    }
    fn with_owner<T>(&self, id: &str, work: impl FnOnce(&Arc<Owner>) -> Result<T>) -> Result<T> {
        let (entry, existing, temporary) = {
            let inner = self.inner.lock().unwrap();
            let retained = inner.retained.get(id).cloned();
            let live = inner.live.get(id).and_then(Weak::upgrade);
            (self.row(&inner, id)?, retained.clone().or(live.clone()), retained.is_some() || live.is_none())
        };
        let existing = existing.filter(|owner| !matches!(local::request_typed(owner, Request::State), Err(error) if error.kind == FailureKind::Closed));
        let temporary = temporary || existing.is_none();
        let owner = match existing {
            Some(owner) => owner,
            None => Arc::new(Owner::open(Path::new(&entry.path), Mode::Document, Arc::new(|_| {}))?),
        };
        let result = work(&owner);
        if temporary {
            match local::request_typed(&owner, Request::Close { preview: None, icon: None }) {
                Ok(_) => {
                    self.inner.lock().unwrap().retained.remove(id);
                }
                Err(error) => {
                    self.inner.lock().unwrap().retained.insert(id.into(), owner);
                    return Err(error.into());
                }
            }
        }
        result
    }
    fn state_dir(&self) -> PathBuf {
        self.root.join("accounts").join(&self.scope).join("relay-state")
    }
    fn cached_path(&self, id: &str) -> PathBuf {
        self.root.join("accounts").join(&self.scope).join("documents").join(format!("{id}.pop"))
    }
    fn row(&self, inner: &Inner, id: &str) -> Result<Entry> {
        Self::valid(id)?;
        let row=inner.db.query_row("SELECT account,title,template,path,version,role,revision,deleted,problem FROM documents WHERE id=?1 AND (account=?2 OR account='') ORDER BY (account=?2) DESC LIMIT 1",params![id,self.account],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,i64>(6)?,row.get::<_,bool>(7)?,row.get::<_,Option<String>>(8)?)))?;
        let resolved_path = paths::resolve(&self.root, &row.3)?;
        let downloaded = resolved_path.is_file();
        let role = match row.5.as_deref() {
            Some("owner") => Some(DocumentRole::Owner),
            Some("editor") => Some(DocumentRole::Editor),
            _ => None,
        };
        let (source_version, digest, saved_stamp, pending, sharing_enabled, opened): (String, String, String, bool, Option<bool>, bool) = inner.db.query_row(
            "SELECT artwork_version,artwork_digest,file_stamp,pending_enable,sharing_enabled,opened FROM documents WHERE account=?1 AND id=?2",
            params![row.0, id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )?;
        let mut version = row.4;
        let stamp = if downloaded { file_stamp(&resolved_path).ok() } else { None };
        let (confirmed, receipt_problem) = if role.is_some() {
            match pop_sharing_client::confirmed_saved_version(&self.state_dir(), id) {
                Ok(version) => (version, None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };
        let client_synced =
            inner.clients.get(id).is_some_and(|client| client.status().phase == pop_sharing_client::Phase::Synced);
        if client_synced && let Some(confirmed) = &confirmed {
            version = confirmed.clone();
            inner.db.execute(
                "UPDATE documents SET version=?3,file_stamp=?4 WHERE account=?1 AND id=?2",
                params![row.0, id, version, stamp.as_deref().unwrap_or("")],
            )?;
        }
        let dirty = role.is_some()
            && (pending
                || downloaded
                    && (confirmed.as_deref() != Some(&version)
                        || (!client_synced && stamp.as_deref() != Some(&saved_stamp))
                        || inner
                            .clients
                            .get(id)
                            .is_some_and(|client| client.status().phase != pop_sharing_client::Phase::Synced)));
        let asset_revision = local::asset_revision(&version, &source_version, &digest);
        let artwork_version = if source_version == version { source_version } else { String::new() };
        Ok(Entry {
            id: id.into(),
            title: row.1,
            template: row.2,
            path: resolved_path.to_string_lossy().into_owned(),
            version,
            asset_revision,
            artwork_version,
            dirty,
            deleted: row.7,
            local_only: role.is_none(),
            account_bound: !row.0.is_empty(),
            downloaded,
            sharing_enabled,
            inbox: !opened && !row.0.is_empty() && role.is_some() && !row.7,
            problem: row.8.or(receipt_problem),
            role,
            revision: u64::try_from(row.6).map_err(failed)?,
        })
    }
    pub fn entries(&self) -> Result<Vec<Entry>> {
        self.check()?;
        self.discover_recoveries()?;
        let inner = self.inner.lock().unwrap();
        let ids = inner
            .db
            .prepare("SELECT DISTINCT id FROM documents WHERE (account=?1 OR account='') ORDER BY title")?
            .query_map([&self.account], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter().map(|id| self.row(&inner, &id)).collect()
    }
    pub fn receive_catalog(&self, entries: &[DocumentEntry]) -> Result<()> {
        self.cloud()?;
        let _projection = self.projections.lock().unwrap();
        let mut accepted = Vec::new();
        for entry in entries {
            Self::valid(&entry.document_id)?;
            if entry.catalog_revision > i64::MAX as u64 || entry.title.len() > 4096 || entry.template_id.len() > 1024 {
                return Err(failed("Invalid cloud catalog entry"));
            }
            let prior = {
                let inner = self.inner.lock().unwrap();
                inner
                    .db
                    .query_row(
                        "SELECT revision,deleted,path FROM documents WHERE account=?1 AND id=?2",
                        params![self.account, entry.document_id],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, bool>(1)?, row.get::<_, String>(2)?)),
                    )
                    .optional()?
            };
            if entry.deleted
                && prior.as_ref().is_some_and(|prior| {
                    !prior.1 && paths::resolve(&self.root, &prior.2).is_ok_and(|path| path.is_file())
                })
            {
                if let Some(client) = self.connection(&entry.document_id) {
                    client.pause();
                }
                // Preserve an independently openable copy before the authoritative
                // tombstone can hide this account's former cache/window entry.
                let title = {
                    let inner = self.inner.lock().unwrap();
                    inner.db.query_row(
                        "SELECT title FROM documents WHERE account=?1 AND id=?2",
                        params![self.account, entry.document_id],
                        |row| row.get::<_, String>(0),
                    )?
                };
                self.account_recovery(&entry.document_id, &format!("Recovered edits — {title}"))?;
            }
            accepted.push(entry);
        }
        let mut inner = self.inner.lock().unwrap();
        let tx = inner.db.transaction()?;
        for entry in &accepted {
            let path: String = tx
                .query_row("SELECT path FROM documents WHERE id=?1 AND account=''", [&entry.document_id], |row| {
                    row.get(0)
                })
                .optional()?
                .map_or_else(|| paths::store(&self.root, &self.cached_path(&entry.document_id)), Ok)?;
            let role = match entry.role {
                DocumentRole::Owner => "owner",
                DocumentRole::Editor => "editor",
            };
            tx.execute("INSERT INTO documents(account,id,title,template,path,role,revision,deleted,catalog_seen,opened) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,1,coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0)) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role=excluded.role,revision=excluded.revision,deleted=excluded.deleted,catalog_seen=1",params![self.account,entry.document_id,entry.title,entry.template_id,path,role,i64::try_from(entry.catalog_revision).map_err(failed)?,entry.deleted])?;
        }
        tx.commit()?;
        let stopped = accepted
            .iter()
            .filter(|entry| entry.deleted)
            .filter_map(|entry| inner.clients.remove(&entry.document_id))
            .collect::<Vec<_>>();
        drop(inner);
        for client in stopped {
            client.pause();
            client.flush().map_err(failed)?;
        }
        Ok(())
    }
    pub fn create(&self, template: &Path, title: &str) -> Result<Entry> {
        self.check()?;
        let temporary = self.root.join("documents").join(format!("new-{}.pop", random_hex(16)));
        file::create_document(template, &temporary)?;
        let opened = file::open(&temporary, true)?;
        let id = opened.document_uuid.ok_or_else(|| failed("Missing document identity"))?;
        let path = self.root.join("documents").join(format!("{id}.pop"));
        std::fs::rename(&temporary, &path)?;
        self.adopt(&path, title)
    }
    pub fn adopt(&self, source: &Path, title: &str) -> Result<Entry> {
        self.check()?;
        let source = file::resolve(source)?;
        let info = local::inspect(&source).map_err(failed)?;
        let store = Store::open(&source, Mode::Snapshot)?;
        let template = store.app().app.metadata().slug.clone();
        store.close()?;
        let inner = self.inner.lock().unwrap();
        let existing = paths::indexed(&inner.db, &self.root, &info.id)?;
        if !existing.is_empty() {
            if existing.iter().any(|(_, path)| path != &source) {
                return Err(Error {
                    issue: Issue::AppMismatch,
                    message: "A different local file has this document identity".into(),
                });
            }
            inner.db.execute(
                "UPDATE documents SET opened=1 WHERE id=?1 AND (account=?2 OR account='')",
                params![info.id, self.account],
            )?;
            return self.row(&inner, &info.id);
        }
        inner.db.execute(
            "INSERT INTO documents(account,id,title,template,path,version) VALUES('',?1,?2,?3,?4,?5)",
            params![info.id, title.trim(), template, paths::store(&self.root, &source)?, info.version],
        )?;
        drop(inner);
        self.cache_file(&info.id, Some(&info.version))?;
        let inner = self.inner.lock().unwrap();
        self.row(&inner, &info.id)
    }
    /// One-time local cutover: no CloudKit fields are imported and no existing
    /// cloud/index binding is replaced. Divergent duplicate identities become visible copies.
    pub fn adopt_local_cache(&self, source: &Path, title: &str) -> Result<Option<Entry>> {
        self.check()?;
        let source = file::resolve(source)?;
        let info = local::inspect(&source).map_err(failed)?;
        let source_text = paths::store(&self.root, &source)?;
        let fingerprint = format!("{}:{}", hex(&info.app_digest), info.fingerprint);
        let (mapped, existing) = {
            let inner = self.inner.lock().unwrap();
            let mapped = inner
                .db
                .query_row("SELECT id,fingerprint FROM legacy_sources WHERE path=?1", [&source_text], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .optional()?;
            let existing = paths::indexed(&inner.db, &self.root, &info.id)?;
            (mapped, existing)
        };
        if let Some((id, saved)) = &mapped
            && ((id == &info.id && existing.iter().any(|(_, path)| path == &source))
                || saved == &fingerprint
                || existing.iter().any(|(account, path)| !account.is_empty() && path == &source))
        {
            let inner = self.inner.lock().unwrap();
            return match self.row(&inner, id) {
                Ok(entry) if !entry.deleted => Ok(Some(entry)),
                _ => Ok(None),
            };
        }
        if existing.iter().any(|(account, path)| !account.is_empty() && path == &source) {
            let inner = self.inner.lock().unwrap();
            let result = self.row(&inner, &info.id).ok();
            inner.db.execute("INSERT INTO legacy_sources(path,id,fingerprint) VALUES(?1,?2,?3) ON CONFLICT(path) DO UPDATE SET id=excluded.id,fingerprint=excluded.fingerprint",params![source_text,info.id,fingerprint])?;
            return Ok(result);
        }
        let mut target = None;
        for (_, path) in &existing {
            if path == &source {
                continue;
            }
            if let Ok(known) = local::inspect(path)
                && known.app_digest == info.app_digest
                && known.fingerprint == info.fingerprint
            {
                target = Some(info.id.clone());
                break;
            }
        }
        let entry = if target.is_some() {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, &info.id).ok()
        } else if existing.is_empty() || existing.iter().all(|(account, path)| account.is_empty() && path == &source) {
            let mut label = title.to_owned();
            if let Some(parent) = source.parent().and_then(Path::parent) {
                let old = parent.join("library.sqlite");
                if old.is_file()
                    && let Ok(db) = Connection::open_with_flags(old, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                    && let Ok(saved) = db
                        .query_row("SELECT title FROM documents WHERE id=?1", [&info.id], |row| row.get::<_, String>(0))
                {
                    label = saved;
                }
            }
            let entry = self.adopt(&source, &label)?;
            self.with_owner(&entry.id, |owner| {
                local::request(owner, Request::Unshare).map_err(failed)?;
                local::request(owner, Request::Flush).map_err(failed)?;
                Ok(())
            })?;
            Some(entry)
        } else {
            let owner = Owner::open(&source, Mode::Document, Arc::new(|_| {}))?;
            let destination = self.root.join("documents").join(format!("recovered-{}.pop", random_hex(16)));
            let copied =
                local::request(&owner, Request::Copy { destination: destination.clone(), preview: None, icon: None })
                    .map_err(failed);
            let closed = local::request(&owner, Request::Close { preview: None, icon: None }).map_err(failed);
            copied?;
            closed?;
            Some(self.adopt(&destination, &format!("Recovered edits — {title}"))?)
        };
        let mapped_id = entry.as_ref().map(|entry| entry.id.clone()).or(target).unwrap_or(info.id);
        self.inner.lock().unwrap().db.execute("INSERT INTO legacy_sources(path,id,fingerprint) VALUES(?1,?2,?3) ON CONFLICT(path) DO UPDATE SET id=excluded.id,fingerprint=excluded.fingerprint",params![source_text,mapped_id,fingerprint])?;
        Ok(entry)
    }
    pub fn register(&self, owner: &Arc<Owner>) -> Result<()> {
        self.check()?;
        let Some(id) = owner.app().document_uuid.as_deref() else { return Ok(()) };
        let token = {
            let mut inner = self.inner.lock().unwrap();
            let Ok(entry) = self.row(&inner, id) else { return Ok(()) };
            if entry.deleted || Path::new(&entry.path) != owner.path() {
                return Ok(());
            }
            inner.db.execute(
                "UPDATE documents SET opened=1 WHERE account=?1 AND id=?2",
                params![if entry.account_bound { self.account.as_str() } else { "" }, id],
            )?;
            inner.live.insert(id.into(), Arc::downgrade(owner));
            if entry.local_only { None } else { inner.token.clone() }
        };
        if let Some(token) = token {
            self.attach(id, owner.clone(), &token)?;
        }
        Ok(())
    }
    pub fn unregister(&self, id: &str) -> Result<()> {
        let (client, owner, deleted) = {
            let inner = self.inner.lock().unwrap();
            (
                inner.clients.get(id).cloned(),
                inner.live.get(id).and_then(Weak::upgrade),
                self.row(&inner, id).is_ok_and(|entry| entry.deleted),
            )
        };
        if let Some(client) = client {
            client.pause();
        }
        if let Some(owner) = owner {
            match local::request_typed(&owner, Request::Flush) {
                Ok(_) => {}
                Err(error) if error.kind == FailureKind::Closed => {}
                Err(error) => return Err(error.into()),
            }
        }
        let downloaded = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id).is_ok_and(|entry| entry.downloaded)
        };
        if downloaded {
            self.cache_file(id, None)?;
        }
        if deleted && downloaded {
            self.account_recovery(id, "Recovered edits")?;
        }
        let mut inner = self.inner.lock().unwrap();
        inner.live.remove(id);
        inner.clients.remove(id);
        Ok(())
    }
    fn attach(&self, id: &str, owner: Arc<Owner>, token: &str) -> Result<Arc<Client>> {
        let generation = self.operation()?;
        let previous = self.inner.lock().unwrap().clients.remove(id);
        if let Some(previous) = previous {
            previous.pause();
            previous.flush().map_err(failed)?;
        }
        let client = Arc::new(Client::cloud(owner, id, token, &self.state_dir()).map_err(failed)?);
        let creator = self
            .inner
            .lock()
            .unwrap()
            .db
            .query_row(
                "SELECT role='owner' FROM documents WHERE account=?1 AND id=?2",
                params![self.account, id],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        client.set_creator(creator);
        client.connect().map_err(failed)?;
        self.current_operation(generation)?;
        let mut inner = self.inner.lock().unwrap();
        let entry = self.row(&inner, id)?;
        if entry.deleted || entry.local_only {
            drop(inner);
            client.pause();
            return Err(failed("Cloud access was removed"));
        }
        inner.clients.insert(id.into(), client.clone());
        Ok(client)
    }
    pub fn sync(&self, token: &str) -> Result<()> {
        self.cloud()?;
        let pending = {
            let inner = self.inner.lock().unwrap();
            inner
                .db
                .prepare("SELECT id FROM documents WHERE account=?1 AND pending_enable=1 AND deleted=0")?
                .query_map([&self.account], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut failure = None;
        let owners = {
            let mut inner = self.inner.lock().unwrap();
            inner.token = Some(token.into());
            inner
                .live
                .iter()
                .filter_map(|(id, owner)| owner.upgrade().map(|owner| (id.clone(), owner)))
                .collect::<Vec<_>>()
        };
        for (id, owner) in owners {
            let entry = {
                let inner = self.inner.lock().unwrap();
                self.row(&inner, &id)?
            };
            if !entry.local_only
                && !entry.deleted
                && let Err(error) = self.attach(&id, owner, token)
            {
                failure = Some(error);
            }
        }
        for id in pending {
            if let Err(error) = self.enable_sync(&id, token) {
                failure = Some(error);
            }
        }
        if let Err(error) = self.repair_catalog(token) {
            failure = Some(error);
        }
        failure.map_or(Ok(()), Err)
    }
    pub fn connection(&self, id: &str) -> Option<Arc<Client>> {
        self.inner.lock().unwrap().clients.get(id).cloned()
    }
    pub fn status(&self, id: &str) -> Option<ClientStatus> {
        self.connection(id).map(|client| client.status())
    }
    pub fn enable_sync(&self, id: &str, token: &str) -> Result<Entry> {
        let generation = self.operation()?;
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        // Selecting synchronization durably binds this exact local file to the account.
        {
            let mut inner = self.inner.lock().unwrap();
            let tx = inner.db.transaction()?;
            tx.execute("INSERT INTO documents(account,id,title,template,path,version,role,pending_enable) VALUES(?1,?2,?3,?4,?5,?6,'owner',1) ON CONFLICT(account,id) DO UPDATE SET pending_enable=1,path=excluded.path,role='owner',deleted=0",params![self.account,id,entry.title,entry.template,paths::store(&self.root,Path::new(&entry.path))?,entry.version])?;
            tx.execute("DELETE FROM documents WHERE account='' AND id=?1", [id])?;
            tx.commit()?;
        }
        let outcome = self.with_owner(id, |owner| {
            pop_sharing_client::bootstrap(owner, token, &self.state_dir()).map_err(failed)?;
            self.current_operation(generation)?;
            pop_sharing_client::update_document(id, &entry.title, &entry.template, token).map_err(failed)?;
            // Sharing is a presentation cache. Bootstrap completion must not
            // depend on another request made only to populate that optional hint.
            self.current_operation(generation)
        });
        if let Err(error) = outcome {
            self.inner.lock().unwrap().db.execute(
                "UPDATE documents SET problem=?3 WHERE account=?1 AND id=?2",
                params![self.account, id, error.message],
            )?;
            return Err(error);
        }
        let live = {
            let mut inner = self.inner.lock().unwrap();
            let tx = inner.db.transaction()?;
            tx.execute(
                "UPDATE documents SET pending_enable=0,problem=NULL WHERE account=?1 AND id=?2",
                params![self.account, id],
            )?;
            tx.execute("DELETE FROM documents WHERE account='' AND id=?1", [id])?;
            tx.commit()?;
            inner.token = Some(token.into());
            inner.live.get(id).and_then(Weak::upgrade)
        };
        self.queue_catalog(id, false)?;
        if let Some(owner) = live {
            self.attach(id, owner, token)?;
        }
        let inner = self.inner.lock().unwrap();
        self.row(&inner, id)
    }
    pub fn download(&self, id: &str, token: &str) -> Result<Entry> {
        let generation = self.operation()?;
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        if entry.deleted {
            return Err(failed("Cloud document is no longer available"));
        }
        let access = pop_sharing_client::authorized_access(id, token).map_err(failed)?;
        self.current_operation(generation)?;
        let Some(access) = access else {
            self.receive_catalog(&[DocumentEntry {
                document_id: id.into(),
                title: entry.title,
                template_id: entry.template,
                app_revision: None,
                preview_ref: None,
                modified_at: 0,
                role: entry.role.ok_or_else(|| failed("Local document is not a cloud download"))?,
                access_generation: 0,
                catalog_revision: 0,
                deleted: true,
            }])?;
            self.queue_catalog(id, true)?;
            return Err(failed("Cloud document access was removed; local edits were retained"));
        };
        self.remember_access(id, &access)?;
        if entry.downloaded {
            return self.row(&self.inner.lock().unwrap(), id);
        }
        let resolved = pop_sharing_client::resolve(id, token)
            .map_err(failed)?
            .ok_or_else(|| failed("Cloud document is not ready"))?;
        self.current_operation(generation)?;
        resolved.install(Path::new(&entry.path), &self.state_dir()).map_err(failed)?;
        self.cache_file(id, None)?;
        let inner = self.inner.lock().unwrap();
        self.row(&inner, id)
    }
    fn remember_access(&self, id: &str, access: &AccessState) -> Result<()> {
        self.cloud()?;
        Self::valid(id)?;
        if access.document != id {
            return Err(failed("Access response belongs to another document"));
        }
        self.inner.lock().unwrap().db.execute(
            "UPDATE documents SET sharing_enabled=?3 WHERE account=?1 AND id=?2 AND role IS NOT NULL AND deleted=0",
            params![self.account, access.document, access.sharing],
        )?;
        Ok(())
    }
    pub fn access(&self, id: &str, token: &str) -> Result<AccessState> {
        let generation = self.operation()?;
        let access = pop_sharing_client::access(id, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access)?;
        Ok(access)
    }
    pub fn sharing(&self, id: &str, enabled: bool, token: &str) -> Result<AccessState> {
        let generation = self.operation()?;
        let access = pop_sharing_client::set_sharing(id, enabled, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access)?;
        Ok(access)
    }
    pub fn join(&self, link: &str, token: &str) -> Result<Entry> {
        let generation = self.operation()?;
        let access = pop_sharing_client::join_member(link, token).map_err(failed)?;
        let id = access.document;
        let metadata = pop_sharing_client::document_metadata(&id, token).map_err(failed)?;
        self.current_operation(generation)?;
        let path = self.cached_path(&id);
        {
            let inner = self.inner.lock().unwrap();
            inner.db.execute("INSERT INTO documents(account,id,title,template,path,role,opened,sharing_enabled) VALUES(?1,?2,?3,?4,?5,'editor',coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0),?6) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role='editor',deleted=0,sharing_enabled=excluded.sharing_enabled",params![self.account,id,metadata.title,metadata.template_id,paths::store(&self.root,&path)?,access.sharing])?;
        }
        self.queue_catalog(&id, false)?;
        self.download(&id, token)
    }
    pub fn leave(&self, id: &str, token: &str) -> Result<()> {
        self.cloud()?;
        self.unregister(id)?;
        let downloaded = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?.downloaded
        };
        if downloaded {
            self.local_copy(id, "Retained local copy")?;
        }
        pop_sharing_client::leave(id, token).map_err(failed)?;
        self.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE documents SET deleted=1 WHERE account=?1 AND id=?2", params![self.account, id])?;
        self.queue_catalog(id, true)?;
        Ok(())
    }
    pub fn delete_everywhere(&self, id: &str, token: &str) -> Result<()> {
        self.cloud()?;
        self.unregister(id)?;
        pop_sharing_client::delete_document(id, token).map_err(failed)?;
        self.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE documents SET deleted=1 WHERE account=?1 AND id=?2", params![self.account, id])?;
        self.queue_catalog(id, true)?;
        Ok(())
    }
    pub fn remove_download(&self, id: &str) -> Result<()> {
        self.cloud()?;
        let entry = {
            let inner = self.inner.lock().unwrap();
            if inner.live.get(id).and_then(Weak::upgrade).is_some() {
                return Err(Error {
                    issue: Issue::Busy,
                    message: "Close this pop before removing its download".into(),
                });
            }
            self.row(&inner, id)?
        };
        if entry.local_only {
            return Err(failed("Local-only documents have no cloud download"));
        }
        if !entry.downloaded {
            return Ok(());
        }
        if !Path::new(&entry.path).starts_with(self.root.join("accounts").join(&self.scope)) {
            return Err(failed("This document is outside the cloud cache; its original file is retained"));
        }
        if !pop_sharing_client::saved_confirmed(Path::new(&entry.path), &self.state_dir(), id).map_err(failed)? {
            return Err(failed("This download has unconfirmed edits; synchronize it before removal"));
        }
        self.unregister(id)?;
        file::delete_document(Path::new(&entry.path))?;
        Ok(())
    }
    pub fn delete_local(&self, id: &str) -> Result<()> {
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        if !entry.local_only {
            return Err(failed("Use cloud deletion for synchronized documents"));
        }
        self.unregister(id)?;
        file::delete_document(Path::new(&entry.path))?;
        self.inner.lock().unwrap().db.execute(
            "DELETE FROM documents WHERE account=?1 AND id=?2",
            params![if entry.account_bound { &self.account } else { "" }, id],
        )?;
        Ok(())
    }
    fn cache_file(&self, id: &str, artwork_source: Option<&str>) -> Result<()> {
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        let info = local::inspect(Path::new(&entry.path)).map_err(failed)?;
        let stamp = file_stamp(Path::new(&entry.path))?;
        let inner = self.inner.lock().unwrap();
        inner.db.execute("UPDATE documents SET version=?3,artwork_digest=?4,file_stamp=?5,artwork_version=coalesce(?6,artwork_version) WHERE account=?1 AND id=?2",params![if entry.account_bound { &self.account } else { "" },id,info.version,info.artwork_digest,stamp,artwork_source])?;
        Ok(())
    }
    fn repair_catalog(&self, token: &str) -> Result<()> {
        let generation = self.operation()?;
        let registered = pop_sharing_client::registered_documents(token).map_err(failed)?;
        for document in registered.documents {
            Self::valid(&document.document)?;
            let needs_repair: bool = self
                .inner
                .lock()
                .unwrap()
                .db
                .query_row(
                    "SELECT catalog_seen=0 AND NOT EXISTS(SELECT 1 FROM catalog_writes WHERE account=?1 AND id=?2) FROM documents WHERE account=?1 AND id=?2",
                    params![self.account, document.document],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(true);
            let access = match pop_sharing_client::authorized_access(&document.document, token) {
                Ok(access) => access,
                Err(_) if !needs_repair => {
                    // An optional cache refresh cannot stop other documents or
                    // overwrite the last authoritative sharing state.
                    self.current_operation(generation)?;
                    continue;
                }
                Err(error) => return Err(failed(error)),
            };
            self.current_operation(generation)?;
            if let Some(access) = &access {
                self.remember_access(&document.document, access)?;
            }
            if !needs_repair {
                continue;
            }
            if let Some(access) = access {
                let Some(metadata) =
                    pop_sharing_client::ready_document_metadata(&document.document, token).map_err(failed)?
                else {
                    continue;
                };
                self.current_operation(generation)?;
                let inner = self.inner.lock().unwrap();
                inner.db.execute("INSERT INTO documents(account,id,title,template,path,role,opened,sharing_enabled) VALUES(?1,?2,?3,?4,?5,?6,coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0),?7) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role=excluded.role,deleted=0",params![self.account,document.document,metadata.title,metadata.template_id,paths::store(&self.root,&self.cached_path(&document.document))?,if access.role == DocumentRole::Owner {"owner"} else {"editor"},access.sharing])?;
                drop(inner);
                self.queue_catalog(&document.document, false)?;
            }
        }
        Ok(())
    }
    fn queue_catalog(&self, id: &str, deleted: bool) -> Result<()> {
        self.cloud()?;
        let mut inner = self.inner.lock().unwrap();
        let entry = self.row(&inner, id)?;
        let tx = inner.db.transaction()?;
        let previous: i64 = tx
            .query_row("SELECT revision FROM catalog_clock WHERE account=?1", [&self.account], |row| row.get(0))
            .optional()?
            .unwrap_or(0);
        let revision = previous.checked_add(1).ok_or_else(|| failed("Catalog revision exhausted"))?;
        let modified_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(failed)?
            .as_millis()
            .try_into()
            .map_err(failed)?;
        let desired = DocumentEntry {
            document_id: id.into(),
            title: entry.title,
            template_id: entry.template,
            app_revision: None,
            preview_ref: None,
            modified_at,
            role: entry.role.ok_or_else(|| failed("Local document has no cloud catalog"))?,
            access_generation: 0,
            catalog_revision: revision as u64,
            deleted,
        };
        tx.execute("INSERT INTO catalog_clock(account,revision) VALUES(?1,?2) ON CONFLICT(account) DO UPDATE SET revision=excluded.revision",params![self.account,revision])?;
        tx.execute("INSERT INTO catalog_writes(account,id,revision,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(account,id) DO UPDATE SET revision=excluded.revision,payload=excluded.payload",params![self.account,id,revision,serde_json::to_string(&desired).map_err(failed)?])?;
        tx.commit()?;
        Ok(())
    }
    pub fn pending_catalog_writes(&self) -> Result<Vec<DocumentEntry>> {
        self.cloud()?;
        let inner = self.inner.lock().unwrap();
        let payloads = inner
            .db
            .prepare("SELECT payload FROM catalog_writes WHERE account=?1 ORDER BY revision")?
            .query_map([&self.account], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        payloads.into_iter().map(|payload| serde_json::from_str(&payload).map_err(failed)).collect()
    }
    pub fn ack_catalog_write(&self, document_id: &str, catalog_revision: u64) -> Result<()> {
        self.cloud()?;
        Self::valid(document_id)?;
        let mut inner = self.inner.lock().unwrap();
        let tx = inner.db.transaction()?;
        let removed = tx.execute(
            "DELETE FROM catalog_writes WHERE account=?1 AND id=?2 AND revision=?3",
            params![self.account, document_id, i64::try_from(catalog_revision).map_err(failed)?],
        )?;
        if removed != 0 {
            tx.execute(
                "UPDATE documents SET catalog_seen=1 WHERE account=?1 AND id=?2",
                params![self.account, document_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn account_paths(&self) -> Result<Vec<String>> {
        let inner = self.inner.lock().unwrap();
        let stored = inner
            .db
            .prepare("SELECT DISTINCT path FROM documents WHERE account<>''")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        stored.into_iter().map(|path| Ok(paths::resolve(&self.root, &path)?.to_string_lossy().into_owned())).collect()
    }
    fn discover_recoveries(&self) -> Result<()> {
        if self.account.is_empty() {
            return Ok(());
        }
        let directory = self.state_dir().join("Recovered");
        let paths = match std::fs::read_dir(directory) {
            Ok(paths) => paths,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        for path in paths {
            let path = path?.path();
            if !path.is_file() || path.extension().is_none_or(|extension| extension != "pop") {
                continue;
            }
            let known: bool = self.inner.lock().unwrap().db.query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE path=?1)",
                [paths::store(&self.root, &path)?],
                |row| row.get(0),
            )?;
            if !known {
                self.index_recovery(&path, "Recovered edits")?;
            }
        }
        Ok(())
    }
    fn index_recovery(&self, path: &Path, title: &str) -> Result<Entry> {
        let info = local::inspect(path).map_err(failed)?;
        let store = Store::open(path, Mode::Snapshot)?;
        if store.share()?.is_some() {
            return Err(failed("Recovery still belongs to a cloud document"));
        }
        let template = store.app().app.metadata().slug.clone();
        store.close()?;
        let inner = self.inner.lock().unwrap();
        inner.db.execute(
            "INSERT OR IGNORE INTO documents(account,id,title,template,path,version) VALUES(?1,?2,?3,?4,?5,?6)",
            params![self.account, info.id, title, template, paths::store(&self.root, path)?, info.version],
        )?;
        self.row(&inner, &info.id)
    }
    fn account_recovery(&self, id: &str, title: &str) -> Result<Entry> {
        let info = self.with_owner(id, |owner| {
            let state = local::request(owner, Request::State).map_err(failed)?;
            let pop_runtime::owner::Reply::State { reading, .. } = state else {
                return Err(failed("Missing owner state"));
            };
            Ok(reading.version)
        })?;
        let directory = self.state_dir().join("Recovered");
        std::fs::create_dir_all(&directory)?;
        let digest = hex(&Sha256::digest(format!("{id}:{info}").as_bytes()));
        let path = directory.join(format!("{digest}.pop"));
        if !path.exists() {
            self.with_owner(id, |owner| {
                local::request(owner, Request::Copy { destination: path.clone(), preview: None, icon: None })
                    .map_err(failed)?;
                Ok(())
            })?;
        }
        self.index_recovery(&path, title)
    }
    pub fn local_copy(&self, id: &str, title: &str) -> Result<Entry> {
        let path = self.root.join("documents").join(format!("copy-{}.pop", random_hex(16)));
        self.with_owner(id, |owner| {
            local::request(owner, Request::Copy { destination: path.clone(), preview: None, icon: None })
                .map_err(failed)?;
            Ok(())
        })?;
        self.adopt(&path, title)
    }
    pub fn rename(&self, id: &str, title: &str) -> Result<()> {
        self.check()?;
        let (entry, token) = {
            let inner = self.inner.lock().unwrap();
            (self.row(&inner, id)?, inner.token.clone())
        };
        if entry.role == Some(DocumentRole::Owner) {
            pop_sharing_client::update_document(
                id,
                title,
                &entry.template,
                token.as_deref().ok_or_else(|| failed("Sign in first"))?,
            )
            .map_err(failed)?;
        }
        let inner = self.inner.lock().unwrap();
        inner.db.execute(
            "UPDATE documents SET title=?3 WHERE account=?1 AND id=?2",
            params![if entry.account_bound { &self.account } else { "" }, id, title.trim()],
        )?;
        drop(inner);
        if !entry.local_only {
            self.queue_catalog(id, false)?;
        }
        Ok(())
    }
    pub fn publish_artwork(
        &self,
        owner: &Arc<Owner>,
        version: &str,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    ) -> Result<bool> {
        let Some(id) = owner.app().document_uuid.as_deref() else {
            return Ok(false);
        };
        {
            let inner = self.inner.lock().unwrap();
            if self.row(&inner, id).is_err() {
                return Ok(false);
            }
        }
        local::request(owner, Request::SetArtwork { source_version: version.into(), preview, icon }).map_err(failed)?;
        self.cache_file(id, Some(version))?;
        Ok(true)
    }
    pub fn detach(&self) -> Result<()> {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.active.store(false, Ordering::Release);
        let (clients, owners) = {
            let mut inner = self.inner.lock().unwrap();
            inner.token = None;
            (
                std::mem::take(&mut inner.clients),
                inner
                    .live
                    .values()
                    .filter_map(Weak::upgrade)
                    .chain(inner.retained.values().cloned())
                    .collect::<Vec<_>>(),
            )
        };
        for client in clients.into_values() {
            client.pause();
        }
        let mut failure = None;
        for owner in owners {
            if let Err(error) = local::request_typed(&owner, Request::Flush)
                && error.kind != FailureKind::Closed
            {
                failure.get_or_insert(error);
            }
        }
        if let Some(error) = failure {
            self.active.store(true, Ordering::Release);
            return Err(error.into());
        }
        Ok(())
    }
}
fn file_stamp(path: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    let mut stamp = format!(
        "{}:{}:{}:{}:{}",
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec()
    );
    let wal = PathBuf::from(format!("{}-wal", path.to_string_lossy()));
    if let Ok(metadata) = std::fs::metadata(wal) {
        stamp.push_str(&format!(":{}:{}:{}", metadata.len(), metadata.mtime(), metadata.mtime_nsec()));
    }
    Ok(stamp)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn random_hex(bytes: usize) -> String {
    let mut value = vec![0; bytes];
    getrandom::getrandom(&mut value).expect("OS random source");
    hex(&value)
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn pending_metadata_survives_restart_and_an_old_ack_cannot_erase_a_new_desired_value() {
        let directory = tempfile::tempdir().unwrap();
        let library = Library::open(directory.path(), "account-a").unwrap();
        let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let mut entry = DocumentEntry {
            document_id: id.into(),
            title: "First".into(),
            template_id: "fixture".into(),
            app_revision: None,
            preview_ref: None,
            modified_at: 0,
            role: DocumentRole::Owner,
            access_generation: 1,
            catalog_revision: 0,
            deleted: false,
        };
        library.receive_catalog(&[entry.clone()]).unwrap();
        library.queue_catalog(id, false).unwrap();
        let first = library.pending_catalog_writes().unwrap().pop().unwrap();
        entry.title = "Latest".into();
        library.receive_catalog(&[entry]).unwrap();
        library.queue_catalog(id, false).unwrap();
        drop(library);
        let library = Library::open(directory.path(), "account-a").unwrap();
        library.ack_catalog_write(id, first.catalog_revision).unwrap();
        let writes = library.pending_catalog_writes().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].title, "Latest");
        assert!(writes[0].catalog_revision > first.catalog_revision);
        assert!(Library::open(directory.path(), "account-b").unwrap().pending_catalog_writes().unwrap().is_empty());
        library.ack_catalog_write(id, writes[0].catalog_revision).unwrap();
        assert!(library.pending_catalog_writes().unwrap().is_empty());
    }
    #[test]
    fn sharing_cache_is_authoritative_optional_durable_and_account_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let first = Library::open(directory.path(), "account-a").unwrap();
        let second = Library::open(directory.path(), "account-b").unwrap();
        let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let reference = DocumentEntry {
            document_id: id.into(),
            title: "Shared".into(),
            template_id: "fixture".into(),
            app_revision: None,
            preview_ref: None,
            modified_at: 0,
            role: DocumentRole::Owner,
            access_generation: 1,
            catalog_revision: 0,
            deleted: false,
        };
        first.receive_catalog(std::slice::from_ref(&reference)).unwrap();
        second.receive_catalog(&[reference]).unwrap();
        assert_eq!(first.entries().unwrap()[0].sharing_enabled, None, "owner role does not imply private or shared");
        let mut access = AccessState {
            document: id.into(),
            sharing: true,
            generation: 1,
            capability: None,
            role: DocumentRole::Owner,
        };
        first.remember_access(id, &access).unwrap();
        assert_eq!(first.entries().unwrap()[0].sharing_enabled, Some(true));
        assert_eq!(second.entries().unwrap()[0].sharing_enabled, None);
        assert!(first.access(id, "").is_err(), "failed credential refresh must preserve cached knowledge");
        assert_eq!(first.entries().unwrap()[0].sharing_enabled, Some(true));
        drop(first);
        let first = Library::open(directory.path(), "account-a").unwrap();
        assert_eq!(first.entries().unwrap()[0].sharing_enabled, Some(true));
        access.sharing = false;
        first.remember_access(id, &access).unwrap();
        assert_eq!(first.entries().unwrap()[0].sharing_enabled, Some(false));
        access.document = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into();
        assert!(first.remember_access(id, &access).is_err());
        assert_eq!(first.entries().unwrap()[0].sharing_enabled, Some(false));
    }
}
