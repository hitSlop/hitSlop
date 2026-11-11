//! Device-local document index and account-scoped cloud cache. Bytes synchronize
//! through the existing owner and persistent cloud protocol, never through catalog assets.
mod changes;
mod cloud;
mod error;
mod feed;
mod index;
mod local;
mod paths;
mod projection;
mod recovery;
mod session;

use crate::{client::Client, transport::account::Account};
pub use error::{Error, Issue};
use error::{Result, failed};
use index::IndexedDocument;
use pop_contracts::hex;
use pop_runtime::owner::{Failure, FailureKind, Owner};
use pop_sharing_protocol::document::{AccessState, DocumentMetadata, DocumentRole};
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
    feed: Mutex<Option<Arc<Account>>>,
    departing: Mutex<std::collections::HashSet<String>>,
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
        if pop_contracts::identity::valid_document_id(id) { Ok(()) } else { Err(failed("Invalid document identity")) }
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
    fn state_dir(&self) -> PathBuf {
        self.root.join("accounts").join(&self.scope).join("relay-state")
    }
    fn cached_path(&self, id: &str) -> PathBuf {
        self.root.join("accounts").join(&self.scope).join("documents").join(format!("{id}.pop"))
    }
}
