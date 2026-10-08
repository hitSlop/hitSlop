//! Page transport stays opaque in Swift; only native actions cross as typed values.
use crate::CoreError;
use hitslop_core::command::PageDispatch;
use hitslop_core::owner::Failure;
use hitslop_core::page_wire::{CaptureMode, HostAction, HostCaptureResult, HostReply, HostRequest, PageErrorKind};

#[uniffi::remote(Enum)]
pub enum PageErrorKind {
    Operation,
    Application,
}
#[uniffi::remote(Enum)]
pub enum HostAction {
    WindowResize { width: u32, height: u32 },
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
    Ready,
    PageRecovered,
    Failed,
    PageError,
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
    CaptureBegin { token: String, mode: CaptureMode },
    CaptureSettle { token: String },
    CaptureRestore { token: String },
}
#[uniffi::remote(Enum)]
pub enum CaptureMode {
    Preview,
    Export,
    Icon,
}
#[uniffi::remote(Record)]
pub struct HostCaptureResult {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub dedicated: bool,
}
#[uniffi::export]
pub fn encode_host_reply(reply: HostReply) -> String {
    reply.to_json()
}
#[uniffi::export]
pub fn encode_host_request(request: HostRequest) -> String {
    request.to_json()
}
#[uniffi::export]
pub fn decode_host_capture_result(json: String) -> Result<HostCaptureResult, CoreError> {
    Ok(HostCaptureResult::decode(&json)?)
}
