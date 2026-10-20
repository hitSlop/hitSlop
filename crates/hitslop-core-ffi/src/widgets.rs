use hitslop_core::capture::{CaptureFormat, CaptureOutput, CaptureValue};
use hitslop_core::widgets::{WidgetFamily, WidgetLimits, WidgetViewport};

#[uniffi::remote(Enum)]
pub enum CaptureFormat {
    Png,
    Pdf,
}

#[uniffi::remote(Enum)]
pub enum CaptureOutput {
    Preview,
    Icon,
    Export { format: CaptureFormat },
    Widgets,
}

#[derive(uniffi::Enum)]
pub enum RenderedOutput {
    Image { bytes: Option<Vec<u8>> },
    Widgets { capture: RenderedWidgets },
    Failed { failure: hitslop_core::owner::Failure },
}

#[derive(uniffi::Record)]
pub struct CaptureResult {
    pub output: CaptureOutput,
    pub result: RenderedOutput,
}

#[derive(uniffi::Record)]
pub struct CapturedArtwork {
    pub source_version: String,
    pub preview: Option<Vec<u8>>,
    pub icon: Option<Vec<u8>>,
}

/// Shared executable configuration, not a shared interpreter. Each output runs in a
/// fresh restricted child and widget freshness still checks the installed binary.
#[derive(uniffi::Object)]
pub struct CaptureRuntime {
    evaluator: hitslop_core::owner::Evaluator,
}

#[uniffi::export]
impl CaptureRuntime {
    #[uniffi::constructor]
    pub fn new(evaluator_path: String) -> Result<std::sync::Arc<Self>, crate::CoreError> {
        let evaluator = hitslop_core::owner::Evaluator::new(evaluator_path.into(), vec![])
            .map_err(hitslop_core::store::Error::Failed)?;
        Ok(std::sync::Arc::new(Self { evaluator }))
    }

    pub fn render_saved(
        &self,
        path: String,
        outputs: Vec<CaptureOutput>,
    ) -> Result<Vec<CaptureResult>, crate::CoreError> {
        #[cfg(target_os = "ios")]
        if outputs.iter().any(|output| matches!(output, CaptureOutput::Export { .. })) {
            return Err(hitslop_core::store::Error::Failed("Export is unavailable on this host".into()).into());
        }
        Ok(hitslop_core::capture::render_batch(std::path::Path::new(&path), &self.evaluator, &outputs)?
            .into_iter()
            .map(|capture| CaptureResult {
                output: capture.output,
                result: match capture.result {
                    Ok(CaptureValue::Image { bytes }) => RenderedOutput::Image { bytes },
                    Ok(CaptureValue::Widgets { capture }) => RenderedOutput::Widgets { capture: capture.into() },
                    Err(error) => RenderedOutput::Failed { failure: error.into() },
                },
            })
            .collect())
    }

    /// A complete pair from one immutable source; a partial failure preserves old artwork.
    pub fn render_artwork(&self, path: String) -> Result<CapturedArtwork, crate::CoreError> {
        let path = std::path::Path::new(&path);
        let store = hitslop_core::store::Store::open(path, hitslop_core::store::Mode::Snapshot)?;
        let source_version = store.document()?.reading()?.version;
        drop(store);
        let mut result = CapturedArtwork { source_version, preview: None, icon: None };
        for capture in
            hitslop_core::capture::render_batch(path, &self.evaluator, &[CaptureOutput::Preview, CaptureOutput::Icon])?
        {
            if let CaptureValue::Image { bytes } = capture.result? {
                match capture.output {
                    CaptureOutput::Preview => result.preview = bytes,
                    CaptureOutput::Icon => result.icon = bytes,
                    _ => unreachable!(),
                }
            }
        }
        Ok(result)
    }

    pub fn saved_widget_version(&self, path: String) -> Result<String, crate::CoreError> {
        Ok(hitslop_core::widgets::saved_version(std::path::Path::new(&path), &self.evaluator)?)
    }
}

#[uniffi::remote(Record)]
pub struct WidgetLimits {
    pub min_refresh_minutes: u32,
    pub max_refresh_minutes: u32,
    pub scale: u32,
}
#[uniffi::export]
pub fn widget_limits() -> WidgetLimits {
    hitslop_core::widgets::widget_limits()
}

#[uniffi::remote(Enum)]
pub enum WidgetFamily {
    Small,
    Medium,
    Large,
}
#[uniffi::remote(Record)]
pub struct WidgetViewport {
    pub family: WidgetFamily,
    pub width: u32,
    pub height: u32,
}
#[uniffi::export]
pub fn widget_viewports() -> Vec<WidgetViewport> {
    hitslop_core::widgets::widget_viewports()
}

#[derive(uniffi::Record)]
pub struct RenderedWidgetImage {
    pub family: WidgetFamily,
    pub png: Vec<u8>,
}
#[derive(uniffi::Record)]
pub struct RenderedWidgets {
    pub source_version: String,
    pub images: Vec<RenderedWidgetImage>,
}
impl From<hitslop_core::widgets::WidgetCapture> for RenderedWidgets {
    fn from(capture: hitslop_core::widgets::WidgetCapture) -> Self {
        Self {
            source_version: capture.source_version,
            images: capture
                .images
                .into_iter()
                .map(|image| RenderedWidgetImage { family: image.family, png: image.png })
                .collect(),
        }
    }
}
