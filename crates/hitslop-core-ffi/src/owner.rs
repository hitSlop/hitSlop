//! Typed native adapter; sequencing and persistence remain in hitslop-core::owner.
use super::*;
use hitslop_core::owner::{self as core, Event, Failure, FailureKind, Request, SaveStatus, ThemeChange};
use std::path::PathBuf;

#[uniffi::remote(Enum)]
pub enum ThemeChange {
    Set { values: std::collections::HashMap<String, String> },
    ResetAll,
    ImportFile { file: String },
}
#[uniffi::export]
pub fn theme_request(change: ThemeChange) -> Request {
    core::theme_request(change)
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
    pub reason: Option<String>,
    pub op_index: Option<u32>,
}
// Request destinations are file paths, which Swift passes as strings.
uniffi::custom_type!(PathBuf, String, {
    remote,
    lower: |path| path.to_string_lossy().into_owned(),
    try_lift: |path| Ok(PathBuf::from(path)),
});
#[uniffi::remote(Enum)]
pub enum Request {
    Command {
        name: String,
        args_json: String,
        origin: Origin,
    },
    State,
    Apply {
        batch_json: String,
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
    Artwork {
        name: Artwork,
    },
    Attachments,
    ReadAttachment {
        id: String,
    },
    PutAttachment {
        bytes: Vec<u8>,
    },
    /// The palette; changes are batch intents from the window.
    Theme,
    ExportTheme,
}
/// The owner's reply as the host receives it: the core's, without what only the core's
/// command path reads, or the failure.
#[derive(uniffi::Enum)]
pub enum OwnerReply {
    Command { sequence: u64, ids: Vec<String>, result_json: String },
    Unit,
    State { json: String },
    ThemeFile { json: String },
    Applied { sequence: u64, ids: Vec<String> },
    Theme { state: ThemeState, sequence: u64 },
    Bytes { bytes: Option<Vec<u8>> },
    Attachments { items: Vec<Attachment> },
    Attachment { item: Attachment },
    Failed { failure: Failure },
}
impl From<core::Reply> for OwnerReply {
    fn from(value: core::Reply) -> Self {
        match value {
            core::Reply::Command { sequence, ids, result_json } => Self::Command { sequence, ids, result_json },
            core::Reply::Unit => Self::Unit,
            core::Reply::State { json, .. } => Self::State { json },
            core::Reply::ThemeFile { json } => Self::ThemeFile { json },
            core::Reply::Applied { sequence, ids, .. } => Self::Applied { sequence, ids },
            core::Reply::Theme { state, sequence } => Self::Theme { state: state.into(), sequence },
            core::Reply::Bytes { bytes } => Self::Bytes { bytes },
            core::Reply::Attachments { items } => Self::Attachments { items },
            core::Reply::Attachment { item } => Self::Attachment { item },
        }
    }
}
#[uniffi::remote(Enum)]
pub enum SaveStatus {
    Saved,
    Saving,
    Failed,
}
#[uniffi::remote(Enum)]
pub enum Event {
    Publication { json: String },
    SaveStatus { status: SaveStatus, failure: Option<Failure> },
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
            evaluator_path.map(|path| core::Evaluator::new(PathBuf::from(path), vec![])).transpose().map_err(|e| {
                Failure {
                    kind: FailureKind::Rejected,
                    message: e,
                    reason: Some("invalid_request".into()),
                    op_index: None,
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
        self.0.submit(
            request,
            view,
            Box::new(move |result| {
                completion.complete(match result {
                    Ok(reply) => reply.into(),
                    Err(failure) => OwnerReply::Failed { failure },
                })
            }),
        );
    }
}
