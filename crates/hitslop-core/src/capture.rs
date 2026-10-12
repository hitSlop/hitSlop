//! Saved-source artwork rendered in the restricted child, without a page or writer.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum ViewCaptureKind {
    Icon,
    Preview,
    Export,
}

#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct ViewRenderRequest {
    pub kind: ViewCaptureKind,
    pub width: u32,
    pub height: u32,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub snapshot: serde_json::Value,
    pub theme: std::collections::BTreeMap<String, String>,
}

/// None means the embedded app has no native icon program; its existing page
/// renderer remains available. A declared native program's failure is not hidden.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn icon(path: &std::path::Path, evaluator: &hitslop_runner::Evaluator) -> crate::store::Result<Option<Vec<u8>>> {
    render_saved(path, evaluator, ViewCaptureKind::Icon)
}

#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn render_saved(
    path: &std::path::Path,
    evaluator: &hitslop_runner::Evaluator,
    kind: ViewCaptureKind,
) -> crate::store::Result<Option<Vec<u8>>> {
    use crate::store::{Mode, Store};
    let invalid = |message: String| crate::store::Error::Rejected(crate::err(crate::Code::InvalidRequest, message));
    let store = Store::open(path, Mode::Snapshot)?;
    let icon = kind == ViewCaptureKind::Icon;
    let Some(program) = (if icon { &store.app().icon } else { &store.app().export }).as_deref() else {
        return Ok(None);
    };
    let document = store.document()?;
    let request = ViewRenderRequest {
        kind,
        width: if icon { 512 } else { store.app().app.window().width },
        height: if icon { 512 } else { store.app().app.window().height },
        snapshot: serde_json::from_str(&document.value()).map_err(|e| invalid(e.to_string()))?,
        theme: document.theme_state().map_err(crate::store::Error::Rejected)?.effective.into_iter().collect(),
    };
    let result = evaluator.capture(store.app().runtime_abi, program, &crate::encode(&request)).map_err(invalid)?;
    #[derive(serde::Deserialize)]
    struct Reply {
        ok: bool,
        images: Option<Vec<String>>,
        error: Option<String>,
    }
    let reply: Reply = serde_json::from_str(&result).map_err(|e| invalid(format!("Invalid capture reply: {e}")))?;
    if !reply.ok {
        return Err(invalid(reply.error.unwrap_or_else(|| "Capture renderer failed".into())));
    }
    let mut images = reply.images.ok_or_else(|| invalid("Capture renderer returned no image".into()))?;
    if images.len() != 1 {
        return Err(invalid("Capture renderer returned an invalid batch".into()));
    }
    let png = data_encoding::BASE64
        .decode(images.pop().unwrap().as_bytes())
        .map_err(|_| invalid("Invalid capture PNG encoding".into()))?;
    crate::images::check(&png, if icon { crate::images::Purpose::Icon } else { crate::images::Purpose::Preview })
        .map_err(crate::store::Error::Rejected)?;
    Ok(Some(png))
}
