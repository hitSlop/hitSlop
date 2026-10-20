//! Native sharing façade: no network frames or Loro bytes cross this boundary.
use crate::owner::NativeOwner;
use hitslop_sharing_client::{AccountAccess, Client, ClientStatus, Phase};
use std::{path::Path, sync::Arc};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SharingError {
    #[error("{message}")]
    Failed { message: String },
}
fn failure(message: String) -> SharingError {
    SharingError::Failed { message }
}

#[uniffi::export]
pub fn sharing_verify_account(token: String) -> Result<(), SharingError> {
    hitslop_sharing_client::verify_account(&token).map_err(failure)
}

#[uniffi::export]
pub fn sharing_delete_account(token: String) -> Result<(), SharingError> {
    hitslop_sharing_client::delete_account(&token).map_err(failure)
}

#[uniffi::remote(Enum)]
pub enum Phase {
    Preparing,
    Offline,
    Syncing,
    Synced,
    Paused,
    Ended,
}
#[uniffi::remote(Record)]
pub struct ClientStatus {
    pub phase: Phase,
    pub detail: String,
    pub link: Option<String>,
    pub creator: bool,
}
#[uniffi::remote(Record)]
pub struct AccountAccess {
    pub credential: Option<String>,
    pub phase: Option<Phase>,
}
#[uniffi::export(callback_interface)]
pub trait SharingListener: Send + Sync {
    fn changed(&self);
}

#[derive(uniffi::Object)]
pub struct NativeSharing(Client);
#[uniffi::export]
impl NativeSharing {
    #[uniffi::constructor]
    pub fn restore(owner: Arc<NativeOwner>, credential: String) -> Result<Arc<Self>, SharingError> {
        Ok(Arc::new(Self(Client::restore(owner.0.clone(), &credential).map_err(failure)?)))
    }
    pub fn connect(&self) -> Result<(), SharingError> {
        self.0.connect().map_err(failure)
    }
    pub fn pause(&self) {
        self.0.pause();
    }
    pub fn stop(&self, token: String) -> Result<String, SharingError> {
        self.0.stop(&token).map_err(failure)
    }
    pub fn set_creator(&self, creator: bool) {
        self.0.set_creator(creator);
    }
    pub fn listen(&self, listener: Box<dyn SharingListener>) {
        self.0.listen(Arc::new(move || listener.changed()));
    }
    pub fn status(&self) -> ClientStatus {
        self.0.status()
    }
}
#[uniffi::export]
pub fn sharing_create(owner: Arc<NativeOwner>, token: String) -> Result<String, SharingError> {
    Client::create(&owner.0, &token).map_err(failure)
}
#[uniffi::export]
pub fn sharing_recover(owner: Arc<NativeOwner>, token: String) -> Result<AccountAccess, SharingError> {
    Client::recover(&owner.0, &token).map_err(failure)
}
#[uniffi::export]
pub fn sharing_stop_document(owner: Arc<NativeOwner>, token: String) -> Result<(), SharingError> {
    Client::stop_document(&owner.0, &token).map_err(failure)
}
#[uniffi::export]
pub fn sharing_document(link: String) -> Result<String, SharingError> {
    hitslop_sharing_client::invitation_document(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_owner_document(owner: Arc<NativeOwner>) -> Option<String> {
    owner.0.app().document_uuid.clone()
}
#[uniffi::export]
pub fn sharing_local_document(path: String) -> Result<Option<String>, SharingError> {
    hitslop_sharing_client::local_document(Path::new(&path)).map_err(failure)
}
#[uniffi::export]
pub fn sharing_join_credential(link: String) -> Result<String, SharingError> {
    hitslop_sharing_client::joined_credential(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_accept_invitation(existing: Option<String>, link: String) -> Result<String, SharingError> {
    hitslop_sharing_client::accept_invitation(existing.as_deref(), &link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_install(path: String, link: String) -> Result<(), SharingError> {
    hitslop_sharing_client::install(Path::new(&path), &link).map_err(failure)
}

#[uniffi::export]
pub fn sharing_join(owner: Arc<NativeOwner>, existing: Option<String>, link: String) -> Result<String, SharingError> {
    hitslop_sharing_client::join(&owner.0, existing.as_deref(), &link).map_err(failure)
}

#[uniffi::export]
pub fn sharing_account_owns(document: String, credential: String, token: String) -> Result<bool, SharingError> {
    hitslop_sharing_client::account_owns(&document, &credential, &token).map_err(failure)
}
