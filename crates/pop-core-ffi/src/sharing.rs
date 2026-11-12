//! Cloud connection status and explicit invitation capability boundaries; the library owns
//! native connections.
use pop_cloud::ErrorKind;
use pop_contracts::relay::PauseKind;
use pop_contracts::relay::SyncPhase;
use pop_sharing_protocol::document::AccessState;
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SharingError {
    #[error("{message}")]
    Failed { kind: pop_cloud::ErrorKind, message: String },
}
pub(crate) fn failure(error: pop_cloud::Error) -> SharingError {
    SharingError::Failed { kind: error.kind(), message: error.to_string() }
}
#[uniffi::remote(Enum)]
pub enum ErrorKind {
    Pause { reason: PauseKind },
    UnknownOutcome,
    Busy,
    RequiresUpdate,
    AppMismatch,
    AccountChanged,
    SignInRequired,
    Failed,
}
#[uniffi::remote(Enum)]
pub enum SyncPhase {
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
    Conflict,
    SignIn,
    Failed,
    Busy,
}
#[uniffi::remote(Record)]
pub struct AccessState {
    pub document: String,
    pub sharing: bool,
    pub capability: Option<String>,
    pub role: pop_sharing_protocol::document::DocumentRole,
}
#[uniffi::export]
pub fn sharing_link(access: pop_sharing_protocol::document::AccessState) -> Result<Option<String>, SharingError> {
    pop_cloud::sharing_link(&access).map_err(failure)
}
#[uniffi::export]
pub fn sharing_document(link: String) -> Result<String, SharingError> {
    pop_cloud::invitation_document(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_delete_account(token: String) -> Result<(), SharingError> {
    pop_cloud::delete_account(&token).map_err(failure)
}
