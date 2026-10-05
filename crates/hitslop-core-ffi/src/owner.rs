//! Typed native adapter; sequencing and persistence remain in hitslop-core::owner.
use super::*;
use hitslop_core::owner as core;

#[derive(uniffi::Enum)]
pub enum OwnerFailureKind {
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
#[derive(uniffi::Record)]
pub struct OwnerFailure {
    pub kind: OwnerFailureKind,
    pub message: String,
    pub reason: Option<String>,
    pub op_index: Option<u32>,
}
impl From<core::Failure> for OwnerFailure {
    fn from(value: core::Failure) -> Self {
        use core::FailureKind as K;
        Self {
            kind: match value.kind {
                K::Rejected => OwnerFailureKind::Rejected,
                K::Replaced => OwnerFailureKind::Replaced,
                K::Closing => OwnerFailureKind::Closing,
                K::Closed => OwnerFailureKind::Closed,
                K::ReadOnly => OwnerFailureKind::ReadOnly,
                K::Invalidated => OwnerFailureKind::Invalidated,
                K::Locked => OwnerFailureKind::Locked,
                K::Busy => OwnerFailureKind::Busy,
                K::Full => OwnerFailureKind::Full,
                K::Moved => OwnerFailureKind::Moved,
                K::SaveFailed => OwnerFailureKind::SaveFailed,
                K::Failed => OwnerFailureKind::Failed,
            },
            message: value.message,
            reason: value.reason,
            op_index: value.op_index,
        }
    }
}
impl From<core::Failure> for CoreError {
    fn from(value: core::Failure) -> Self {
        match value.kind {
            core::FailureKind::Rejected => Self::Rejected {
                code: value.reason.unwrap_or_else(|| "invalid_request".into()),
                message: value.message,
                op_index: value.op_index,
            },
            core::FailureKind::Invalidated => Self::Invalidated {
                message: value.message,
            },
            core::FailureKind::Locked => Self::Locked,
            core::FailureKind::Busy => Self::Busy,
            core::FailureKind::Full => Self::Full,
            core::FailureKind::Moved => Self::Moved,
            core::FailureKind::Closed => Self::Closed,
            _ => Self::Failed {
                message: value.message,
            },
        }
    }
}
#[derive(uniffi::Enum)]
pub enum OwnerRequest {
    State,
    Apply {
        batch_json: String,
        origin: EditOrigin,
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
    Copy {
        destination: String,
        durable: bool,
    },
    Artwork {
        name: String,
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
impl From<OwnerRequest> for core::Request {
    fn from(value: OwnerRequest) -> Self {
        match value {
            OwnerRequest::State => Self::State,
            OwnerRequest::Apply { batch_json, origin } => Self::Apply {
                batch_json,
                origin: origin.into(),
            },
            OwnerRequest::Undo { redo } => Self::Undo { redo },
            OwnerRequest::Flush => Self::Flush,
            OwnerRequest::Discard => Self::Discard,
            OwnerRequest::Close { preview, icon } => Self::Close { preview, icon },
            OwnerRequest::Copy { destination, durable } => Self::Copy {
                destination: destination.into(),
                durable,
            },
            OwnerRequest::Artwork { name } => Self::Artwork { name },
            OwnerRequest::Attachments => Self::Attachments,
            OwnerRequest::ReadAttachment { id } => Self::ReadAttachment { id },
            OwnerRequest::PutAttachment { bytes } => Self::PutAttachment { bytes },
            OwnerRequest::Theme => Self::Theme,
            OwnerRequest::ExportTheme => Self::ExportTheme,
        }
    }
}
#[derive(uniffi::Enum)]
pub enum OwnerReply {
    Unit,
    State {
        json: String,
    },
    Applied {
        sequence: u64,
        ids: Vec<String>,
    },
    Theme {
        state: ThemeState,
        sequence: u64,
    },
    Bytes {
        bytes: Option<Vec<u8>>,
    },
    Attachments {
        items: Vec<AttachmentRecord>,
    },
    Attachment {
        item: AttachmentRecord,
    },
    Failed {
        failure: OwnerFailure,
    },
}
impl From<core::Reply> for OwnerReply {
    fn from(value: core::Reply) -> Self {
        match value {
            core::Reply::Unit => Self::Unit,
            core::Reply::State { json } => Self::State { json },
            core::Reply::Applied { sequence, ids, .. } => Self::Applied { sequence, ids },
            core::Reply::Theme { state, sequence } => Self::Theme {
                state: ThemeState {
                    defaults: state.defaults,
                    overrides: state.overrides,
                    effective: state.effective,
                },
                sequence,
            },
            core::Reply::Bytes { bytes } => Self::Bytes { bytes },
            core::Reply::Attachments { items } => Self::Attachments {
                items: items.into_iter().map(Into::into).collect(),
            },
            core::Reply::Attachment { item } => Self::Attachment { item: item.into() },
        }
    }
}
#[derive(uniffi::Enum)]
pub enum OwnerSaveStatus {
    Saved,
    Saving,
    Failed,
}
#[derive(uniffi::Enum)]
pub enum OwnerEvent {
    Publication {
        json: String,
    },
    SaveStatus {
        status: OwnerSaveStatus,
        failure: Option<OwnerFailure>,
    },
    UndoState {
        can_undo: bool,
        can_redo: bool,
    },
    ThemeChanged,
}
impl From<core::Event> for OwnerEvent {
    fn from(value: core::Event) -> Self {
        match value {
            core::Event::Publication { json } => Self::Publication { json },
            core::Event::ThemeChanged => Self::ThemeChanged,
            core::Event::UndoState { can_undo, can_redo } => Self::UndoState { can_undo, can_redo },
            core::Event::SaveStatus { status, failure } => Self::SaveStatus {
                status: match status {
                    core::SaveStatus::Saved => OwnerSaveStatus::Saved,
                    core::SaveStatus::Saving => OwnerSaveStatus::Saving,
                    core::SaveStatus::Failed => OwnerSaveStatus::Failed,
                },
                failure: failure.map(Into::into),
            },
        }
    }
}
#[uniffi::export(callback_interface)]
pub trait OwnerCompletion: Send + Sync {
    fn complete(&self, reply: OwnerReply);
}
#[uniffi::export(callback_interface)]
pub trait OwnerListener: Send + Sync {
    fn event(&self, event: OwnerEvent);
}

#[derive(uniffi::Object)]
pub struct NativeOwner(pub(crate) Arc<core::Owner>);
#[uniffi::export]
impl NativeOwner {
    #[uniffi::constructor]
    pub fn open(
        path: String,
        mode: StoreMode,
        listener: Box<dyn OwnerListener>,
    ) -> Result<Arc<Self>, CoreError> {
        let mode = match mode {
            StoreMode::Document => store::Mode::Document,
            StoreMode::Snapshot => store::Mode::Snapshot,
        };
        Ok(Arc::new(Self(Arc::new(core::Owner::open(
            Path::new(&path),
            mode,
            Arc::new(move |event| listener.event(event.into())),
        )?))))
    }
    pub fn app(&self) -> OpenedFile {
        self.0.app().into()
    }
    pub fn asset_reader(&self) -> Result<Arc<AssetReader>, CoreError> {
        Ok(Arc::new(AssetReader(Mutex::new(self.0.asset_reader()?))))
    }
    pub fn attach(&self, view: String) {
        self.0.attach(view);
    }
    pub fn publish_discovery(&self, json: String) -> Result<(), CoreError> {
        Ok(self.0.publish_discovery(&json)?)
    }
    pub fn withdraw_discovery(&self) {
        self.0.withdraw_discovery();
    }
    pub fn submit(
        &self,
        request: OwnerRequest,
        view: Option<String>,
        completion: Box<dyn OwnerCompletion>,
    ) {
        self.0.submit(
            request.into(),
            view,
            Box::new(move |result| {
                completion.complete(match result {
                    Ok(reply) => reply.into(),
                    Err(failure) => OwnerReply::Failed {
                        failure: failure.into(),
                    },
                })
            }),
        );
    }
}
