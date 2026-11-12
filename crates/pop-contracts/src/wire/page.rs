//! Same-build page messages. The owner parses once; native UI receives typed actions.
use super::engine::True;
use crate::{Code, Result, app::Capability, err};
use serde::{Deserialize, Serialize};

/// Responsive editor layout, in CSS pixels of the actual document viewport.
pub const COMPACT_LAYOUT_WIDTH: u32 = 640;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum HostPlatform {
    Macos,
    Ios,
    Android,
    Windows,
    Linux,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum HostDevice {
    Phone,
    Tablet,
    Desktop,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum HostLayout {
    Compact,
    Regular,
}
/// Presentation hints, never document state or a capability/permission grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct HostEnvironment {
    pub platform: HostPlatform,
    pub device: HostDevice,
}
impl HostEnvironment {
    /// Development owners cannot infer the page's device from their process.
    pub fn unknown() -> Self {
        Self { platform: HostPlatform::Unknown, device: HostDevice::Unknown }
    }
}

/// Discrete, best-effort feedback; never document state or a permission grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum HapticFeedback {
    Selection {},
    Impact { style: HapticImpactStyle },
    Notification { notification: HapticNotification },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum HapticImpactStyle {
    Light,
    Medium,
    Heavy,
    Soft,
    Rigid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum HapticNotification {
    Success,
    Warning,
    Error,
}

// Host request, action, reply and success variants share one operation registry.
macro_rules! page_operations {
    (
        requests { $($request_extra:tt)* }
        successes { $($success_extra:tt)* }
        operations {
            $( $request:ident => $action:ident
                $( { $( $(#[$field_meta:meta])* $field:ident: $ty:ty ),* $(,)? } )?
                ; $method:literal
                => $reply:ident $( { $( $result:ident: $result_ty:ty ),* $(,)? } )? ;
            )*
        }
    ) => {
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(tag = "method", deny_unknown_fields, rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
        pub enum PageRequest {
            $($request_extra)*
            $(
                #[serde(rename = $method)]
                $request { $( $( $(#[$field_meta])* $field: $ty, )* )? },
            )*
        }

        /// Host-only operations. These cross UniFFI; document payloads do not.
        #[derive(Debug)]
        pub enum HostAction {
            $( $action $( { $( $field: $ty, )* } )?, )*
        }

        #[derive(Debug, Serialize)]
        #[serde(tag = "method", rename_all = "camelCase", rename_all_fields = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "page.generated.ts"))]
        pub enum PageSuccess {
            $($success_extra)*
            $(
                #[serde(rename = $method)]
                $request { ok: True, $( $( $result: $result_ty, )* )? },
            )*
        }

        /// Native results use the same serializer as owner results.
        pub enum HostReply {
            $( $reply $( { $( $result: $result_ty, )* } )?, )*
        }

        impl PageRequest {
            /// Separate a host action after admission and capability checks.
            pub fn into_host(self) -> std::result::Result<HostAction, Self> {
                match self {
                    $( Self::$request { $( $( $field, )* )? } =>
                        Ok(HostAction::$action $( { $( $field, )* } )?), )*
                    request => Err(request),
                }
            }
        }
        impl HostReply {
            fn success(self) -> PageSuccess {
                match self {
                    $( Self::$reply $( { $( $result, )* } )? =>
                        PageSuccess::$request { ok: True, $( $( $result, )* )? }, )*
                }
            }
        }
    };
}

page_operations! {
    requests {
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

    }
    successes {
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

    }
    operations {
        HostEnvironment => Environment; "host.environment"
            => Environment { environment: HostEnvironment };
        HapticsPlay => HapticsPlay { feedback: HapticFeedback }; "haptics.play" => HapticsPlay;
        WindowResize => WindowResize { width: u32, height: u32 }; "window.resize"
            => WindowResize { width: f64, height: f64 };
        SpeechTranscribe => SpeechTranscribe {
            attachment: String,
            #[serde(default)]
            #[cfg_attr(feature = "ts", ts(optional))]
            locale: Option<String>,
        }; "speech.transcribe" => SpeechTranscribe { outcome: SpeechOutcome };
        Ready => Ready; "ready" => Ready;
        PageRecovered => PageRecovered; "pageRecovered" => PageRecovered;
        Failed => Failed { error: String }; "failed" => Failed;
        PageError => PageError { kind: PageErrorKind, error: String }; "pageError" => PageError;
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum PageErrorKind {
    Operation,
    Application,
}
impl PageRequest {
    pub fn requires(&self) -> Option<Capability> {
        match self {
            Self::SpeechTranscribe { .. } => Some(Capability::Speech),
            _ => None,
        }
    }
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
/// speech service (`pop dev`); the OS cannot
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
impl HostReply {
    pub fn to_json(self) -> String {
        let reply = match self {
            Self::WindowResize { width, height } if !width.is_finite() || !height.is_finite() => {
                return crate::encode(&super::engine::Failure {
                    ok: super::engine::False,
                    error: "Invalid window dimensions".into(),
                    code: super::OutcomeCode::Rejected,
                    reason: Some(Code::InvalidRequest),
                    op_index: None,
                });
            }
            Self::SpeechTranscribe { outcome } => Self::SpeechTranscribe { outcome: outcome.bounded() },
            reply => reply,
        };
        crate::encode(&reply.success())
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
    pub fn publication_json(publication: String) -> String {
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
    Publish { payload: String },
    Flush,
    Undo,
    Redo,
    PrepareClose,
    CancelClose,
    Close,
    ReloadInterface,
}
impl HostRequest {
    pub fn to_json(&self) -> String {
        crate::encode(self)
    }
}

/// Page-local text with an uncertain outcome. Recovery never applies it to the owner.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct UnresolvedDraft {
    pub path: Vec<super::Segment>,
    pub text: String,
}
impl UnresolvedDraft {
    pub fn field_label(&self) -> String {
        crate::encode(&self.path)
    }
}

/// The private close barrier's reply; full drafts are separate from bounded diagnostics.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct ClosePreparation {
    pub unresolved_drafts: Vec<UnresolvedDraft>,
}
impl ClosePreparation {
    pub fn decode(input: &str) -> Result<Self> {
        // This is a recovery copy of already-retained page memory, not an edit or a saved
        // document. Applying the edit/diagnostic budget would truncate or strand drafts.
        serde_json::from_str(input).map_err(|_| err(Code::InvalidRequest, "Invalid close preparation reply"))
    }
}
