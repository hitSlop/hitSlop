//! Engine process messages. Rust owns the shape and checks; ts-rs exports the client types.

use super::{Code, OutcomeCode, Segment, present_option};
use crate::file::Catalog;
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::collections::BTreeMap;

/// A palette: declared token names and their colors.
pub type ThemeValues = BTreeMap<String, String>;

/// `true` on the wire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "true"))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct True;
impl Serialize for True {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(true)
    }
}
impl<'de> Deserialize<'de> for True {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if bool::deserialize(d)? { Ok(True) } else { Err(serde::de::Error::custom("expected true")) }
    }
}
/// `false` on the wire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "false"))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct False;
impl Serialize for False {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(false)
    }
}
impl<'de> Deserialize<'de> for False {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if bool::deserialize(d)? { Err(serde::de::Error::custom("expected false")) } else { Ok(False) }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum ExportFormat {
    Png,
    Pdf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum ArtworkTarget {
    Preview,
    Icon,
}

/// One request on the engine's standard input. Protocol negotiation stays outside it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", deny_unknown_fields, rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum EngineRequest {
    #[serde(rename = "attachments.list")]
    AttachmentsList { document_path: String },
    #[serde(rename = "attachments.read")]
    AttachmentsRead {
        document_path: String,
        #[serde(rename = "attachmentID")]
        attachment_id: String,
    },
    #[serde(rename = "theme.export")]
    ThemeExport { document_path: String },
    #[serde(rename = "get")]
    Get { document_path: String },
    #[serde(rename = "batch")]
    Batch {
        document_path: String,
        batch: super::Batch,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        attachments: Option<Vec<String>>,
    },
    #[serde(rename = "export")]
    Export { document_path: String, format: ExportFormat, output: String },
    #[serde(rename = "templates")]
    Templates {},
    #[serde(rename = "create")]
    Create { from: String, output: String },
    #[serde(rename = "inspect")]
    Inspect { file: String },
    #[serde(rename = "artwork.export")]
    ArtworkExport { file: String, target: ArtworkTarget, output: String },
    #[serde(rename = "schema")]
    Schema { file: String },
    #[serde(rename = "pack")]
    Pack {
        stage: String,
        file: String,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        app: Box<RawValue>,
    },
    #[serde(rename = "validateApp")]
    ValidateApp {
        stage: String,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        app: Box<RawValue>,
    },
    #[serde(rename = "validateMetadata")]
    ValidateMetadata { metadata: crate::app::AppMetadata },
    #[serde(rename = "describe")]
    Describe { document_path: String },
    #[serde(rename = "call")]
    Call {
        document_path: String,
        command: String,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        args: Value,
    },
    #[serde(rename = "open")]
    Open { document_path: String },
    #[serde(rename = "screenshot")]
    Screenshot { document_path: String, output: String, target: ArtworkTarget, if_present: bool },
}

impl EngineRequest {
    /// Decode and check one bounded request. A future app must reach its marker
    /// check before any JSON value inside it is interpreted; serde's tagged-enum
    /// buffering cannot retain RawValue, so only that variant needs a raw path.
    pub fn parse(input: &str) -> serde_json::Result<Self> {
        use serde::de::Error;
        if input.len() > super::SOCKET_ATTACHMENT {
            return Err(serde_json::Error::custom("Engine request is too large"));
        }
        #[derive(Deserialize)]
        struct Header {
            method: String,
        }
        let header: Header = serde_json::from_str(input)?;
        // RawValue is read directly from the input, before serde's tagged-enum buffer.
        macro_rules! build_request {
            ($variant:ident, $($field:ident),+) => {{
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Envelope { #[serde(rename="method")] _method:String, app:Box<RawValue>, $($field:String),+ }
                let value:Envelope=serde_json::from_str(input)?;
                Self::$variant {app:value.app, $($field:value.$field),+}
            }};
        }
        let request = match header.method.as_str() {
            "validateApp" => build_request!(ValidateApp, stage),
            "pack" => build_request!(Pack, stage, file),
            _ => serde_json::from_str(input)?,
        };
        request.check().map_err(serde_json::Error::custom)?;
        Ok(request)
    }

    pub fn method(&self) -> &'static str {
        match self {
            Self::AttachmentsList { .. } => "attachments.list",
            Self::AttachmentsRead { .. } => "attachments.read",
            Self::ThemeExport { .. } => "theme.export",
            Self::Get { .. } => "get",
            Self::Batch { .. } => "batch",
            Self::Export { .. } => "export",
            Self::Templates { .. } => "templates",
            Self::Create { .. } => "create",
            Self::Inspect { .. } => "inspect",
            Self::ArtworkExport { .. } => "artwork.export",
            Self::Schema { .. } => "schema",
            Self::Pack { .. } => "pack",
            Self::ValidateApp { .. } => "validateApp",
            Self::ValidateMetadata { .. } => "validateMetadata",
            Self::Describe { .. } => "describe",
            Self::Call { .. } => "call",
            Self::Open { .. } => "open",
            Self::Screenshot { .. } => "screenshot",
        }
    }

