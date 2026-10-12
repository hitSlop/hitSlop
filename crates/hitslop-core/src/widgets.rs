//! Disposable widget presentation contracts. These contain no document state.
use serde::{Deserialize, Serialize};

pub const MIN_REFRESH_MINUTES: u32 = 15;
pub const MAX_REFRESH_MINUTES: u32 = 1440;
#[derive(Clone, Copy, Debug)]
pub struct WidgetLimits {
    pub min_refresh_minutes: u32,
    pub max_refresh_minutes: u32,
    pub scale: u32,
}
pub fn widget_limits() -> WidgetLimits {
    WidgetLimits { min_refresh_minutes: MIN_REFRESH_MINUTES, max_refresh_minutes: MAX_REFRESH_MINUTES, scale: 2 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum WidgetFamily {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct WidgetViewport {
    pub family: WidgetFamily,
    pub width: u32,
    pub height: u32,
}

/// macOS capture canvases. WidgetKit scales the image for the actual placement.
pub fn widget_viewports() -> Vec<WidgetViewport> {
    [(WidgetFamily::Small, 164, 164), (WidgetFamily::Medium, 344, 164), (WidgetFamily::Large, 344, 344)]
        .map(|(family, width, height)| WidgetViewport { family, width, height })
        .to_vec()
}
