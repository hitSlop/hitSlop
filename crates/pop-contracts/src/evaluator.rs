//! The parent and restricted child share job decoding and resource budgets.
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "runner.generated.ts"))]
pub enum JobKind {
    #[default]
    Command,
    Definition,
    Markup,
    WidgetMarkup,
    Widget,
    Capture,
    Pdf,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "runner.generated.ts"))]
pub struct Job<'a> {
    #[serde(rename = "runtimeABI")]
    pub runtime_abi: u64,
    #[serde(borrow)]
    pub bundle: Cow<'a, str>,
    #[serde(borrow)]
    pub request: Cow<'a, str>,
    #[serde(default)]
    pub mode: JobKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub raster: Option<RasterRequest>,
}

impl<'a> Job<'a> {
    pub fn decode(input: &'a [u8]) -> Result<Self, crate::Error> {
        if input.len() > INPUT {
            return Err(invalid("Evaluator input is too large"));
        }
        serde_json::from_slice(input).map_err(invalid)
    }
}

pub struct JobLimits {
    pub output: usize,
    pub watchdog_ms: u64,
    pub cpu_seconds: u64,
    pub execution_ms: u64,
    pub resident_bytes: Option<u64>,
}

impl JobKind {
    pub fn timeout_message(self) -> &'static str {
        match self {
            Self::Command | Self::Definition => "Command execution timed out",
            Self::Markup | Self::WidgetMarkup | Self::Widget | Self::Capture | Self::Pdf => {
                "Capture rendering timed out"
            }
        }
    }
    pub fn limits(self) -> JobLimits {
        let capture = matches!(self, Self::Markup | Self::WidgetMarkup | Self::Widget | Self::Capture | Self::Pdf);
        JobLimits {
            output: match self {
                Self::Command | Self::Definition => OUTPUT,
                Self::Markup | Self::WidgetMarkup => MARKUP_BYTES,
                Self::Widget => WIDGET_OUTPUT,
                Self::Capture => CAPTURE_OUTPUT,
                Self::Pdf => PDF_OUTPUT,
            },
            watchdog_ms: if matches!(self, Self::Pdf) { PDF_WATCHDOG_MS } else { WATCHDOG_MS },
            cpu_seconds: if matches!(self, Self::Pdf) { PDF_CPU_SECONDS } else { 3 },
            execution_ms: EXECUTION_MS,
            resident_bytes: capture.then_some(512 << 20),
        }
    }
}

/// A shape-checked renderer outcome. Consumers still validate the encoded media.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum RenderReply {
    Images { ok: crate::wire::engine::True, images: Vec<String> },
    Failure { ok: crate::wire::engine::False, error: String },
}
impl RenderReply {
    pub fn into_images(self) -> Result<Vec<String>, crate::Error> {
        match self {
            Self::Images { images, .. } => Ok(images),
            Self::Failure { error, .. } => Err(invalid(error)),
        }
    }
}

/// Stored programs may add ignorable hints to these replies.
#[derive(Clone, Serialize, Deserialize)]
pub struct Frame {
    pub html: String,
    pub css: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Font {
    pub family: String,
    pub bytes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
    pub italic: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Batch {
    pub frames: Vec<Frame>,
    pub fonts: Vec<Font>,
}
impl Batch {
    pub fn decode(json: &str) -> Result<Self, crate::Error> {
        if json.len() > MARKUP_BYTES {
            return Err(invalid("Capture markup exceeds 16 MiB"));
        }
        let batch: Self = serde_json::from_str(json).map_err(|e| invalid(format!("Invalid capture markup: {e}")))?;
        batch.validate()?;
        Ok(batch)
    }
    pub fn validate(&self) -> Result<(), crate::Error> {
        if self.fonts.len() > 64 {
            return Err(invalid("Capture uses more than 64 font faces"));
        }
        for font in &self.fonts {
            if font.family.is_empty()
                || font.family.len() > 256
                || font.weight.is_some_and(|w| !w.is_finite() || !(1.0..=1000.0).contains(&w))
            {
                return Err(invalid("Invalid packaged font metadata"));
            }
        }
        Ok(())
    }
}
/// Same-build raster input is exact, carries no executable program, and serializes
/// markup only once so 16 MiB markup + 32 MiB encoded images fits INPUT.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterRequest {
    pub batch: Batch,
    pub images: std::collections::BTreeMap<String, String>,
}

pub const MARKUP_BYTES: usize = 16 << 20;
pub const IMAGE_BYTES: usize = 24 << 20;
pub const WIDGET_OUTPUT: usize = 8 << 20;
pub const CAPTURE_OUTPUT: usize = 36 << 20;
pub const PDF_OUTPUT: usize = 36 << 20;
pub const PDF_BYTES: usize = 24 << 20;
pub const PDF_WATCHDOG_MS: u64 = 10_000;
pub const PDF_CPU_SECONDS: u64 = 8;
pub const INPUT: usize = 64 << 20;
pub const OUTPUT: usize = 4 << 20;
pub const WATCHDOG_MS: u64 = 3000;
pub const EXECUTION_MS: u64 = 2000;
pub use crate::wire::RUNTIME_ABI;
/// The bounded envelope the command drivers send to the evaluator.
pub fn command_input(runtime_abi: u64, bundle: &str, request: &str) -> Result<String, crate::Error> {
    let input = serde_json::to_string(&Job {
        runtime_abi,
        bundle: Cow::Borrowed(bundle),
        request: Cow::Borrowed(request),
        mode: JobKind::Command,
        raster: None,
    })
    .map_err(invalid)?;
    if input.len() > INPUT {
        return Err(invalid("Command input is too large"));
    }
    Ok(input)
}

fn invalid(message: impl ToString) -> crate::Error {
    crate::Error::new(crate::wire::Code::InvalidRequest, message)
}
