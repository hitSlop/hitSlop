//! Native sharing façade: no network frames or Loro bytes cross this boundary.
use crate::owner::NativeOwner;
use pop_sharing_client::{AccountAccess, ActivatedRoom, Client, ClientStatus, PauseKind, Phase};
use std::{path::Path, sync::Arc};

/// A creator-authorized, accepted seed plan that needs no local document owner.
/// Seed/history bytes stay inside Rust; this object is safe to pass between native tasks.
#[derive(uniffi::Object)]
pub struct NativeResolvedRoom(pop_sharing_client::ResolvedRoom);
#[uniffi::export]
impl NativeResolvedRoom {
    pub fn document(&self) -> String {
        self.0.document().into()
    }
    pub fn room(&self) -> String {
        self.0.room().into()
    }
    pub fn seed_hash(&self) -> String {
        self.0.seed_hash().into()
    }
    pub fn validate_authority(&self, authority: pop_library::CatalogAuthority) -> Result<(), SharingError> {
        self.0.validate_authority(&authority).map_err(failure)
    }
    pub fn credential(&self) -> String {
        self.0.credential().into()
    }
    pub fn handoff(&self) -> Result<Option<String>, SharingError> {
        self.0.handoff().map_err(failure)
    }
    pub fn install(&self, path: String, state_directory: String) -> Result<(), SharingError> {
        self.0.install(Path::new(&path), Path::new(&state_directory)).map_err(failure)
    }
}
#[uniffi::export]
pub fn sharing_resolve(document: String, token: String) -> Result<Option<Arc<NativeResolvedRoom>>, SharingError> {
    pop_sharing_client::resolve(&document, &token)
        .map(|resolved| resolved.map(|room| Arc::new(NativeResolvedRoom(room))))
        .map_err(failure)
}

#[uniffi::remote(Record)]
pub struct ActivatedRoom {
    pub room: String,
    pub transition: String,
    pub seed_hash: String,
    pub credential: String,
    pub recovery_path: Option<String>,
}
#[derive(uniffi::Object)]
pub struct NativePreparedRoom(pop_sharing_client::PreparedRoom);
#[uniffi::export]
impl NativePreparedRoom {
    pub fn document(&self) -> String {
        self.0.document().into()
    }
    pub fn room(&self) -> Result<String, SharingError> {
        self.0.room().map_err(failure)
    }
    pub fn phase(&self) -> Phase {
        self.0.phase()
    }
    pub fn activate(
        &self,
        owner: Arc<NativeOwner>,
        token: String,
        authority: Option<pop_library::CatalogAuthority>,
        state_directory: String,
    ) -> Result<pop_sharing_client::ActivatedRoom, SharingError> {
        self.0.activate(&owner.0, &token, authority.as_ref(), Path::new(&state_directory)).map_err(failure)
    }
}
#[uniffi::export]
pub fn sharing_reserve(document: String, token: String) -> Result<Arc<NativePreparedRoom>, SharingError> {
    pop_sharing_client::reserve(&document, &token).map(|room| Arc::new(NativePreparedRoom(room))).map_err(failure)
}

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
    pop_sharing_client::verify_account(&token).map_err(failure)
}

#[uniffi::export]
pub fn sharing_delete_account(token: String) -> Result<(), SharingError> {
    pop_sharing_client::delete_account(&token).map_err(failure)
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
    pub fn restore(
        owner: Arc<NativeOwner>,
        credential: String,
        state_directory: String,
        identity: Option<String>,
    ) -> Result<Arc<Self>, SharingError> {
        Ok(Arc::new(Self(
            Client::restore(owner.0.clone(), &credential, Path::new(&state_directory), identity.as_deref())
                .map_err(failure)?,
        )))
    }
    pub fn connect(&self) -> Result<(), SharingError> {
        self.0.connect().map_err(failure)
    }
    pub fn pause(&self) {
        self.0.pause();
    }
    pub fn freeze(&self, token: String) -> Result<String, SharingError> {
        self.0.freeze(&token).map_err(failure)
    }
    pub fn finalize(
        &self,
        token: String,
        freeze: String,
        receipt: Option<pop_library::CatalogReceipt>,
    ) -> Result<String, SharingError> {
        self.0.finalize(&token, &freeze, receipt.as_ref()).map_err(failure)
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
pub fn sharing_recover(owner: Arc<NativeOwner>, token: String) -> Result<AccountAccess, SharingError> {
    Client::recover(&owner.0, &token).map_err(failure)
}

#[uniffi::export]
pub fn sharing_document(link: String) -> Result<String, SharingError> {
    pop_sharing_client::invitation_document(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_owner_document(owner: Arc<NativeOwner>) -> Option<String> {
    owner.0.app().document_uuid.clone()
}
#[uniffi::export]
pub fn sharing_local_document(path: String) -> Result<Option<String>, SharingError> {
    pop_sharing_client::local_document(Path::new(&path)).map_err(failure)
}
#[uniffi::export]
pub fn sharing_join_credential(link: String) -> Result<String, SharingError> {
    pop_sharing_client::joined_credential(&link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_accept_invitation(existing: Option<String>, link: String) -> Result<String, SharingError> {
    pop_sharing_client::accept_invitation(existing.as_deref(), &link).map_err(failure)
}
#[uniffi::export]
pub fn sharing_install(path: String, link: String, state_directory: String) -> Result<(), SharingError> {
    pop_sharing_client::install(Path::new(&path), &link, Path::new(&state_directory)).map_err(failure)
}

#[uniffi::export]
pub fn sharing_join(owner: Arc<NativeOwner>, existing: Option<String>, link: String) -> Result<String, SharingError> {
    pop_sharing_client::join(&owner.0, existing.as_deref(), &link).map_err(failure)
}

#[uniffi::export]
pub fn sharing_account_owns(document: String, credential: String, token: String) -> Result<bool, SharingError> {
    pop_sharing_client::account_owns(&document, &credential, &token).map_err(failure)
}

#[uniffi::export]
pub fn sharing_cancel_preparation(
    document: String,
    room: Option<String>,
    token: String,
) -> Result<Option<String>, SharingError> {
    pop_sharing_client::cancel_preparation(&document, room.as_deref(), &token).map_err(failure)
}

#[uniffi::export]
pub fn sharing_finalize_catalog_handoff(
    owner: Arc<NativeOwner>,
    document: String,
    token: String,
    receipt: pop_library::CatalogReceipt,
) -> Result<(), SharingError> {
    pop_sharing_client::finalize_catalog_handoff(&owner.0, &document, &token, &receipt).map_err(failure)
}

#[uniffi::export]
pub fn sharing_recovered_documents(state_directory: String) -> Result<Vec<String>, SharingError> {
    pop_sharing_client::recovered_documents(Path::new(&state_directory)).map_err(failure)
}
