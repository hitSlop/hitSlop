//! Device-local document index and account-scoped cloud cache. Bytes synchronize
//! through the existing owner and persistent cloud protocol, never through catalog assets.
mod changes;
mod feed;
mod index;
use index::IndexedDocument;
mod paths;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;

/// These tables. Any other version is refused, and nothing in it is changed.
const INDEX_VERSION: u32 = 2;
/// The device index, beside the managed `documents` and per-account `accounts` folders.
const INDEX: &str = "index.sqlite";
use pop_runtime::owner::{Failure, FailureKind, Owner, Request};
use pop_sharing_client::{Client, local};
pub use pop_sharing_protocol::document::{AccessState, DocumentRole};
use pop_sharing_protocol::{document::DocumentMetadata, hex};
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
    notice: changes::Notice,
    feed: Mutex<Option<Arc<pop_sharing_client::Account>>>,
    departing: Mutex<std::collections::HashSet<String>>,
    feed_active: AtomicBool,
}
impl Library {
    pub fn open(root: &Path, account: &str) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root)?;
        let root = std::fs::canonicalize(root)?;
        let db = Connection::open(root.join(INDEX))?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > INDEX_VERSION {
            return Err(Error {
                issue: Issue::RequiresUpdate,
                message: "This local index requires a newer pop; existing files are retained".into(),
            });
        }
        if version != 0 && version != INDEX_VERSION {
            return Err(failed("This local index was made by an older version of pop and can't be opened"));
        }
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS documents(account TEXT NOT NULL,id TEXT NOT NULL,title TEXT NOT NULL,template TEXT NOT NULL,path TEXT NOT NULL,version TEXT NOT NULL DEFAULT '',role TEXT,revision INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0,problem TEXT,pending_enable INTEGER NOT NULL DEFAULT 0,artwork_version TEXT NOT NULL DEFAULT '',artwork_digest TEXT NOT NULL DEFAULT '',file_stamp TEXT NOT NULL DEFAULT '',listed INTEGER NOT NULL DEFAULT 0,sharing_enabled INTEGER,opened INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(account,id));
            ",
        )?;
        db.pragma_update(None, "user_version", INDEX_VERSION)?;
        let scope = crate::hex(&Sha256::digest(account.as_bytes()));
        std::fs::create_dir_all(root.join("documents"))?;
        std::fs::create_dir_all(root.join("accounts").join(&scope).join("documents"))?;
        let library = Arc::new(Self {
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
            notice: Arc::new(Mutex::new(None)),
            feed: Mutex::new(None),
            departing: Mutex::new(std::collections::HashSet::new()),
            feed_active: AtomicBool::new(true),
        });
        library.reconcile()?;
        Ok(library)
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
        self.project(IndexedDocument::one(&inner.db, &self.account, id)?, inner.clients.get(id).map(Arc::as_ref))
    }
    fn project(&self, row: IndexedDocument, client: Option<&Client>) -> Result<Entry> {
        let id = row.id.as_str();
        let resolved_path = paths::resolve(&self.root, &row.path)?;
        let downloaded = resolved_path.is_file();
        let role = match row.role.as_deref() {
            Some("owner") => Some(DocumentRole::Owner),
            Some("editor") => Some(DocumentRole::Editor),
            _ => None,
        };
        let mut version = row.version;
        let stamp = if downloaded { file_stamp(&resolved_path).ok() } else { None };
        let (confirmed, receipt_problem) = if role.is_some() {
            match pop_sharing_client::confirmed_saved_version(&self.state_dir(), id) {
                Ok(version) => (version, None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };
        let client_phase = client.map(|client| client.status().phase);
        let client_synced = client_phase == Some(pop_sharing_client::Phase::Synced);
        if client_synced && let Some(confirmed) = &confirmed {
            version = confirmed.clone();
        }
        let dirty = role.is_some()
            && (row.pending
                || downloaded
                    && (confirmed.as_deref() != Some(&version)
                        || (!client_synced && stamp.as_deref() != Some(&row.file_stamp))
                        || client_phase.is_some_and(|phase| phase != pop_sharing_client::Phase::Synced)));
        let asset_revision = local::asset_revision(&version, &row.artwork_version, &row.artwork_digest);
        let artwork_version = if row.artwork_version == version { row.artwork_version } else { String::new() };
        Ok(Entry {
            id: id.into(),
            title: row.title,
            template: row.template,
            path: resolved_path.to_string_lossy().into_owned(),
            version,
            asset_revision,
            artwork_version,
            dirty,
            deleted: row.deleted,
            local_only: role.is_none(),
            account_bound: !row.account.is_empty(),
            downloaded,
            sharing_enabled: row.sharing_enabled,
            inbox: !row.opened && !row.account.is_empty() && role.is_some() && !row.deleted,
            problem: row.problem.or(receipt_problem),
            role,
            revision: u64::try_from(row.revision).map_err(failed)?,
        })
    }
    pub fn entries(&self) -> Result<Vec<Entry>> {
        self.check()?;
        let (rows, clients) = {
            let inner = self.inner.lock().unwrap();
            (IndexedDocument::all(&inner.db, &self.account)?, inner.clients.clone())
        };
        rows.into_iter()
            .map(|row| {
                let client = clients.get(&row.id).map(Arc::as_ref);
                self.project(row, client)
            })
            .collect()
    }

    pub fn create(&self, template: &Path, title: &str) -> Result<Entry> {
        let _change = self.changing();
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
        let _change = self.changing();
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
    pub fn register(&self, owner: &Arc<Owner>) -> Result<()> {
        let _change = self.changing();
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
            if let Some(client) = self.connection(id).filter(|client| client.is_running()) {
                client.set_identity(&token).map_err(failed)?;
            } else {
                self.attach(id, owner.clone(), &token)?;
            }
        }
        Ok(())
    }
    pub fn unregister(&self, id: &str) -> Result<()> {
        let _change = self.changing();
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
        let notice = Arc::downgrade(&self.notice);
        client.observe(Arc::new(move || {
            if let Some(notice) = notice.upgrade() {
                changes::notify(&notice);
            }
        }));
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
    pub fn sync(self: &Arc<Self>, token: &str) -> Result<()> {
        let _change = self.changing();
        let generation = self.operation()?;
        self.reconcile()?;
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
            if entry.local_only || entry.deleted {
                continue;
            }
            self.current_operation(generation)?;
            let existing = self.connection(&id).filter(|client| client.is_running());
            let refreshed = existing.map_or_else(
                || self.attach(&id, owner, token).map(|_| ()),
                |client| client.set_identity(token).map_err(failed),
            );
            self.current_operation(generation)?;
            if let Err(error) = refreshed {
                failure = Some(error);
            }
        }
        for id in pending {
            if let Err(error) = self.enable_sync(&id, token) {
                failure = Some(error);
            }
        }
        self.connect_feed(token)?;
        let feed = self.feed.lock().unwrap().clone();
        if let Some(feed) = feed
            && !feed.wait_snapshot(std::time::Duration::from_secs(20))
        {
            failure.get_or_insert_with(|| {
                failed(
                    feed.last_error()
                        .unwrap_or_else(|| "The cloud library is unavailable. Local files are retained.".into()),
                )
            });
        }
        failure.map_or(Ok(()), Err)
    }
    pub fn connection(&self, id: &str) -> Option<Arc<Client>> {
        self.inner.lock().unwrap().clients.get(id).cloned()
    }
    pub fn enable_sync(&self, id: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
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
        if let Some(owner) = live {
            self.attach(id, owner, token)?;
        }
        let inner = self.inner.lock().unwrap();
        self.row(&inner, id)
    }
    pub fn download(&self, id: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
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
            self.apply(pop_sharing_client::LibraryUpdate::Remove(id.into()))?;
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
        resolved
            .install_if_current(Path::new(&entry.path), &self.state_dir(), || {
                self.current_operation(generation).map_err(|error| error.message)
            })
            .map_err(failed)?;
        self.current_operation(generation)?;
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
        let _change = self.changing();
        let generation = self.operation()?;
        let access = pop_sharing_client::access(id, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access)?;
        Ok(access)
    }
    pub fn sharing(&self, id: &str, enabled: bool, token: &str) -> Result<AccessState> {
        let _change = self.changing();
        let generation = self.operation()?;
        let access = pop_sharing_client::set_sharing(id, enabled, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access)?;
        Ok(access)
    }
    pub fn join(&self, link: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
        let generation = self.operation()?;
        let access = pop_sharing_client::join_member(link, token).map_err(failed)?;
        let metadata = pop_sharing_client::document_metadata(&access.document, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_join(&access, &metadata)?;
        self.download(&access.document, token)
    }
    /// The document decides the role: an owner opening their own invitation stays the owner,
    /// so catalog deletion later deletes rather than leaves.
    fn remember_join(&self, access: &AccessState, metadata: &DocumentMetadata) -> Result<()> {
        let id = &access.document;
        let path = self.cached_path(id);
        {
            let inner = self.inner.lock().unwrap();
            inner.db.execute("INSERT INTO documents(account,id,title,template,path,role,opened,sharing_enabled) VALUES(?1,?2,?3,?4,?5,?6,coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0),?7) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role=excluded.role,deleted=0,sharing_enabled=excluded.sharing_enabled",params![self.account,id,metadata.title,metadata.template_id,paths::store(&self.root,&path)?,stored_role(access.role),access.sharing])?;
        }
        Ok(())
    }
    pub fn leave(&self, id: &str, token: &str) -> Result<()> {
        self.remove_cloud(id, token, false)
    }
    pub fn delete_everywhere(&self, id: &str, token: &str) -> Result<()> {
        self.remove_cloud(id, token, true)
    }
    fn remove_cloud(&self, id: &str, token: &str, delete: bool) -> Result<()> {
        let _change = self.changing();
        let generation = self.operation()?;
        self.unregister(id)?;
        if !delete && self.row(&self.inner.lock().unwrap(), id)?.downloaded {
            self.local_copy(id, "Retained local copy")?;
        }
        self.departing.lock().unwrap().insert(id.into());
        let result = (|| {
            self.inner.lock().unwrap().db.execute(
                "UPDATE documents SET deleted=1,listed=1 WHERE account=?1 AND id=?2",
                params![self.account, id],
            )?;
            let result = if delete {
                pop_sharing_client::delete_document(id, token)
            } else {
                pop_sharing_client::leave(id, token)
            };
            self.current_operation(generation)?;
            if let Err(error) = result {
                self.inner
                    .lock()
                    .unwrap()
                    .db
                    .execute("UPDATE documents SET deleted=0 WHERE account=?1 AND id=?2", params![self.account, id])?;
                return Err(failed(error));
            }
            Ok(())
        })();
        self.departing.lock().unwrap().remove(id);
        // Discard the connection's queued frames; its next complete snapshot is authoritative.
        if let Some(feed) = self.feed.lock().unwrap().as_ref() {
            feed.wake();
        }
        result
    }
    pub fn set_account_active(&self, active: bool) {
        self.feed_active.store(active, Ordering::Release);
        if !active && let Some(feed) = self.feed.lock().unwrap().take() {
            feed.stop();
        }
    }
    pub fn remove_download(&self, id: &str) -> Result<()> {
        let _change = self.changing();
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
        let _change = self.changing();
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
    pub fn account_paths(&self) -> Result<Vec<String>> {
        let inner = self.inner.lock().unwrap();
        let stored = inner
            .db
            .prepare("SELECT DISTINCT path FROM documents WHERE account<>''")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        stored.into_iter().map(|path| Ok(paths::resolve(&self.root, &path)?.to_string_lossy().into_owned())).collect()
    }
    /// Index completed recovery copies at explicit lifecycle/refresh boundaries.
    /// Snapshot reads do not mutate the index or create files.
    pub fn reconcile(&self) -> Result<()> {
        self.check()?;
        let before = self.inner.lock().unwrap().db.total_changes();
        let result = self.discover_recoveries();
        let changed = self.inner.lock().unwrap().db.total_changes() != before;
        if changed {
            self.notify();
        }
        result
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
        let _change = self.changing();
        let path = self.root.join("documents").join(format!("copy-{}.pop", random_hex(16)));
        self.with_owner(id, |owner| {
            local::request(owner, Request::Copy { destination: path.clone(), preview: None, icon: None })
                .map_err(failed)?;
            Ok(())
        })?;
        self.adopt(&path, title)
    }
    pub fn publish_artwork(
        &self,
        owner: &Arc<Owner>,
        version: &str,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    ) -> Result<bool> {
        let _change = self.changing();
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
        let _change = self.changing();
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.active.store(false, Ordering::Release);
        if let Some(feed) = self.feed.lock().unwrap().take() {
            feed.stop();
        }
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
/// The index's `role` column, read back by `Library::project`.
fn stored_role(role: DocumentRole) -> &'static str {
    match role {
        DocumentRole::Owner => "owner",
        DocumentRole::Editor => "editor",
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
fn random_hex(bytes: usize) -> String {
    let mut value = vec![0; bytes];
    getrandom::getrandom(&mut value).expect("OS random source");
    hex(&value)
}
