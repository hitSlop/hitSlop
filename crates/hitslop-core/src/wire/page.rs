//! Same-build page messages. The owner parses once; native UI receives typed actions.
use super::engine::True;
use crate::{Code, Result, err};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum PageRequest {
    #[serde(rename = "commands.run")]
    CommandsRun {
        name: String,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        args: serde_json::Value,
    },
    Open {},
    Apply {
        batch: super::Batch,
    },
    Flush {},
    Undo {},
    Redo {},
    Config {},
    #[serde(rename = "attachments.put")]
    AttachmentsPut {
        bytes: String,
    },
    #[serde(rename = "window.resize")]
    WindowResize {
        width: u32,
        height: u32,
    },
    Ready {},
    PageRecovered {},
    Failed {
        error: String,
    },
    PageError {
        kind: PageErrorKind,
        error: String,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum PageErrorKind {
    Operation,
    Application,
}
impl PageRequest {
    pub fn decode(input: &str) -> Result<Self> {
        if input.len() > super::SOCKET_ATTACHMENT {
            return Err(err(Code::TooLarge, "Page request is too large"));
        }
        let request: Self =
            serde_json::from_str(input).map_err(|_| err(Code::InvalidRequest, "Invalid page request"))?;
        request.check()?;
        Ok(request)
    }
    pub fn check(&self) -> Result<()> {
        if let Self::Apply { batch } = self {
            batch.check_size(super::PAGE_PAYLOAD)?;
        }
        let valid = match self {
            Self::CommandsRun { name, args } => {
                super::engine::valid_command_name(name) && crate::encode(args).len() <= super::PAGE_PAYLOAD
            }
            Self::Apply { .. } => true,
            Self::AttachmentsPut { bytes } => bytes.len() <= super::ATTACHMENT_FILE_BYTES.div_ceil(3) * 4,
            Self::WindowResize { width, height } => {
                (super::WINDOW_MIN_WIDTH..=super::WINDOW_MAX).contains(width)
                    && (super::WINDOW_MIN_HEIGHT..=super::WINDOW_MAX).contains(height)
            }
            Self::Failed { error } | Self::PageError { error, .. } => error.encode_utf16().count() <= super::ERROR_TEXT,
            _ => true,
        };
        if valid { Ok(()) } else { Err(err(Code::InvalidRequest, "Invalid page request")) }
    }
}

/// Host-only operations. These cross UniFFI; document payloads do not.
#[derive(Debug)]
pub enum HostAction {
    WindowResize { width: u32, height: u32 },
    Ready,
    PageRecovered,
    Failed { error: String },
    PageError { kind: PageErrorKind, error: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "method", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "page.generated.ts"))]
pub enum PageSuccess {
    #[serde(rename = "commands.run")]
    CommandsRun {
        ok: True,
        sequence: u64,
        ids: Vec<String>,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        result: Box<serde_json::value::RawValue>,
    },
    Open {
        ok: True,
        state: String,
    },
    Apply {
        ok: True,
        sequence: u64,
        ids: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        selection_start: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        selection_end: Option<usize>,
    },
    Flush {
        ok: True,
    },
    Undo {
        ok: True,
        sequence: u64,
    },
    Redo {
        ok: True,
        sequence: u64,
    },
    Config {
        ok: True,
        read_only: bool,
        #[serde(rename = "runtimeABI")]
        runtime_abi: u64,
        style: bool,
        window: super::build::WindowInput,
        #[cfg_attr(feature = "ts", ts(type = "object"))]
        descriptor: Box<serde_json::value::RawValue>,
    },
    #[serde(rename = "attachments.put")]
    AttachmentsPut {
        ok: True,
        id: String,
        byte_length: u64,
        mime_type: String,
    },
    #[serde(rename = "window.resize")]
    WindowResize {
        ok: True,
        width: f64,
        height: f64,
    },
    Ready {
        ok: True,
    },
    PageRecovered {
        ok: True,
    },
    Failed {
        ok: True,
    },
    PageError {
        ok: True,
    },
}
/// Native results use the same serializer as owner results.
pub enum HostReply {
    WindowResize { width: f64, height: f64 },
    Ready,
    PageRecovered,
    Failed,
    PageError,
}
impl HostReply {
    pub fn to_json(self) -> String {
        let ok = True;
        crate::encode(&match self {
            Self::WindowResize { width, height } if width.is_finite() && height.is_finite() => {
                PageSuccess::WindowResize { ok, width, height }
            }
            Self::WindowResize { .. } => {
                return crate::command::failure(
                    crate::owner::Failure::rejected(Code::InvalidRequest, "Invalid window dimensions"),
                    false,
                    false,
                );
            }
            Self::Ready => PageSuccess::Ready { ok },
            Self::PageRecovered => PageSuccess::PageRecovered { ok },
            Self::Failed => PageSuccess::Failed { ok },
            Self::PageError => PageSuccess::PageError { ok },
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum PagePush {
    Publication { publication: super::Publication },
    Resync,
}

/// Private host-to-shell calls. Authored apps only receive ctx.
#[derive(Debug, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum HostRequest {
    Publish {
        payload: String,
    },
    Flush,
    Undo,
    Redo,
    PrepareClose,
    CancelClose,
    Close,
    ReloadInterface,
    #[serde(rename = "capture.begin")]
    CaptureBegin {
        token: String,
        mode: CaptureMode,
    },
    #[serde(rename = "capture.settle")]
    CaptureSettle {
        token: String,
    },
    #[serde(rename = "capture.restore")]
    CaptureRestore {
        token: String,
    },
}
impl HostRequest {
    pub fn to_json(&self) -> String {
        crate::encode(self)
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum CaptureMode {
    Preview,
    Export,
    Icon,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct HostCaptureResult {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub dedicated: bool,
}
impl HostCaptureResult {
    pub fn decode(input: &str) -> Result<Self> {
        if input.len() > 4096 {
            return Err(err(Code::TooLarge, "Capture result is too large"));
        }
        let result: Self =
            serde_json::from_str(input).map_err(|_| err(Code::InvalidRequest, "Invalid capture result"))?;
        if [result.x, result.y, result.width, result.height].into_iter().all(f64::is_finite) {
            Ok(result)
        } else {
            Err(err(Code::InvalidRequest, "Invalid capture dimensions"))
        }
    }
}
