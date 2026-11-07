//! Cloud connection status and explicit invitation capability boundaries; the library owns
//! native connections.
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
/// A library-owned document connection, observed by the host (`NativeLibrary::connection`).
#[derive(uniffi::Object)]
pub struct NativeSharing(pub(crate) Arc<Client>);
#[uniffi::export]
impl NativeSharing {
    pub fn status(&self) -> ClientStatus {
        self.0.status()
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
pub fn sharing_document(link: String) -> Result<String, SharingError> {
    pop_sharing_client::invitation_document(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_local_document(path: String) -> Result<Option<String>, SharingError> {
    pop_sharing_client::local_document(Path::new(&path)).map_err(failure)
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
pub fn sharing_owner_document(owner: Arc<crate::owner::NativeOwner>) -> Option<String> {
    owner.0.app().document_uuid.clone()
}
