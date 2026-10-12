use hitslop_core::capture::ViewCaptureKind;
use hitslop_core::widgets::{WidgetFamily, WidgetLimits, WidgetViewport};

#[uniffi::remote(Enum)]
pub enum ViewCaptureKind {
    Icon,
    Preview,
    Export,
}

#[uniffi::export]
pub fn render_saved_view(
    path: String,
    evaluator_path: String,
    kind: ViewCaptureKind,
) -> Result<Option<Vec<u8>>, crate::CoreError> {
    let evaluator = hitslop_core::owner::Evaluator::new(evaluator_path.into(), vec![])
        .map_err(hitslop_core::store::Error::Failed)?;
    Ok(hitslop_core::capture::render_saved(std::path::Path::new(&path), &evaluator, kind)?)
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
#[uniffi::export]
pub fn saved_widget_version(path: String, evaluator_path: String) -> Result<String, crate::CoreError> {
    let evaluator = hitslop_core::owner::Evaluator::new(evaluator_path.into(), vec![])
        .map_err(hitslop_core::store::Error::Failed)?;
    Ok(hitslop_core::widgets::saved_version(std::path::Path::new(&path), &evaluator)?)
}
#[uniffi::export]
pub fn render_saved_icon(path: String, evaluator_path: String) -> Result<Option<Vec<u8>>, crate::CoreError> {
    let evaluator = hitslop_core::owner::Evaluator::new(evaluator_path.into(), vec![])
        .map_err(hitslop_core::store::Error::Failed)?;
    Ok(hitslop_core::capture::icon(std::path::Path::new(&path), &evaluator)?)
}
#[uniffi::export]
pub fn render_saved_widgets(path: String, evaluator_path: String) -> Result<RenderedWidgets, crate::CoreError> {
    let evaluator = hitslop_core::owner::Evaluator::new(evaluator_path.into(), vec![])
        .map_err(hitslop_core::store::Error::Failed)?;
    let capture = hitslop_core::widgets::render_saved(std::path::Path::new(&path), &evaluator)?;
    Ok(RenderedWidgets {
        source_version: capture.source_version,
        images: capture
            .images
            .into_iter()
            .map(|image| RenderedWidgetImage { family: image.family, png: image.png })
            .collect(),
    })
}
