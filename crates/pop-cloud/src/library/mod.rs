//! Device-local document index and account-scoped cloud cache. Bytes synchronize
//! through the existing owner and persistent cloud protocol, never through catalog assets.
pub(crate) mod changes;
mod cloud;
mod error;
mod feed;
pub(crate) mod index;
mod local;
mod paths;
mod projection;
pub(crate) mod receipts;
mod recovery;
mod session;

use crate::error::Result;
use crate::{Error, Issue};
use crate::{client::Client, transport::account::Account};
use error::failed;
use index::IndexedDocument;
use pop_contracts::hex;
use pop_runtime::owner::{FailureKind, Owner};
use pop_sharing_protocol::document::{AccessState, DocumentRole, JoinedDocument};
use pop_storage::file;
use pop_storage::store::{Mode, Store};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

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
    pub phase: pop_contracts::relay::SyncPhase,
    pub pause_kind: Option<pop_contracts::relay::PauseKind>,
    pub detail: String,
}
struct Inner {
    db: Connection,
    live: HashMap<String, Weak<Owner>>,
    clients: HashMap<String, Arc<Client>>,
    retained: HashMap<String, Arc<Owner>>,
    token: Option<String>,
    syncing: bool,
    switching: bool,
}
pub struct Library {
    root: PathBuf,
    account: Mutex<String>,
    weak: Weak<Self>,
    inner: Mutex<Inner>,
    active: AtomicBool,
    generation: AtomicU64,
    notice: changes::Notice,
    feed: Mutex<Option<Arc<Account>>>,
    departing: Mutex<std::collections::HashSet<(u64, String)>>,
    feed_active: AtomicBool,
}
impl Library {
    pub fn open(root: &Path, account: &str) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root)?;
        let root = std::fs::canonicalize(root)?;
        let db = index::open(&root)?;
        let scope = hex(&Sha256::digest(account.as_bytes()));
        std::fs::create_dir_all(root.join("documents"))?;
        std::fs::create_dir_all(root.join("accounts").join(&scope).join("documents"))?;
        let library = Arc::new_cyclic(|weak| Self {
            root,
            account: Mutex::new(account.into()),
            weak: weak.clone(),
            inner: Mutex::new(Inner {
                db,
                live: HashMap::new(),
                clients: HashMap::new(),
                retained: HashMap::new(),
                token: None,
                syncing: false,
                switching: false,
            }),
            active: AtomicBool::new(true),
            generation: AtomicU64::new(1),
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
            Err(Error::Library { issue: Issue::AccountChanged, message: "The cloud account changed".into() })
        } else {
            Ok(())
        }
    }
    fn cloud(&self) -> Result<()> {
        self.check()?;
        if self.account().is_empty() {
            Err(Error::Library { issue: Issue::AccountChanged, message: "Sign in to enable cloud sync".into() })
        } else {
            Ok(())
        }
    }
    fn valid(id: &str) -> Result<()> {
        if pop_contracts::identity::valid_document_id(id) { Ok(()) } else { Err(failed("Invalid document identity")) }
    }
    fn operation(&self) -> Result<u64> {
        self.cloud()?;
        Ok(self.generation.load(Ordering::Acquire))
    }
    fn operation_for(&self, account: &str) -> Result<u64> {
        let inner = pop_core::lock(&self.inner);
        let generation = self.operation()?;
        if self.account() != account {
            return Err(Error::Library { issue: Issue::AccountChanged, message: "The cloud account changed".into() });
        }
        drop(inner);
        Ok(generation)
    }
    fn current_operation(&self, generation: u64) -> Result<()> {
        self.check()?;
        if generation != self.generation.load(Ordering::Acquire) {
            return Err(Error::Library { issue: Issue::AccountChanged, message: "The cloud account changed".into() });
        }
        Ok(())
    }
    fn account(&self) -> String {
        pop_core::lock(&self.account).clone()
    }
    fn scope(&self) -> String {
        hex(&Sha256::digest(self.account().as_bytes()))
    }
    fn receipt(&self, id: &str, generation: u64) -> Result<receipts::Receipt> {
        self.current_operation(generation)?;
        let account = self.account();
        let id = id.to_owned();
        let weak = self.weak.clone();
        Ok(Arc::new(move |version| {
            let library = weak.upgrade().ok_or_else(|| failed("Library closed"))?;
            {
                let mut inner = pop_core::lock(&library.inner);
                library.current_operation(generation)?;
                index::confirm(&mut inner.db, &account, &id, &version)?;
            }
            library.notify();
            Ok(())
        }))
    }
    fn state_dir(&self) -> PathBuf {
        self.root.join("accounts").join(self.scope()).join("relay-state")
    }
    fn cached_path(&self, id: &str) -> PathBuf {
        self.root.join("accounts").join(self.scope()).join("documents").join(format!("{id}.pop"))
    }
}
