//! Native cloud transport and explicit invitation capability boundaries.
use super::*;
use pop_sharing_client::{Client, ClientStatus, PauseKind, Phase};
use pop_sharing_protocol::document::AccessState;
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
#[uniffi::remote(Enum)]
pub enum PauseKind {
    RoomFull,
    Outgrown,
    NeedsUpdate,
    Refused,
    MissingHistory,
    FilesFull,
    Ended,
}
#[uniffi::remote(Record)]
pub struct ClientStatus {
    pub pause_kind: Option<PauseKind>,
    pub phase: Phase,
    pub detail: String,
    pub link: Option<String>,
    pub creator: bool,
    pub recovery_path: Option<String>,
}
#[uniffi::remote(Record)]
pub struct AccessState {
    pub document: String,
    pub sharing: bool,
    pub generation: u64,
    pub capability: Option<String>,
    pub role: pop_sharing_protocol::document::DocumentRole,
}
#[uniffi::export(callback_interface)]
pub trait SharingListener: Send + Sync {
    fn changed(&self);
}
#[derive(uniffi::Object)]
pub struct NativeSharing(pub(crate) Arc<Client>);
#[uniffi::export]
impl NativeSharing {
    #[uniffi::constructor]
    pub fn restore(
        owner: Arc<crate::owner::NativeOwner>,
        credential: String,
        state_directory: String,
    ) -> Result<Arc<Self>, SharingError> {
        Ok(Arc::new(Self(Arc::new(
            Client::restore(owner.0.clone(), &credential, Path::new(&state_directory)).map_err(failure)?,
        ))))
    }
    #[uniffi::constructor]
    pub fn cloud(
        owner: Arc<crate::owner::NativeOwner>,
        document: String,
        token: String,
        state_directory: String,
    ) -> Result<Arc<Self>, SharingError> {
        Ok(Arc::new(Self(Arc::new(
            Client::cloud(owner.0.clone(), &document, &token, Path::new(&state_directory)).map_err(failure)?,
        ))))
    }
    pub fn connect(&self) -> Result<(), SharingError> {
        self.0.connect().map_err(failure)
    }
    pub fn pause(&self) {
        self.0.pause();
    }
    pub fn status(&self) -> ClientStatus {
        self.0.status()
    }
    pub fn set_creator(&self, creator: bool) {
        self.0.set_creator(creator);
    }
    pub fn listen(&self, listener: Box<dyn SharingListener>) {
        self.0.listen(Arc::new(move || listener.changed()));
    }
}
#[uniffi::export]
pub fn sharing_link(access: pop_sharing_protocol::document::AccessState) -> Result<Option<String>, SharingError> {
    pop_sharing_client::sharing_link(&access).map_err(failure)
}
#[uniffi::export]
pub fn sharing_join_credential(link: String) -> Result<String, SharingError> {
    pop_sharing_client::joined_credential(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_credential_document(credential: String) -> Option<String> {
    pop_sharing_client::credential_document(&credential)
}
#[uniffi::export]
pub fn sharing_document(link: String) -> Result<String, SharingError> {
    pop_sharing_client::invitation_document(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_local_document(path: String) -> Result<Option<String>, SharingError> {
    pop_sharing_client::local_document(Path::new(&path)).map_err(failure)
}
#[uniffi::export]
pub fn sharing_install(path: String, link: String, state_directory: String) -> Result<(), SharingError> {
    pop_sharing_client::install(Path::new(&path), &link, Path::new(&state_directory)).map_err(failure)
}
#[uniffi::export]
pub fn sharing_join(
    owner: Arc<crate::owner::NativeOwner>,
    existing: Option<String>,
    link: String,
) -> Result<String, SharingError> {
    let _ = existing;
    let document = pop_sharing_client::invitation_document(&link).map_err(failure)?;
    if owner.0.app().document_uuid.as_deref() != Some(&document) {
        return Err(failure("Different invitation document".into()));
    }
    pop_sharing_client::joined_credential(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_recovered_documents(state_directory: String) -> Result<Vec<String>, SharingError> {
    pop_sharing_client::recovered_documents(Path::new(&state_directory)).map_err(failure)
}
#[uniffi::export]
pub fn sharing_verify_account(token: String) -> Result<(), SharingError> {
    pop_sharing_client::verify_account(&token).map_err(failure)
}
#[uniffi::export]
pub fn sharing_delete_account(token: String) -> Result<(), SharingError> {
    pop_sharing_client::delete_account(&token).map_err(failure)
}
#[uniffi::export]
pub fn sharing_account_owns(document: String, credential: String, token: String) -> Result<bool, SharingError> {
    pop_sharing_client::account_owns(&document, &credential, &token).map_err(failure)
}

#[uniffi::export]
pub fn sharing_owner_document(owner: Arc<crate::owner::NativeOwner>) -> Option<String> {
    owner.0.app().document_uuid.clone()
}
