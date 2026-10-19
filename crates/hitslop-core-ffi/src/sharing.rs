//! Native sharing façade: no network frames or Loro bytes cross this boundary.
use crate::owner::NativeOwner;
use hitslop_sharing_client::{Client, ClientStatus, Phase};
use std::{path::Path, sync::Arc};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SharingError {
    #[error("{message}")]
    Failed { message: String },
}
fn failure(message: String) -> SharingError {
    SharingError::Failed { message }
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

#[derive(uniffi::Object)]
pub struct NativeSharing(Client);
#[uniffi::export]
impl NativeSharing {
    #[uniffi::constructor]
    pub fn restore(owner: Arc<NativeOwner>, credential: String) -> Result<Arc<Self>, SharingError> {
        Ok(Arc::new(Self(Client::restore(owner.0.clone(), &credential).map_err(failure)?)))
    }
    pub fn publish(&self, create_key: String) -> Result<String, SharingError> {
        self.0.publish(&create_key).map_err(failure)
    }
    pub fn connect(&self) -> Result<(), SharingError> {
        self.0.connect().map_err(failure)
    }
    pub fn pause(&self) {
        self.0.pause();
    }
    pub fn stop(&self) -> Result<String, SharingError> {
        self.0.stop().map_err(failure)
    }
    pub fn status(&self) -> ClientStatus {
        self.0.status()
    }
}
#[uniffi::export]
pub fn sharing_prepare(owner: Arc<NativeOwner>, root: String) -> Result<String, SharingError> {
    Client::prepare(&owner.0, Path::new(&root)).map_err(failure)
}
#[uniffi::export]
pub fn sharing_stage(owner: Arc<NativeOwner>, credential: String) -> Result<String, SharingError> {
    Client::stage(&owner.0, &credential).map_err(failure)
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
