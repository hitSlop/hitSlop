//! The parent and restricted child share job decoding and resource budgets.
use super::*;

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
    pub fn decode(input: &'a [u8]) -> Result<Self, String> {
        if input.len() > INPUT {
            return Err("Evaluator input is too large".into());
        }
        serde_json::from_slice(input).map_err(|e| e.to_string())
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

/// Output is still untrusted: consumers validate the batch and encoded media.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderReply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RenderReply {
    pub fn into_images(self) -> Result<Vec<String>, String> {
        match (self.ok, self.images, self.error) {
            (true, Some(images), None) => Ok(images),
            (false, None, Some(error)) => Err(error),
            _ => Err("Invalid capture reply".into()),
        }
    }
}

/// The common refusal shape understood by every evaluator consumer. Success shapes
/// remain specific to the command ABI, definition evaluation or media renderer.
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
#[derive(Serialize)]
pub(crate) struct JobFailure {
    pub ok: bool,
    pub error: String,
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
    pub fn decode(json: &str) -> Result<Self, String> {
        if json.len() > MARKUP_BYTES {
            return Err("Capture markup exceeds 16 MiB".into());
        }
        let batch: Self = serde_json::from_str(json).map_err(|e| format!("Invalid capture markup: {e}"))?;
        batch.validate()?;
        Ok(batch)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.fonts.len() > 64 {
            return Err("Capture uses more than 64 font faces".into());
        }
        for font in &self.fonts {
            if font.family.is_empty()
                || font.family.len() > 256
                || font.weight.is_some_and(|w| !w.is_finite() || !(1.0..=1000.0).contains(&w))
            {
                return Err("Invalid packaged font metadata".into());
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
