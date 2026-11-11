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
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
    pub reason: Option<Code>,
    pub op_index: Option<u32>,
    pub path: Vec<String>,
}
// Request destinations are file paths, which Swift passes as strings.
uniffi::custom_type!(PathBuf, String, {
    remote,
    lower: |path| path.to_string_lossy().into_owned(),
    try_lift: |path| Ok(PathBuf::from(path)),
});
/// Native input remains opaque JSON; decoding failures complete through the same
/// callback as owner refusals, rather than failing during UniFFI argument lifting.
#[derive(uniffi::Enum)]
pub enum Request {
    State,
    Apply {
        batch: String,
        origin: Origin,
    },
    Undo {
        redo: bool,
    },
    Flush,
    Discard,
    Close {
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// Moves a document that cannot save to a new file at `dest`, holding its live state,
    /// and releases it without saving. Refused (the file exists, the copy fails its check)
    /// with the owner open and every edit untouched, after the host's close barrier.
    Handoff {
        dest: PathBuf,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// A copy a person keeps, as a document of its own, with the artwork rendered for it.
    Copy {
        destination: PathBuf,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// The saved document as stored, for a capture to render once.
    CaptureSource {
        destination: PathBuf,
    },
    /// The complete saved file, durably exported with identity, history and artwork.
    Backup {
        destination: PathBuf,
    },
    Artwork {
        name: Artwork,
    },
    SetArtwork {
        source_version: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// The palette; changes are batch intents from the window.
    Theme,
    ExportTheme,
    ChangeTheme {
        change: ThemeChange,
    },
}
/// The owner's reply as the host receives it: the core's, without what only the core's
/// command path reads, or the failure.
#[derive(uniffi::Enum)]
pub enum OwnerReply {
    Unit,
    State { json: String },
    ThemeFile { json: String },
    Applied { sequence: u64, ids: Vec<String> },
    Theme { state: ThemeState, sequence: u64 },
    Bytes { bytes: Option<Vec<u8>> },
    Failed { failure: Failure },
}
#[uniffi::remote(Enum)]
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
pub enum Event {
    Publication { json: String },
    SaveStatus { status: SaveStatus, failure: Option<Failure>, pressure: Pressure, shared: bool },
    UndoState { can_undo: bool, can_redo: bool },
    ThemeChanged,
}
#[uniffi::export(callback_interface)]
pub trait OwnerCompletion: Send + Sync {
    fn complete(&self, reply: OwnerReply);
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
    pub fn submit(&self, request: Request, view: Option<String>, completion: Box<dyn OwnerCompletion>) {
        let options = core::CallOptions { view, deadline: None };
        match request {
            Request::State => self.0.state(
                options,
                native_completion(completion, |value: core::Reading| OwnerReply::State {
                    json: value.reading.to_json(),
                }),
            ),
            Request::Apply { batch, origin } => match Batch::decode(&batch) {
                Ok(batch) => self.0.apply(batch, origin, options, native_completion(completion, applied)),
                Err(error) => completion.complete(OwnerReply::Failed { failure: error.into() }),
            },
            Request::Undo { redo } => {
                let callback = native_completion(completion, applied);
                if redo {
                    self.0.redo(options, callback);
                } else {
                    self.0.undo(options, callback);
                }
            }
            Request::Flush => self.0.flush(options, native_completion(completion, unit)),
            Request::Discard => self.0.discard(options, native_completion(completion, unit)),
            Request::Close { preview, icon } => {
                self.0.close(preview, icon, options, native_completion(completion, unit))
            }
            Request::Handoff { dest, preview, icon } => {
                self.0.handoff(dest, preview, icon, options, native_completion(completion, unit))
            }
            Request::Copy { destination, preview, icon } => {
                self.0.copy(destination, preview, icon, options, native_completion(completion, unit))
            }
            Request::CaptureSource { destination } => {
                self.0.capture_source(destination, options, native_completion(completion, unit))
            }
            Request::Backup { destination } => self.0.backup(destination, options, native_completion(completion, unit)),
            Request::Artwork { name } => {
                self.0.artwork(name, options, native_completion(completion, |bytes| OwnerReply::Bytes { bytes }))
            }
            Request::SetArtwork { source_version, preview, icon } => {
                self.0.set_artwork(source_version, preview, icon, options, native_completion(completion, unit))
            }
            Request::Theme => self.0.theme(
                options,
                native_completion(completion, |core::Theme { state, sequence }| OwnerReply::Theme {
                    state: state.into(),
                    sequence,
                }),
            ),
            Request::ExportTheme => {
                self.0.export_theme(options, native_completion(completion, |json| OwnerReply::ThemeFile { json }))
            }
            Request::ChangeTheme { change } => {
                self.0.change_theme(change, options, native_completion(completion, applied))
            }
        }
    }
}
fn unit((): ()) -> OwnerReply {
    OwnerReply::Unit
}
fn applied(core::Applied { sequence, ids, .. }: core::Applied) -> OwnerReply {
    OwnerReply::Applied { sequence, ids }
}
fn native_completion<T>(
    completion: Box<dyn OwnerCompletion>,
    success: impl FnOnce(T) -> OwnerReply + Send + 'static,
) -> core::Completion<T> {
    Box::new(move |result| {
        completion.complete(match result {
            Ok(value) => success(value),
            Err(failure) => OwnerReply::Failed { failure },
        })
    })
}