    /// Rules beyond the serde shape. Payload interpretation belongs to the core
    /// operation that owns it, after protocol and format requirements are checked.
    pub fn check(&self) -> Result<(), &'static str> {
        match self {
            Self::Get { document_path }
            | Self::AttachmentsList { document_path }
            | Self::ThemeExport { document_path }
            | Self::Describe { document_path }
            | Self::Open { document_path } => check_path(document_path)?,
            Self::AttachmentsRead { document_path, attachment_id } => {
                check_path(document_path)?;
                if !super::valid_attachment_id(attachment_id) {
                    return Err("Invalid attachment ID");
                }
            }
            Self::Batch { document_path, batch, attachments, .. } => {
                check_path(document_path)?;
                check_batch(batch, attachments.as_deref())?;
            }
            Self::Call { document_path, command, .. } => {
                check_path(document_path)?;
                if !valid_command_name(command) {
                    return Err("Invalid command name");
                }
            }
            Self::Export { document_path, output, .. }
            | Self::Screenshot { document_path, output, .. }
            | Self::ArtworkExport { file: document_path, output, .. } => {
                check_path(document_path)?;
                check_path(output)?;
            }
            Self::Create { from, output } => {
                check_path(from)?;
                check_path(output)?;
            }
            Self::Pack { stage, file, .. } => {
                check_path(stage)?;
                check_path(file)?;
            }
            Self::Inspect { file } | Self::Schema { file } => check_path(file)?,
            Self::Templates {} | Self::ValidateMetadata { .. } => {}
            Self::ValidateApp { stage, .. } => check_path(stage)?,
        }
        Ok(())
    }
}

/// Constraints shared by the socket and engine request boundaries.
pub(super) fn check_batch(batch: &super::Batch, attachments: Option<&[String]>) -> Result<(), &'static str> {
    batch.check_size(super::SOCKET_REQUEST).map_err(|_| "Batch exceeds size limit")?;
    if let Some(blobs) = attachments {
        let max = super::ATTACHMENT_FILE_BYTES.div_ceil(3) * 4;
        if blobs.is_empty() || blobs.iter().any(|bytes| bytes.chars().count() > max) {
            return Err("Invalid attachment payload length");
        }
    }
    Ok(())
}

