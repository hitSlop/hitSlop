use crate::wire::engine::ExportFormat;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum ViewCaptureKind {
    Icon,
    Preview,
    Export,
}

/// Outputs requested together use the same saved document and accepted app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureOutput {
    Preview,
    Icon,
    Export { format: ExportFormat },
    Widgets,
}

#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct ViewRenderRequest<'a> {
    pub kind: ViewCaptureKind,
    pub format: ExportFormat,
    pub attachments: &'a std::collections::BTreeMap<String, String>,
    pub width: u32,
    pub height: u32,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub snapshot: &'a serde_json::Value,
    pub theme: &'a std::collections::BTreeMap<String, String>,
}
