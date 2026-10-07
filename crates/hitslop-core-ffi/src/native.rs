//! Typed renderer messages. Swift never inspects their JSON representation.
use hitslop_core::engine::{ArtworkTarget, ExportFormat};
use hitslop_core::native::{NativeReply, NativeRequest};
use hitslop_core::{Code, OutcomeCode};

#[uniffi::remote(Enum)]
pub enum ArtworkTarget {
    Preview,
    Icon,
}
#[uniffi::remote(Enum)]
pub enum ExportFormat {
    Png,
    Pdf,
}
#[uniffi::remote(Enum)]
pub enum OutcomeCode {
    Rejected,
    OwnerReplaced,
    Closing,
    SaveFailed,
    OwnerInvalidated,
    UnknownOutcome,
}
#[uniffi::remote(Enum)]
pub enum Code {
    TypeMismatch,
    OutOfRange,
    PathNotFound,
    InvalidKey,
    Exists,
    DuplicateId,
    InvalidRequest,
    InvalidId,
    InvalidPath,
    InvalidSchema,
    TooLarge,
    StaleBase,
    InvalidVersion,
    InvalidBytes,
    MissingDependencies,
    EngineError,
    InvalidShape,
    RequiresUpdate,
    IsTemplate,
}
#[uniffi::remote(Enum)]
pub enum NativeRequest {
    Open { document_path: String },
    Screenshot { document_path: String, output: String, target: ArtworkTarget, if_present: bool },
    Export { document_path: String, format: ExportFormat, output: String },
}
#[uniffi::remote(Enum)]
pub enum NativeReply {
    Open { document_path: String },
    Screenshot { output: Option<String> },
    Export { output: String },
    Failure { error: String, code: OutcomeCode, reason: Option<Code>, op_index: Option<u32> },
}
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum NativeRefusal {
    #[error("refused")]
    Refused { reply: NativeReply },
}
/// Decode and check one bounded request before the helper starts AppKit.
#[uniffi::export]
pub fn decode_native_request(input: Vec<u8>) -> Result<NativeRequest, NativeRefusal> {
    NativeRequest::decode(&input).map_err(|reply| NativeRefusal::Refused { reply })
}
#[uniffi::export]
pub fn encode_native_reply(reply: NativeReply) -> String {
    reply.to_json()
}

#[uniffi::export]
pub fn parse_core_code(name: String) -> Option<Code> {
    Code::from_name(&name)
}
#[uniffi::export]
pub fn core_code_name(code: Code) -> String {
    code.as_str().into()
}
#[uniffi::export]
pub fn parse_outcome_code(name: String) -> Option<OutcomeCode> {
    OutcomeCode::from_name(&name)
}
#[uniffi::export]
pub fn outcome_code_name(code: OutcomeCode) -> String {
    code.name()
}
