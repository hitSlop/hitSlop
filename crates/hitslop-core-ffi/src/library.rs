//! Paths, typed library metadata and opaque CloudKit state; never Loro payloads.
use super::*;
use core::{Entry, Issue, Upload};
use hitslop_core::library as core;
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
    pub problem: Option<String>,
}
#[uniffi::remote(Record)]
pub struct Upload {
    pub entry: core::Entry,
    pub asset_path: Option<String>,
    pub fields: Option<Vec<u8>>,
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
    pub fn publish_artwork(
        &self,
        owner: Arc<crate::owner::NativeOwner>,
        source_version: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    ) -> Result<bool, LibraryError> {
        Ok(self.0.publish_artwork(&owner.0, &source_version, preview, icon)?)
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
        )?)
    }
    pub fn block(&self, id: String, title: String, template: String, message: String) -> Result<(), LibraryError> {
        Ok(self.0.block(&id, &title, &template, &message)?)
    }
    pub fn acknowledge(
        &self,
        id: String,
        version: String,
        asset_revision: Option<String>,
        fields: Vec<u8>,
        deleted: bool,
    ) -> Result<(), LibraryError> {
        Ok(self.0.acknowledge_asset(&id, &version, asset_revision.as_deref(), &fields, deleted)?)
    }
    pub fn delete(&self, id: String) -> Result<(), LibraryError> {
        Ok(self.0.delete(&id)?)
    }
    pub fn cleanup(&self) -> Result<(), LibraryError> {
        Ok(self.0.cleanup()?)
    }
    pub fn engine_state(&self) -> Result<Option<Vec<u8>>, LibraryError> {
        Ok(self.0.engine_state()?)
    }
    pub fn save_engine_state(&self, bytes: Vec<u8>) -> Result<(), LibraryError> {
        Ok(self.0.save_engine_state(&bytes)?)
    }
    pub fn release_upload(&self, path: String) -> Result<(), LibraryError> {
        Ok(self.0.release_upload(Path::new(&path))?)
    }
}
