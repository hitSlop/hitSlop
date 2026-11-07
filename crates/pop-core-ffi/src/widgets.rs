use pop_capture::capture::{CaptureFormat, CaptureOutput, CaptureValue};
use pop_capture::widgets::{WidgetFamily, WidgetLimits, WidgetViewport};

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
    Failed { failure: pop_runtime::owner::Failure },
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

/// Explicit platform capture policy. Desktop outputs use fresh restricted children;
/// iOS capture runs in process. Widget freshness checks the installed binary on both.
#[derive(uniffi::Object)]
pub struct CaptureRuntime {
    evaluator: pop_runtime::owner::Evaluator,
}

#[uniffi::export]
impl CaptureRuntime {
    #[uniffi::constructor]
    pub fn new(evaluator_path: String) -> Result<std::sync::Arc<Self>, crate::CoreError> {
        #[cfg(target_os = "ios")]
        let evaluator = pop_runtime::owner::Evaluator::in_process_capture(evaluator_path.into());
        #[cfg(not(target_os = "ios"))]
        let evaluator = pop_runtime::owner::Evaluator::process(evaluator_path.into(), vec![]);
        let evaluator = evaluator.map_err(pop_storage::store::Error::Failed)?;
        Ok(std::sync::Arc::new(Self { evaluator }))
    }

    pub fn render_saved(
        &self,
        path: String,
        outputs: Vec<CaptureOutput>,
    ) -> Result<Vec<CaptureResult>, crate::CoreError> {
        #[cfg(target_os = "ios")]
        if outputs.iter().any(|output| matches!(output, CaptureOutput::Export { .. })) {
            return Err(pop_storage::store::Error::Failed("Export is unavailable on this host".into()).into());
        }
        Ok(pop_capture::capture::render_batch(std::path::Path::new(&path), &self.evaluator, &outputs)?
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
        let store = pop_storage::store::Store::open(path, pop_storage::store::Mode::Snapshot)?;
        let source_version = store.document()?.reading()?.version;
        drop(store);
        let mut result = CapturedArtwork { source_version, preview: None, icon: None };
        for capture in
            pop_capture::capture::render_batch(path, &self.evaluator, &[CaptureOutput::Preview, CaptureOutput::Icon])?
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
        Ok(pop_capture::widgets::saved_version(std::path::Path::new(&path), &self.evaluator)?)
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
    pop_capture::widgets::widget_limits()
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
    pop_capture::widgets::widget_viewports()
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
impl From<pop_capture::widgets::WidgetCapture> for RenderedWidgets {
    fn from(capture: pop_capture::widgets::WidgetCapture) -> Self {
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
