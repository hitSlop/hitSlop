//! The renderer's private boundary. Rust decodes once; Swift receives UniFFI enums.
use super::engine::{ArtworkTarget, EngineReply, EngineSuccess, ExportFormat, Failure, False, True, check_path};
use super::{Code, OutcomeCode};
use serde::{Deserialize, Serialize};

/// Only operations requiring AppKit or WebKit reach the helper.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", deny_unknown_fields, rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NativeRequest {
    Open { document_path: String },
    Screenshot { document_path: String, output: String, target: ArtworkTarget, if_present: bool },
    Export { document_path: String, format: ExportFormat, output: String },
}
impl NativeRequest {
    pub fn decode(input: &[u8]) -> Result<Self, NativeReply> {
        if input.len() > super::SOCKET_REQUEST {
            return Err(NativeReply::rejected(Code::TooLarge, "Native request is too large"));
        }
        let request: Self = serde_json::from_slice(input).map_err(|_| Self::invalid())?;
        request.check().map_err(|_| Self::invalid())?;
        Ok(request)
    }
    fn invalid() -> NativeReply {
        NativeReply::rejected(Code::InvalidRequest, "Invalid native request; send document edits to slop-engine")
    }
    pub fn check(&self) -> Result<(), &'static str> {
        match self {
            Self::Open { document_path } => check_path(document_path),
            Self::Screenshot { document_path, output, .. } | Self::Export { document_path, output, .. } => {
                check_path(document_path)?;
                check_path(output)
            }
        }
    }
}

/// Swift constructs these outcomes; Rust supplies the wire's literal discriminants.
/// The helper is a subset of the engine boundary, so both use the same wire serializer.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "EngineReply", into = "EngineReply")]
pub enum NativeReply {
    Open { document_path: String },
    Screenshot { output: Option<String> },
    Export { output: String },
    Failure { error: String, code: OutcomeCode, reason: Option<Code>, op_index: Option<u32> },
}
impl NativeReply {
    fn rejected(reason: Code, error: &str) -> Self {
        Self::Failure { error: error.into(), code: OutcomeCode::Rejected, reason: Some(reason), op_index: None }
    }
    pub fn method(&self) -> Option<&'static str> {
        match self {
            Self::Open { .. } => Some("open"),
            Self::Screenshot { .. } => Some("screenshot"),
            Self::Export { .. } => Some("export"),
            Self::Failure { .. } => None,
        }
    }
    pub fn to_json(&self) -> String {
        crate::encode(self)
    }
}
impl From<NativeReply> for EngineReply {
    fn from(reply: NativeReply) -> Self {
        Self::Success(Box::new(match reply {
            NativeReply::Open { document_path } => EngineSuccess::Open { ok: True, document_path },
            NativeReply::Screenshot { output } => EngineSuccess::Screenshot { ok: True, output },
            NativeReply::Export { output } => EngineSuccess::Export { ok: True, output },
            NativeReply::Failure { error, code, reason, op_index } => {
                return Self::Failure(Failure { ok: False, error, code, reason, op_index });
            }
        }))
    }
}
impl TryFrom<EngineReply> for NativeReply {
    type Error = &'static str;
    fn try_from(reply: EngineReply) -> Result<Self, Self::Error> {
        let success = match reply {
            EngineReply::Success(success) => *success,
            EngineReply::Failure(Failure { error, code, reason, op_index, .. }) => {
                return Ok(Self::Failure { error, code, reason, op_index });
            }
        };
        Ok(match success {
            EngineSuccess::Open { document_path, .. } => {
                check_path(&document_path)?;
                Self::Open { document_path }
            }
            EngineSuccess::Screenshot { output, .. } => {
                if let Some(output) = &output {
                    check_path(output)?;
                }
                Self::Screenshot { output }
            }
            EngineSuccess::Export { output, .. } => {
                check_path(&output)?;
                Self::Export { output }
            }
            _ => return Err("Not a native helper reply"),
        })
    }
}
