//! Typed device-local index and persistent cloud document composition.
use super::*;
use core::Entry;
use pop_cloud as core;
use pop_sharing_protocol::document::{AccessState, DocumentRole};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LibraryError {
    #[error("{message}")]
    Failed { kind: core::ErrorKind, message: String },
}
impl From<core::Error> for LibraryError {
    fn from(error: core::Error) -> Self {
        Self::Failed { kind: error.kind(), message: error.to_string() }
    }
}
#[uniffi::remote(Enum)]
pub enum DocumentRole {
    Owner,
    Editor,
}
#[uniffi::remote(Record)]
#[uniffi(name = "LibraryEntry")]
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
#[uniffi::export(callback_interface)]
pub trait LibraryListener: Send + Sync {
    fn changed(&self);
}
#[derive(uniffi::Object)]
pub struct NativeLibrary(Arc<core::Library>);
#[uniffi::export]
impl NativeLibrary {
    #[uniffi::constructor]
    pub fn open(root: String, account: String) -> Result<Arc<Self>, LibraryError> {
        Ok(Arc::new(Self(core::Library::open(Path::new(&root), &account)?)))
    }
    pub fn listen(&self, listener: Box<dyn LibraryListener>) {
        self.0.listen(Arc::new(move || listener.changed()));
    }
    pub fn reconcile(&self) -> Result<(), LibraryError> {
        Ok(self.0.reconcile()?)
    }
    pub fn account_paths(&self) -> Result<Vec<String>, LibraryError> {
        Ok(self.0.account_paths()?)
    }
    pub fn entries(&self) -> Result<Vec<core::Entry>, LibraryError> {
        Ok(self.0.entries()?)
    }
    pub fn register_owner(&self, owner: Arc<crate::owner::NativeOwner>) -> Result<(), LibraryError> {
        Ok(self.0.register(&owner.0)?)
    }
    pub fn unregister_owner(&self, id: String) -> Result<(), LibraryError> {
        Ok(self.0.unregister(&id)?)
    }
    pub fn create(&self, template: String, title: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.create(Path::new(&template), &title)?)
    }
    pub fn adopt(&self, source: String, title: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.adopt(Path::new(&source), &title)?)
    }
    pub fn enable_sync(&self, account: String, id: String, token: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.enable_sync(&account, &id, &token)?)
    }
    pub fn download(&self, account: String, id: String, token: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.download(&account, &id, &token)?)
    }
    pub fn set_account_active(&self, active: bool) {
        self.0.set_account_active(active);
    }
    pub fn sync(&self, account: String, token: String) -> Result<(), LibraryError> {
        Ok(self.0.sync(&account, &token)?)
    }
    pub fn access(&self, account: String, id: String, token: String) -> Result<AccessState, LibraryError> {
        Ok(self.0.access(&account, &id, &token)?)
    }
    pub fn sharing(
        &self,
        account: String,
        id: String,
        enabled: bool,
        token: String,
    ) -> Result<AccessState, LibraryError> {
        Ok(self.0.sharing(&account, &id, enabled, &token)?)
    }
    pub fn join(&self, account: String, link: String, token: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.join(&account, &link, &token)?)
    }
    pub fn leave(&self, account: String, id: String, token: String) -> Result<(), LibraryError> {
        Ok(self.0.leave(&account, &id, &token)?)
    }
    pub fn delete_everywhere(&self, account: String, id: String, token: String) -> Result<(), LibraryError> {
        Ok(self.0.delete_everywhere(&account, &id, &token)?)
    }
    pub fn delete_local(&self, id: String) -> Result<(), LibraryError> {
        Ok(self.0.delete_local(&id)?)
    }
    pub fn remove_download(&self, id: String) -> Result<(), LibraryError> {
        Ok(self.0.remove_download(&id)?)
    }
    pub fn local_copy(&self, id: String, title: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.local_copy(&id, &title)?)
    }
    pub fn publish_artwork(
        &self,
        owner: Arc<crate::owner::NativeOwner>,
        source_version: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    ) -> Result<bool, LibraryError> {
        Ok(self.0.publish_artwork(&owner.0, &source_version, preview, icon)?)
    }
    pub fn suspend_account(&self) -> Result<(), LibraryError> {
        Ok(self.0.suspend_account()?)
    }
    pub fn set_account(&self, account: Option<String>) -> Result<(), LibraryError> {
        Ok(self.0.set_account(account)?)
    }
}
