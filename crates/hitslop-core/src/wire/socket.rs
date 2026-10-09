//! Same-build owner socket messages; protocol refusal precedes decoding.
use super::engine::{ExportFormat, check_batch, check_path};
use super::{OutcomeCode, present_option};
use crate::{Code, Error, err};
use serde::{Deserialize, Serialize};
#[cfg(feature = "storage")]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "socket.generated.ts"))]
#[serde(tag = "method", deny_unknown_fields)]
#[allow(non_snake_case)]
pub enum SocketRequest {
    #[serde(rename = "attachments.list")]
    AttachmentsList { protocol: u64, documentPath: String },
    #[serde(rename = "attachments.read")]
    AttachmentsRead { protocol: u64, documentPath: String, attachmentID: String },
    #[serde(rename = "theme.export")]
    ThemeExport { protocol: u64, documentPath: String },
    #[serde(rename = "get")]
    Get { protocol: u64, documentPath: String },
    #[serde(rename = "call")]
    Call {
        protocol: u64,
        documentPath: String,
        command: String,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        args: serde_json::Value,
    },
    #[serde(rename = "describe")]
    Describe { protocol: u64, documentPath: String },
    #[serde(rename = "batch")]
    Batch {
        protocol: u64,
        documentPath: String,
        batch: super::Batch,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        attachments: Option<Vec<String>>,
    },
    #[serde(rename = "copy")]
    Copy { protocol: u64, documentPath: String, output: String },
    #[serde(rename = "export")]
    Export { protocol: u64, documentPath: String, format: ExportFormat, output: String },
}

#[cfg(feature = "storage")]
impl SocketRequest {
    /// Requirements are read without interpreting a future protocol's payload.
    pub fn decode(input: &str) -> Result<Self, Error> {
        if input.len() > super::SOCKET_ATTACHMENT {
            return Err(err(Code::InvalidRequest, "Oversized socket request"));
        }
        check_protocol(input)?;
        let request: Self = serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))?;
        request.check().map_err(|e| err(Code::InvalidRequest, e))?;
        if input.len() > super::SOCKET_REQUEST && !matches!(request, Self::Batch { attachments: Some(_), .. }) {
            return Err(err(Code::InvalidRequest, "Oversized socket request"));
        }
        Ok(request)
    }
    pub fn check(&self) -> Result<(), &'static str> {
        let limit = if matches!(self, Self::Batch { attachments: Some(_), .. }) {
            super::SOCKET_ATTACHMENT
        } else {
            super::SOCKET_REQUEST
        };
        super::check_json_size(self, limit).map_err(|_| "Oversized socket request")?;
        check_path(self.path())?;
        match self {
            Self::Call { command, .. } if !super::engine::valid_command_name(command) => Err("Invalid command name"),
            Self::AttachmentsRead { attachmentID, .. } if !super::valid_attachment_id(attachmentID) => {
                Err("Invalid attachment ID")
            }
            Self::Batch { batch, attachments, .. } => check_batch(batch, attachments.as_deref()),
            Self::Copy { output, .. } | Self::Export { output, .. } => check_path(output),
            _ => Ok(()),
        }
    }
    pub fn method(&self) -> &'static str {
        match self {
            Self::AttachmentsList { .. } => "attachments.list",
            Self::AttachmentsRead { .. } => "attachments.read",
            Self::ThemeExport { .. } => "theme.export",
            Self::Get { .. } => "get",
            Self::Batch { .. } => "batch",
            Self::Call { .. } => "call",
            Self::Describe { .. } => "describe",
            Self::Copy { .. } => "copy",
            Self::Export { .. } => "export",
        }
    }
    pub fn path(&self) -> &str {
        match self {
            Self::AttachmentsList { documentPath, .. } => documentPath,
            Self::AttachmentsRead { documentPath, .. } => documentPath,
            Self::ThemeExport { documentPath, .. } => documentPath,
            Self::Get { documentPath, .. } => documentPath,
            Self::Batch { documentPath, .. }
            | Self::Call { documentPath, .. }
            | Self::Describe { documentPath, .. } => documentPath,
            Self::Copy { documentPath, .. } | Self::Export { documentPath, .. } => documentPath,
        }
    }
}
#[cfg(feature = "storage")]
#[derive(Debug, Serialize)]
#[serde(tag = "method", deny_unknown_fields)]
#[allow(non_snake_case)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum SocketSuccess {
    #[serde(rename = "call")]
    Call { result: Box<serde_json::value::RawValue>, ids: Vec<String> },
    #[serde(rename = "describe")]
    Describe { state: Box<serde_json::value::RawValue> },
    #[serde(rename = "get")]
    Get { state: Box<serde_json::value::RawValue> },
    #[serde(rename = "batch")]
    Batch { ids: Vec<String> },
    #[serde(rename = "copy")]
    Copy { output: String },
    #[serde(rename = "export")]
    Export { output: String },
    #[serde(rename = "theme.export")]
    ThemeExport { state: Box<serde_json::value::RawValue> },
    #[serde(rename = "attachments.list")]
    AttachmentsList { state: Box<serde_json::value::RawValue> },
    #[serde(rename = "attachments.read")]
    AttachmentsRead { state: Box<serde_json::value::RawValue> },
}

#[cfg(feature = "storage")]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
pub(crate) struct SocketFailure {
    pub error: String,
    pub code: OutcomeCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opIndex: Option<u32>,
}

/// The only fields a discovery reader interprets. Other fields are always ignored,
/// including values a future protocol could not represent in this build.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "socket.generated.ts"))]
pub struct Discovery {
    pub document_path: String,
    pub socket: String,
}
impl Discovery {
    pub fn decode(input: &str) -> Result<Self, Error> {
        if input.len() > super::SOCKET_ATTACHMENT {
            return Err(err(Code::InvalidRequest, "Oversized discovery"));
        }
        let value: Self = serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))?;
        check_path(&value.socket)
            .and_then(|()| check_path(&value.document_path))
            .map_err(|e| err(Code::InvalidRequest, e))?;
        Ok(value)
    }
}
#[derive(Deserialize)]
pub(crate) struct ProtocolHeader {
    pub protocol: u64,
}
pub(crate) fn check_protocol(input: &str) -> Result<(), Error> {
    let header: ProtocolHeader = serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))?;
    if let Some(message) = crate::command::protocol_mismatch(header.protocol) {
        return Err(err(Code::RequiresUpdate, message));
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum Hello {
    #[serde(rename = "hello")]
    Hello { protocol: u64 },
}
#[derive(Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum HelloSuccess {
    #[serde(rename = "hello")]
    Hello {
        #[serde(rename = "ok")]
        _ok: super::engine::True,
    },
}
