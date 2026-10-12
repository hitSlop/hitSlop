//! Disposable widget presentation contracts. These contain no document state.
use serde::{Deserialize, Serialize};

#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
mod fingerprint;

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

/// One immutable saved snapshot, never a second document engine.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct WidgetRenderRequest {
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub snapshot: serde_json::Value,
    pub theme: std::collections::BTreeMap<String, String>,
    pub viewports: Vec<WidgetViewport>,
}

pub struct WidgetImage {
    pub family: WidgetFamily,
    pub png: Vec<u8>,
}

pub struct WidgetCapture {
    pub source_version: String,
    pub images: Vec<WidgetImage>,
}

#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
fn source_version(document: &crate::Document, renderer: &fingerprint::Fingerprint) -> String {
    use sha2::{Digest, Sha256};
    // An opaque presentation token, not a document format version. Include the
    // installed renderer/fonts and canvases so app updates refresh unchanged state.
    // Artwork writes and compaction still do not change the saved semantic version.
    let mut hash = Sha256::new();
    hash.update(b"hitslop-widget-capture-v1\0");
    hash.update(renderer.digest);
    hash.update(widget_limits().scale.to_le_bytes());
    hash.update(crate::encode(&widget_viewports()).as_bytes());
    hash.update(document.version().as_bytes());
    data_encoding::HEXLOWER.encode(&hash.finalize())
}

#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn saved_version(path: &std::path::Path, evaluator: &hitslop_runner::Evaluator) -> crate::store::Result<String> {
    let renderer = fingerprint::renderer(evaluator.executable_path()).map_err(crate::error::failed)?;
    let store = crate::store::Store::open(path, crate::store::Mode::Snapshot)?;
    let version = source_version(&store.document()?, &renderer);
    renderer.check(evaluator.executable_path()).map_err(crate::error::failed)?;
    Ok(version)
}

/// The caller acquires a saved source through the owner before calling this function.
/// Only immutable state and the private program leave the core, into a restricted child.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn render_saved(
    path: &std::path::Path,
    evaluator: &hitslop_runner::Evaluator,
) -> crate::store::Result<WidgetCapture> {
    use crate::store::{Mode, Store};
    let invalid = |message: String| crate::store::Error::Rejected(crate::err(crate::Code::InvalidRequest, message));
    let renderer = fingerprint::renderer(evaluator.executable_path()).map_err(crate::error::failed)?;
    let store = Store::open(path, Mode::Snapshot)?;
    if store.app().app.views().widget.is_none() {
        return Err(invalid("This slop has no Widget.svelte".into()));
    }
    let program = store
        .app()
        .widget
        .as_deref()
        .ok_or_else(|| invalid("Rebuild this pre-launch slop to include its widget render program".into()))?;
    let document = store.document()?;
    let source_version = source_version(&document, &renderer);
    let request = WidgetRenderRequest {
        snapshot: serde_json::from_str(&document.value()).map_err(|e| invalid(e.to_string()))?,
        theme: document.theme_state().map_err(crate::store::Error::Rejected)?.effective.into_iter().collect(),
        viewports: widget_viewports(),
    };
    let result = evaluator.widget(store.app().runtime_abi, program, &crate::encode(&request)).map_err(invalid)?;
    #[derive(Deserialize)]
    struct Reply {
        ok: bool,
        images: Option<Vec<String>>,
        error: Option<String>,
    }
    let reply: Reply = serde_json::from_str(&result).map_err(|e| invalid(format!("Invalid widget reply: {e}")))?;
    if !reply.ok {
        return Err(invalid(reply.error.unwrap_or_else(|| "Widget renderer failed".into())));
    }
    let images = reply.images.ok_or_else(|| invalid("Widget renderer returned no images".into()))?;
    if images.len() != request.viewports.len() {
        return Err(invalid("Widget renderer returned an incomplete batch".into()));
    }
    let images = images
        .into_iter()
        .zip(request.viewports)
        .map(|(image, viewport)| {
            let png = data_encoding::BASE64
                .decode(image.as_bytes())
                .map_err(|_| invalid("Invalid widget PNG encoding".into()))?;
            // The sandbox's output is untrusted. The cache independently validates PNGs too.
            let checked =
                crate::images::check(&png, crate::images::Purpose::Preview).map_err(crate::store::Error::Rejected)?;
            if checked.width != viewport.width * 2 || checked.height != viewport.height * 2 {
                return Err(invalid("Widget PNG does not match its viewport".into()));
            }
            Ok(WidgetImage { family: viewport.family, png })
        })
        .collect::<crate::store::Result<_>>()?;
    renderer.check(evaluator.executable_path()).map_err(crate::error::failed)?;
    Ok(WidgetCapture { source_version, images })
}
