//! Page transport stays opaque in Swift; only native actions cross as typed values.
use hitslop_core::command::PageDispatch;
use hitslop_core::owner::Failure;
use hitslop_core::page_wire::{HostAction, HostReply, HostRequest, PageErrorKind, SpeechOutcome, SpeechUnavailable};

#[uniffi::remote(Enum)]
pub enum PageErrorKind {
    Operation,
    Application,
}
#[uniffi::remote(Enum)]
pub enum HostAction {
    WindowResize { width: u32, height: u32 },
    SpeechTranscribe { attachment: String, locale: Option<String> },
    Ready,
    PageRecovered,
    Failed { error: String },
    PageError { kind: PageErrorKind, error: String },
}
#[uniffi::remote(Enum)]
pub enum PageDispatch {
    Reply { json: String, failure: Option<Failure>, storage: bool },
    Host { action: HostAction },
}
#[uniffi::remote(Enum)]
pub enum HostReply {
    WindowResize { width: f64, height: f64 },
    SpeechTranscribe { outcome: SpeechOutcome },
    Ready,
    PageRecovered,
    Failed,
    PageError,
}
#[uniffi::remote(Enum)]
pub enum SpeechOutcome {
    Transcribed { text: String, locale: String },
    Unavailable { reason: SpeechUnavailable, message: String },
    Cancelled { message: String },
    Failed { message: String },
}
#[uniffi::remote(Enum)]
pub enum SpeechUnavailable {
    NotDeclared,
    Host,
    System,
    Locale,
    Permission,
    Format,
}
#[uniffi::remote(Enum)]
pub enum HostRequest {
    Publish { payload: String },
    Flush,
    Undo,
    Redo,
    PrepareClose,
    CancelClose,
    Close,
    ReloadInterface,
}
#[uniffi::export]
pub fn encode_host_reply(reply: HostReply) -> String {
    reply.to_json()
}
#[uniffi::export]
pub fn encode_host_request(request: HostRequest) -> String {
    request.to_json()
}

/// Native presentation receives a field label, never an unparsed document path.
#[derive(uniffi::Record)]
pub struct UnresolvedTextDraft {
    pub field: String,
    pub text: String,
}

#[uniffi::export]
pub fn decode_close_preparation(json: String) -> Result<Vec<UnresolvedTextDraft>, crate::CoreError> {
    Ok(hitslop_core::page_wire::ClosePreparation::decode(&json)?
        .unresolved_drafts
        .into_iter()
        .map(|draft| UnresolvedTextDraft { field: draft.field_label(), text: draft.text })
        .collect())
}
