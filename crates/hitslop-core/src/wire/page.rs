//! Same-build page messages. The owner parses once; native UI receives typed actions.
use super::engine::True;
use crate::{Code, Result, err};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum PageRequest {
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
    /// Transcribes a recording the page already stored as an attachment, by its ID, so the
    /// recording never travels through the page request again. Absent `locale` means the
    /// person's own.
    #[serde(rename = "speech.transcribe")]
    SpeechTranscribe {
        attachment: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        locale: Option<String>,
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
            Self::Apply { .. } => true,
            Self::AttachmentsPut { bytes } => bytes.len() <= super::ATTACHMENT_FILE_BYTES.div_ceil(3) * 4,
            Self::WindowResize { width, height } => {
                (super::WINDOW_MIN_WIDTH..=super::WINDOW_MAX).contains(width)
                    && (super::WINDOW_MIN_HEIGHT..=super::WINDOW_MAX).contains(height)
            }
            Self::Failed { error } | Self::PageError { error, .. } => error.encode_utf16().count() <= super::ERROR_TEXT,
            Self::SpeechTranscribe { attachment, locale } => {
                super::valid_attachment_id(attachment) && locale.as_deref().is_none_or(super::valid_speech_locale)
            }
            _ => true,
        };
        if valid { Ok(()) } else { Err(err(Code::InvalidRequest, "Invalid page request")) }
    }
}

/// Host-only operations. These cross UniFFI; document payloads do not.
#[derive(Debug)]
pub enum HostAction {
    WindowResize {
        width: u32,
        height: u32,
    },
    /// The app declared `speech` and this page may use it. The host reads the recording
    /// through the owner's resource reader, which verifies it before the provider decodes it.
    SpeechTranscribe {
        attachment: String,
        locale: Option<String>,
    },
    Ready,
    PageRecovered,
    Failed {
        error: String,
    },
    PageError {
        kind: PageErrorKind,
        error: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "method", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "page.generated.ts"))]
pub enum PageSuccess {
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
        window: crate::app::WindowDefinition,
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
    #[serde(rename = "speech.transcribe")]
    SpeechTranscribe {
        ok: True,
        outcome: SpeechOutcome,
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
/// What a transcription came to. Everything but `transcribed` reaches the author as a
/// `SpeechError` with the same `code`; the page request itself still succeeded.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum SpeechOutcome {
    /// `locale` is the one the recognizer used.
    Transcribed {
        text: String,
        locale: String,
    },
    Unavailable {
        reason: SpeechUnavailable,
        message: String,
    },
    /// The page was replaced, closed or lost its renderer before the transcript was ready.
    Cancelled {
        message: String,
    },
    Failed {
        message: String,
    },
}
/// Why this host cannot transcribe: the app did not declare `speech`; this host has no
/// speech service (`slop dev`, a browser copy, a background renderer); the OS cannot
/// (too old, or recognition is off); this locale has no on-device recognizer, and the
/// host never sends a recording off the device instead; the person declined; or the
/// provider cannot decode this recording's format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum SpeechUnavailable {
    NotDeclared,
    Host,
    System,
    Locale,
    Permission,
    Format,
}
impl SpeechOutcome {
    /// The answer of every host without a speech service.
    pub fn unsupported_host() -> Self {
        Self::Unavailable { reason: SpeechUnavailable::Host, message: "This host cannot transcribe speech".into() }
    }
    /// The provider's outcome as the page may receive it: messages shortened to the
    /// diagnostic limit, and a transcript or locale beyond its limit refused.
    fn bounded(self) -> Self {
        let message = |text: String| -> String {
            let mut units = 0;
            text.chars()
                .take_while(|c| {
                    units += c.len_utf16();
                    units <= super::ERROR_TEXT
                })
                .collect()
        };
        match self {
            Self::Transcribed { text, .. } if text.len() > super::SPEECH_TEXT_BYTES => Self::Failed {
                message: format!("The transcript is longer than {} MiB", super::SPEECH_TEXT_BYTES >> 20),
            },
            Self::Transcribed { locale, .. } if !super::valid_speech_locale(&locale) => {
                Self::Failed { message: "The recognizer named an invalid locale".into() }
            }
            Self::Transcribed { .. } => self,
            Self::Unavailable { reason, message: text } => Self::Unavailable { reason, message: message(text) },
            Self::Cancelled { message: text } => Self::Cancelled { message: message(text) },
            Self::Failed { message: text } => Self::Failed { message: message(text) },
        }
    }
}
/// Native results use the same serializer as owner results.
pub enum HostReply {
    WindowResize { width: f64, height: f64 },
    SpeechTranscribe { outcome: SpeechOutcome },
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
            Self::SpeechTranscribe { outcome } => PageSuccess::SpeechTranscribe { ok, outcome: outcome.bounded() },
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
#[cfg_attr(feature = "ts", ts(concrete(P = super::Publication)))]
pub enum PagePush<P = super::Publication> {
    Publication { publication: P },
    Resync,
}

impl PagePush {
    /// Wraps a publication the core already encoded. Every accepted edit passes through
    /// here, so the payload (up to `PAGE_PAYLOAD`) is not scanned again as JSON.
    pub(crate) fn publication_json(publication: String) -> String {
        let json = format!(r#"{{"type":"publication","publication":{publication}}}"#);
        debug_assert_eq!(
            json,
            crate::encode(&PagePush::Publication {
                publication: serde_json::value::RawValue::from_string(publication).expect("core publication"),
            })
        );
        json
    }
    pub fn resync_json() -> String {
        crate::encode(&Self::Resync)
    }
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
    #[serde(rename = "capture.widget")]
    CaptureWidget {
        token: String,
        viewport: crate::widgets::WidgetViewport,
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
