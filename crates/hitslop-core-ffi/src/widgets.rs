use hitslop_core::capture::{CaptureFormat, CaptureOutput, CaptureProvider, CaptureValue};
use hitslop_core::widgets::{WidgetFamily, WidgetLimits, WidgetViewport};

#[uniffi::remote(Enum)]
pub enum CaptureFormat {
    Png,
    Pdf,
}
#[uniffi::remote(Enum)]
pub enum CaptureProvider {
    Takumi,
    WebKit,
}
#[uniffi::export]
pub fn select_capture_provider(
    output: CaptureOutput,
    requested: Option<CaptureProvider>,
) -> Result<CaptureProvider, crate::CoreError> {
    Ok(hitslop_core::capture::select_provider(output, requested).map_err(hitslop_core::store::Error::Rejected)?)
}
#[uniffi::export]
pub fn capture_outputs(provider: CaptureProvider) -> Vec<CaptureOutput> {
    hitslop_core::capture::capture_outputs(provider)
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
