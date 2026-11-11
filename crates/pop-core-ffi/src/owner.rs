//! Typed native adapter; sequencing and persistence remain in pop-runtime::owner.
use super::*;
use pop_contracts::wire::{Batch, Code};
use pop_runtime::owner::{self as core, Event, Failure, FailureKind, SaveStatus, ThemeChange};
use pop_storage::store::Pressure;
use std::path::PathBuf;
#[uniffi::remote(Enum)]
pub enum ThemeChange {
    Set { values: std::collections::HashMap<String, String> },
    ResetAll,
    ImportFile { file: String },
}

#[uniffi::remote(Enum)]
#[uniffi(name = "OwnerFailureKind")]
pub enum FailureKind {
    Rejected,
    Replaced,
    Closing,
    Closed,
    ReadOnly,
    Invalidated,
    Locked,
    Busy,
    Full,
    Moved,
    SaveFailed,
    Failed,
}
#[uniffi::remote(Record)]
#[uniffi(name = "OwnerFailure")]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
    pub reason: Option<Code>,
    pub op_index: Option<u32>,
    pub path: Vec<String>,
}
#[derive(uniffi::Record)]
pub struct OwnerApplied {
    pub sequence: u64,
    pub ids: Vec<String>,
}
impl From<core::Applied> for OwnerApplied {
    fn from(value: core::Applied) -> Self {
        Self { sequence: value.sequence, ids: value.ids }
    }
}
#[derive(uniffi::Record)]
pub struct OwnerTheme {
    pub state: ThemeState,
    pub sequence: u64,
}
impl From<core::Theme> for OwnerTheme {
    fn from(value: core::Theme) -> Self {
        Self { state: value.state.into(), sequence: value.sequence }
    }
}
#[uniffi::remote(Enum)]
#[uniffi(name = "OwnerSaveStatus")]
pub enum SaveStatus {
    Saved,
    Saving,
    Failed,
}
#[uniffi::remote(Enum)]
pub enum Pressure {
    Ok,
    Near,
    Full,
}
#[uniffi::remote(Enum)]
#[uniffi(name = "OwnerEvent")]
pub enum Event {
    Publication { json: String },
    SaveStatus { status: SaveStatus, failure: Option<Failure>, pressure: Pressure, shared: bool },
    UndoState { can_undo: bool, can_redo: bool },
    ThemeChanged,
}
#[uniffi::export(callback_interface)]
pub trait OwnerUnitCompletion: Send + Sync {
    fn complete(&self);
    fn failed(&self, failure: Failure);
}
#[uniffi::export(callback_interface)]
pub trait OwnerAppliedCompletion: Send + Sync {
    fn complete(&self, value: OwnerApplied);
    fn failed(&self, failure: Failure);
}
#[uniffi::export(callback_interface)]
pub trait OwnerThemeCompletion: Send + Sync {
    fn complete(&self, value: OwnerTheme);
    fn failed(&self, failure: Failure);
}
#[uniffi::export(callback_interface)]
pub trait OwnerBytesCompletion: Send + Sync {
    fn complete(&self, value: Option<Vec<u8>>);
    fn failed(&self, failure: Failure);
}
#[uniffi::export(callback_interface)]
pub trait OwnerStringCompletion: Send + Sync {
    fn complete(&self, value: String);
    fn failed(&self, failure: Failure);
}
#[uniffi::export(callback_interface)]
pub trait OwnerListener: Send + Sync {
    fn event(&self, event: Event);
}