pub(crate) fn valid_command_name(name: &str) -> bool {
    (1..=80).contains(&name.len())
        && name.as_bytes()[0].is_ascii_lowercase()
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// What an agent reads: the descriptor and declared colors, and the document's version,
/// value and effective colors.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct GetState {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub schema: Value,
    pub defaults: ThemeValues,
    pub version: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub value: Value,
    pub theme: ThemeValues,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct ThemeExportState {
    pub file: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct AttachmentInfo {
    pub mime_type: String,
    pub id: String,

    pub byte_length: u64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct AttachmentBytes {
    pub bytes: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum FileKind {
    Template,
    Document,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct NamedSize {
    pub name: String,

    pub bytes: u64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct AttachmentTotals {
    pub count: u64,

    pub bytes: u64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct StateSizes {
    pub checkpoint_bytes: u64,

    pub updates: u64,

    pub update_bytes: u64,
}
/// A file's kind, markers, app and sizes, and whether a live owner published its socket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct InspectInfo {
    pub kind: FileKind,

    pub package_format: u64,
    #[serde(rename = "runtimeABI")]
    pub runtime_abi: u64,
    pub metadata: crate::app::AppMetadata,
    pub window: super::build::WindowInput,
    pub views: crate::app::Views,
    pub assets: Vec<NamedSize>,
    pub artwork: Vec<NamedSize>,
    pub defaults: ThemeValues,
    pub attachments: AttachmentTotals,
    pub state: StateSizes,

    pub bytes: u64,

    pub stored_asset_bytes: u64,
    pub live: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct DescribedField {
    pub path: Vec<Segment>,
    pub kind: String,
    #[serde(deserialize_with = "required_option")]
    pub description: Option<String>,
    pub operations: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct DescribeState {
    pub metadata: crate::app::AppMetadata,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub schema: Value,
    pub version: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub value: Value,
    pub theme: ThemeValues,
    pub fields: Vec<DescribedField>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, { description: string; args: Record<string, unknown> }>"))]
    pub commands: Value,
}

/// Every successful result names its method and carries all fields that method promises.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", deny_unknown_fields, rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum EngineSuccess {
    #[serde(rename = "get")]
    Get { ok: True, state: GetState },
    #[serde(rename = "batch")]
    Batch { ok: True, ids: Vec<String> },
    #[serde(rename = "export")]
    Export { ok: True, output: String },
    #[serde(rename = "theme.export")]
    ThemeExport { ok: True, state: ThemeExportState },
    #[serde(rename = "attachments.list")]
    AttachmentsList { ok: True, state: Vec<AttachmentInfo> },
    #[serde(rename = "attachments.read")]
    AttachmentsRead { ok: True, state: AttachmentBytes },
    #[serde(rename = "call")]
    Call {
        ok: True,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        result: Value,
        ids: Vec<String>,
    },
    #[serde(rename = "templates")]
    Templates { ok: True, catalog: Catalog },
    #[serde(rename = "create")]
    Create { ok: True, document_path: String },
    #[serde(rename = "inspect")]
    Inspect { ok: True, info: InspectInfo },
    #[serde(rename = "artwork.export")]
    ArtworkExport {
        ok: True,
        #[serde(deserialize_with = "required_option")]
        output: Option<String>,
    },
    #[serde(rename = "schema")]
    Schema {
        ok: True,
        #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
        schema: Value,
    },
    #[serde(rename = "pack")]
    Pack { ok: True },
    #[serde(rename = "validateApp")]
    ValidateApp { ok: True },
    #[serde(rename = "validateMetadata")]
    ValidateMetadata { ok: True },
    #[serde(rename = "describe")]
    Describe { ok: True, state: DescribeState },
    #[serde(rename = "open")]
    Open { ok: True, document_path: String },
    #[serde(rename = "screenshot")]
    Screenshot {
        ok: True,
        #[serde(deserialize_with = "required_option")]
        output: Option<String>,
    },
}

/// Nullable fields still have to be present; absence is a malformed acknowledgement.
fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

/// A classified failure. Unknown outcomes are explicit, never inferred from missing fields.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub struct Failure {
    pub ok: False,
    pub error: String,
    pub code: OutcomeCode,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub reason: Option<Code>,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub op_index: Option<u32>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum EngineReply {
    Success(Box<EngineSuccess>),
    Failure(Failure),
}

/// Wire paths retain the JSON Schema boundary's Unicode-scalar length limit.
pub(super) fn check_path(value: &str) -> Result<(), &'static str> {
    if (1..=4096).contains(&value.chars().count()) { Ok(()) } else { Err("Invalid path length") }
}
