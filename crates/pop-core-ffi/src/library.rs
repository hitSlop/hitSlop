//! Paths, typed library metadata and opaque CloudKit state; never Loro payloads.
use super::*;
use core::{
    CatalogAuthority, CatalogCheckpointProof, CatalogReceipt, CatalogRelayBinding, CatalogStamp, Entry, Issue, Retry,
    Upload,
};
use pop_library as core;
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LibraryError {
    #[error("{message}")]
    Failed { issue: core::Issue, message: String },
}
impl From<core::Error> for LibraryError {
    fn from(e: core::Error) -> Self {
        Self::Failed { issue: e.issue, message: e.message }
    }
}
#[uniffi::remote(Enum)]
pub enum Issue {
    Busy,
    RequiresUpdate,
    AppMismatch,
    TooLarge,
    AccountChanged,
    StaleCatalog,
    Failed,
}
#[uniffi::remote(Record)]
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
    pub downloaded: bool,
    pub problem: Option<String>,
}
#[uniffi::remote(Record)]
pub struct Retry {
    pub id: String,
    pub title: String,
    pub template: String,
    pub message: String,
    pub retry_at: f64,
    pub incoming: bool,
}
#[uniffi::remote(Record)]
pub struct Upload {
    pub entry: core::Entry,
    pub asset_path: Option<String>,
    pub fields: Option<Vec<u8>>,
    pub authority: core::CatalogAuthority,
}
#[uniffi::remote(Record)]
pub struct CatalogStamp {
    pub revision: u64,
    pub change_tag: String,
}
#[uniffi::remote(Record)]
pub struct CatalogReceipt {
    pub transition: String,
    pub room: String,
    pub freeze: String,
    pub snapshot_version: String,
}
#[uniffi::remote(Record)]
pub struct CatalogCheckpointProof {
    pub transition: String,
    pub source_version: String,
    pub version: String,
}
#[uniffi::remote(Record)]
pub struct CatalogRelayBinding {
    pub transition: String,
    pub room: String,
    pub seed_hash: String,
}
#[uniffi::export]
pub fn library_relay_binding(
    authority: core::CatalogAuthority,
) -> Result<Option<core::CatalogRelayBinding>, LibraryError> {
    Ok(authority.relay_binding()?)
}
#[uniffi::remote(Enum)]
pub enum CatalogAuthority {
    Snapshot {
        receipt: Option<core::CatalogReceipt>,
    },
    Checkpoint {
        proof: core::CatalogCheckpointProof,
    },
    Preparing {
        transition: String,
        room: String,
        seed_hash: Option<String>,
        prior_checkpoint: Option<core::CatalogCheckpointProof>,
    },
    Relay {
        transition: String,
        room: String,
        seed_hash: String,
    },
    Handoff {
        transition: String,
        room: String,
        freeze: String,
    },
}
#[uniffi::export]
pub fn library_decode_authority(format: u32, json: Option<String>) -> Result<core::CatalogAuthority, LibraryError> {
    Ok(core::CatalogAuthority::decode(format, json.as_deref())?)
}
#[uniffi::export]
pub fn library_encode_authority(authority: core::CatalogAuthority) -> Result<String, LibraryError> {
    Ok(authority.encode()?)
}
#[derive(uniffi::Object)]
pub struct NativeLibrary(Arc<core::Library>);
#[uniffi::export]
impl NativeLibrary {
    #[uniffi::constructor]
    pub fn open(root: String, account: String) -> Result<Arc<Self>, LibraryError> {
        Ok(Arc::new(Self(core::Library::open(Path::new(&root), &account)?)))
    }
    pub fn entries(&self) -> Result<Vec<core::Entry>, LibraryError> {
        Ok(self.0.entries()?)
    }
    pub fn register_owner(&self, owner: Arc<crate::owner::NativeOwner>) -> Result<(), LibraryError> {
        Ok(self.0.register(&owner.0)?)
    }
    pub fn create(&self, template: String, title: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.create(Path::new(&template), &title)?)
    }
    pub fn adopt(&self, source: String, title: String) -> Result<core::Entry, LibraryError> {
        Ok(self.0.adopt(Path::new(&source), &title)?)
    }
    pub fn trash_adopted(&self, source: String, id: String) -> Result<(), LibraryError> {
        #[cfg(target_os = "macos")]
        {
            Ok(self.0.trash_adopted(Path::new(&source), &id)?)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (source, id);
            Err(LibraryError::Failed { issue: Issue::Failed, message: "Move to iCloud is available on Mac".into() })
        }
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
    pub fn catalog_stamp(&self, id: String) -> Result<Option<core::CatalogStamp>, LibraryError> {
        Ok(self.0.catalog_stamp(&id)?)
    }
    pub fn authority(&self, id: String) -> Result<core::CatalogAuthority, LibraryError> {
        Ok(self.0.authority(&id)?)
    }
    pub fn pending_authority(&self, id: String) -> Result<Option<core::CatalogAuthority>, LibraryError> {
        Ok(self.0.pending_authority(&id)?)
    }
    pub fn prepare_authority(&self, id: String, authority: core::CatalogAuthority) -> Result<(), LibraryError> {
        Ok(self.0.prepare_authority(&id, authority)?)
    }
    pub fn accept_authority(
        &self,
        id: String,
        title: String,
        template: String,
        authority: core::CatalogAuthority,
        fields: Vec<u8>,
        stamp: Option<core::CatalogStamp>,
    ) -> Result<(), LibraryError> {
        Ok(self.0.accept_authority(&id, &title, &template, authority, &fields, stamp.as_ref())?)
    }
    pub fn admit_authority(
        &self,
        id: String,
        authority: core::CatalogAuthority,
        stamp: Option<core::CatalogStamp>,
    ) -> Result<(), LibraryError> {
        Ok(self.0.admit_authority(&id, &authority, stamp.as_ref())?)
    }
    pub fn stage_preparing(&self, id: String, transition: String) -> Result<core::Upload, LibraryError> {
        Ok(self.0.stage_preparing(&id, &transition)?)
    }
    pub fn confirm_relay_download(&self, id: String, room: String, seed_hash: String) -> Result<(), LibraryError> {
        Ok(self.0.confirm_relay_download(&id, &room, &seed_hash)?)
    }
    pub fn stage_handoff(&self, id: String, transition: String) -> Result<core::Upload, LibraryError> {
        Ok(self.0.stage_handoff(&id, &transition)?)
    }
    pub fn pending_checkpoint_new_record(&self, id: String) -> Result<bool, LibraryError> {
        Ok(self.0.pending_checkpoint_new_record(&id)?)
    }
    pub fn has_pending_checkpoint(&self, id: String) -> Result<bool, LibraryError> {
        Ok(self.0.has_pending_checkpoint(&id)?)
    }
    pub fn stage_allowed(&self, id: String, explicit: bool) -> Result<bool, LibraryError> {
        Ok(self.0.stage_allowed(&id, explicit)?)
    }
    pub fn stage_sync(&self, id: String, explicit: bool) -> Result<Option<core::Upload>, LibraryError> {
        Ok(self.0.stage_sync(&id, explicit)?)
    }
    pub fn stage(&self, id: String) -> Result<core::Upload, LibraryError> {
        Ok(self.0.stage(&id)?)
    }
    #[allow(clippy::too_many_arguments, reason = "A single complete cloud record enters at this boundary")]
    pub fn receive(
        &self,
        id: String,
        title: String,
        template: String,
        asset: Option<String>,
        version: String,
        fields: Vec<u8>,
        deleted: bool,
        artwork_version: Option<String>,
        asset_revision: Option<String>,
        stamp: Option<core::CatalogStamp>,
    ) -> Result<(), LibraryError> {
        Ok(self.0.receive_with_artwork(
            &id,
            &title,
            &template,
            asset.as_deref().map(Path::new),
            &version,
            &fields,
            deleted,
            artwork_version.as_deref(),
            asset_revision.as_deref(),
            stamp.as_ref(),
        )?)
    }
    pub fn block(&self, id: String, title: String, template: String, message: String) -> Result<(), LibraryError> {
        Ok(self.0.block(&id, &title, &template, &message)?)
    }
    #[allow(clippy::too_many_arguments, reason = "A complete accepted cloud snapshot enters this boundary")]
    pub fn receive_catalog_snapshot(
        &self,
        id: String,
        title: String,
        template: String,
        asset: Option<String>,
        version: String,
        fields: Vec<u8>,
        deleted: bool,
        artwork_version: Option<String>,
        asset_revision: Option<String>,
        authority: core::CatalogAuthority,
        stamp: Option<core::CatalogStamp>,
    ) -> Result<(), LibraryError> {
        Ok(self.0.receive_catalog_snapshot(
            &id,
            &title,
            &template,
            asset.as_deref().map(Path::new),
            &version,
            &fields,
            deleted,
            artwork_version.as_deref(),
            asset_revision.as_deref(),
            authority,
            stamp.as_ref(),
        )?)
    }
    pub fn acknowledge(
        &self,
        id: String,
        version: String,
        asset_revision: Option<String>,
        fields: Vec<u8>,
        deleted: bool,
        stamp: Option<core::CatalogStamp>,
    ) -> Result<(), LibraryError> {
        Ok(self.0.acknowledge_asset(&id, &version, asset_revision.as_deref(), &fields, deleted, stamp.as_ref())?)
    }
    pub fn delete(&self, id: String) -> Result<(), LibraryError> {
        Ok(self.0.delete(&id)?)
    }
    pub fn remove_download(&self, id: String) -> Result<(), LibraryError> {
        Ok(self.0.remove_download(&id)?)
    }
    #[allow(clippy::too_many_arguments, reason = "One complete cloud metadata record enters at this boundary")]
    pub fn receive_removed_metadata(
        &self,
        id: String,
        title: String,
        template: String,
        version: String,
        fields: Vec<u8>,
        artwork_version: Option<String>,
        asset_revision: Option<String>,
        stamp: Option<core::CatalogStamp>,
    ) -> Result<bool, LibraryError> {
        Ok(self.0.receive_removed_metadata(
            &id,
            &title,
            &template,
            &version,
            &fields,
            artwork_version.as_deref(),
            asset_revision.as_deref(),
            stamp.as_ref(),
        )?)
    }
    pub fn cleanup(&self) -> Result<(), LibraryError> {
        Ok(self.0.cleanup()?)
    }
    pub fn confirmed_revision(&self, id: String) -> Result<Option<Vec<String>>, LibraryError> {
        Ok(self.0.confirmed_revision(&id)?.map(|(version, asset)| vec![version, asset]))
    }
    pub fn retries(&self) -> Result<Vec<core::Retry>, LibraryError> {
        Ok(self.0.retries()?)
    }
    pub fn retry(&self, retry: core::Retry) -> Result<(), LibraryError> {
        Ok(self.0.retry(&retry)?)
    }
    pub fn prepare_deletion_reupload(&self, id: String) -> Result<bool, LibraryError> {
        Ok(self.0.prepare_deletion_reupload(&id)?)
    }
    pub fn clear_retry(&self, id: String) -> Result<(), LibraryError> {
        Ok(self.0.clear_retry(&id)?)
    }
    pub fn change_token(&self) -> Result<Option<Vec<u8>>, LibraryError> {
        Ok(self.0.change_token()?)
    }
    pub fn save_change_token(&self, bytes: Vec<u8>) -> Result<(), LibraryError> {
        Ok(self.0.save_change_token(&bytes)?)
    }
    pub fn release_upload(&self, path: String) -> Result<(), LibraryError> {
        Ok(self.0.release_upload(Path::new(&path))?)
    }
}