#[derive(uniffi::Object)]
pub struct NativeOwner(pub(crate) Arc<core::Owner>);
#[uniffi::export]
impl NativeOwner {
    #[uniffi::constructor]
    pub fn open(
        path: String,
        mode: Mode,
        evaluator_path: Option<String>,
        listener: Box<dyn OwnerListener>,
    ) -> Result<Arc<Self>, CoreError> {
        Ok(Arc::new(Self(Arc::new(core::Owner::open_with_evaluator(
            Path::new(&path),
            mode,
            Arc::new(move |event| listener.event(event)),
            evaluator_path.map(|path| pop_runtime::evaluator(PathBuf::from(path))).transpose().map_err(|e| {
                Failure {
                    kind: FailureKind::Rejected,
                    message: e.to_string(),
                    reason: Some(Code::EngineError),
                    op_index: None,
                    path: vec![],
                }
            })?,
        )?))))
    }
    pub fn app(&self) -> OpenedFile {
        self.0.app().into()
    }
    pub fn resource_reader(&self) -> Result<Arc<ResourceReader>, CoreError> {
        Ok(Arc::new(ResourceReader(Mutex::new(self.0.resource_reader()?))))
    }
    pub fn attach(&self, view: String) {
        self.0.attach(view);
    }
    /// Retained for native page and save-recovery integration tests, including iPhone lifecycle tests.
    pub fn state(&self, completion: Box<dyn OwnerStringCompletion>) {
        self.0.state(
            core::CallOptions::default(),
            string_completion(completion, |value: core::Reading| value.reading.to_json()),
        );
    }
    /// Integration tests inject edits without evaluating an authored app.
    pub fn apply(
        &self,
        batch: String,
        origin: Origin,
        view: Option<String>,
        completion: Box<dyn OwnerAppliedCompletion>,
    ) {
        match Batch::decode(&batch) {
            Ok(batch) => {
                self.0.apply(batch, origin, core::CallOptions { view, deadline: None }, applied_completion(completion))
            }
            Err(error) => completion.failed(error.into()),
        }
    }
    pub fn undo(&self, completion: Box<dyn OwnerAppliedCompletion>) {
        self.0.undo(core::CallOptions::default(), applied_completion(completion));
    }
    pub fn redo(&self, completion: Box<dyn OwnerAppliedCompletion>) {
        self.0.redo(core::CallOptions::default(), applied_completion(completion));
    }
    pub fn flush(&self, completion: Box<dyn OwnerUnitCompletion>) {
        self.0.flush(core::CallOptions::default(), unit_completion(completion));
    }
    pub fn discard(&self, completion: Box<dyn OwnerUnitCompletion>) {
        self.0.discard(core::CallOptions::default(), unit_completion(completion));
    }
    pub fn close(&self, preview: Option<Vec<u8>>, icon: Option<Vec<u8>>, completion: Box<dyn OwnerUnitCompletion>) {
        self.0.close(preview, icon, core::CallOptions::default(), unit_completion(completion));
    }
    pub fn handoff(
        &self,
        dest: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        completion: Box<dyn OwnerUnitCompletion>,
    ) {
        self.0.handoff(PathBuf::from(dest), preview, icon, core::CallOptions::default(), unit_completion(completion));
    }
    pub fn copy(
        &self,
        destination: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        completion: Box<dyn OwnerUnitCompletion>,
    ) {
        self.0.copy(
            PathBuf::from(destination),
            preview,
            icon,
            core::CallOptions::default(),
            unit_completion(completion),
        );
    }
    pub fn capture_source(&self, destination: String, completion: Box<dyn OwnerUnitCompletion>) {
        self.0.capture_source(PathBuf::from(destination), core::CallOptions::default(), unit_completion(completion));
    }
    pub fn backup(&self, destination: String, completion: Box<dyn OwnerUnitCompletion>) {
        self.0.backup(PathBuf::from(destination), core::CallOptions::default(), unit_completion(completion));
    }
    pub fn set_artwork(
        &self,
        source_version: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        completion: Box<dyn OwnerUnitCompletion>,
    ) {
        self.0.set_artwork(source_version, preview, icon, core::CallOptions::default(), unit_completion(completion));
    }
    pub fn artwork(&self, name: Artwork, completion: Box<dyn OwnerBytesCompletion>) {
        self.0.artwork(name, core::CallOptions::default(), bytes_completion(completion));
    }
    pub fn theme(&self, completion: Box<dyn OwnerThemeCompletion>) {
        self.0.theme(core::CallOptions::default(), theme_completion(completion));
    }
    pub fn export_theme(&self, completion: Box<dyn OwnerStringCompletion>) {
        self.0.export_theme(core::CallOptions::default(), string_completion(completion, |value| value));
    }
    pub fn change_theme(&self, change: ThemeChange, completion: Box<dyn OwnerAppliedCompletion>) {
        self.0.change_theme(change, core::CallOptions::default(), applied_completion(completion));
    }
}
fn unit_completion(completion: Box<dyn OwnerUnitCompletion>) -> core::Completion<()> {
    Box::new(move |result| match result {
        Ok(()) => completion.complete(),
        Err(failure) => completion.failed(failure),
    })
}
fn applied_completion(completion: Box<dyn OwnerAppliedCompletion>) -> core::Completion<core::Applied> {
    Box::new(move |result| match result {
        Ok(value) => completion.complete(value.into()),
        Err(failure) => completion.failed(failure),
    })
}
fn theme_completion(completion: Box<dyn OwnerThemeCompletion>) -> core::Completion<core::Theme> {
    Box::new(move |result| match result {
        Ok(value) => completion.complete(value.into()),
        Err(failure) => completion.failed(failure),
    })
}
fn bytes_completion(completion: Box<dyn OwnerBytesCompletion>) -> core::Completion<Option<Vec<u8>>> {
    Box::new(move |result| match result {
        Ok(value) => completion.complete(value),
        Err(failure) => completion.failed(failure),
    })
}
fn string_completion<T: 'static>(
    completion: Box<dyn OwnerStringCompletion>,
    success: impl FnOnce(T) -> String + Send + 'static,
) -> core::Completion<T> {
    Box::new(move |result| match result {
        Ok(value) => completion.complete(success(value)),
        Err(failure) => completion.failed(failure),
    })
}
