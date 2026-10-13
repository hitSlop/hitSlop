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

/// Capture choices are host policy, independent of a document's persisted format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum CaptureFormat {
    Png,
    Pdf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum CaptureProvider {
    Takumi,
    WebKit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub enum CaptureCapability {
    IconPng,
    PreviewPng,
    ExportPng,
    VectorPdf,
    WidgetBatch,
}

pub fn capture_capabilities(provider: CaptureProvider) -> Vec<CaptureCapability> {
    use CaptureCapability::*;
    match provider {
        CaptureProvider::Takumi => vec![IconPng, PreviewPng, ExportPng, VectorPdf, WidgetBatch],
        CaptureProvider::WebKit => vec![IconPng, PreviewPng, ExportPng, VectorPdf],
    }
}

/// One selection point for production and explicit internal comparisons. No fallback.
pub fn select_provider(
    capability: CaptureCapability,
    requested: Option<CaptureProvider>,
) -> crate::Result<CaptureProvider> {
    let provider = requested.unwrap_or(CaptureProvider::Takumi);
    if !capture_capabilities(provider).contains(&capability) {
        return Err(crate::err(crate::Code::InvalidRequest, "Capture provider does not support this output"));
    }
    Ok(provider)
}

#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct ViewRenderRequest {
    pub kind: ViewCaptureKind,
    pub format: CaptureFormat,
    pub resources: std::collections::BTreeMap<String, String>,
    pub width: u32,
    pub height: u32,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub snapshot: serde_json::Value,
    pub theme: std::collections::BTreeMap<String, String>,
}

/// A stored app without an icon produces no artwork. A missing native program is an error.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn render_saved(
    path: &std::path::Path,
    evaluator: &hitslop_runner::Evaluator,
    kind: ViewCaptureKind,
    format: CaptureFormat,
) -> crate::store::Result<Option<Vec<u8>>> {
    use crate::store::{Mode, Store};
    let invalid = |message: String| crate::store::Error::Rejected(crate::err(crate::Code::InvalidRequest, message));
    if format == CaptureFormat::Pdf && kind != ViewCaptureKind::Export {
        return Err(invalid("PDF requires the export role".into()));
    }
    let store = Store::open(path, Mode::Snapshot)?;
    let icon = kind == ViewCaptureKind::Icon;
    if kind == ViewCaptureKind::Export && !store.app().app.views().export {
        return Err(invalid("This slop does not support PNG or PDF export (no Export.svelte)".into()));
    }
    let program = match kind {
        ViewCaptureKind::Icon => &store.app().icon,
        ViewCaptureKind::Preview => &store.app().preview,
        ViewCaptureKind::Export => &store.app().export,
    };
    let Some(program) = program.as_deref() else {
        if icon && !store.app().app.views().icon {
            return Ok(None);
        }
        return Err(invalid("Rebuild this pre-launch slop with a synchronous native capture view".into()));
    };
    let document = store.document()?;
    let request = ViewRenderRequest {
        kind,
        format,
        resources: resources(&store, &document)?,
        width: if icon { 512 } else { store.app().app.window().width },
        height: if icon { 512 } else { store.app().app.window().height },
        snapshot: serde_json::from_str(&document.value()).map_err(|e| invalid(e.to_string()))?,
        theme: document.theme_state().map_err(crate::store::Error::Rejected)?.effective.into_iter().collect(),
    };
    let result = match format {
        CaptureFormat::Png => evaluator.capture(store.app().runtime_abi, program, &crate::encode(&request)),
        CaptureFormat::Pdf => evaluator.pdf(store.app().runtime_abi, program, &crate::encode(&request)),
    }
    .map_err(invalid)?;
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
    let bytes = data_encoding::BASE64
        .decode(images.pop().unwrap().as_bytes())
        .map_err(|_| invalid("Invalid capture output encoding".into()))?;
    match format {
        CaptureFormat::Png => {
            crate::images::check(
                &bytes,
                if icon { crate::images::Purpose::Icon } else { crate::images::Purpose::Preview },
            )
            .map_err(crate::store::Error::Rejected)?;
        }
        CaptureFormat::Pdf => {
            if bytes.len() > hitslop_runner::PDF_BYTES
                || !bytes.starts_with(b"%PDF-")
                || !bytes.ends_with(b"%%EOF") && !bytes.ends_with(b"%%EOF\n")
            {
                return Err(invalid("Invalid or oversized capture PDF".into()));
            }
        }
    }
    Ok(Some(bytes))
}

/// A bounded inventory of referenced passive images. No decoding or authored code
/// runs here: the restricted child resolves data URLs and decodes the bytes.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub(crate) fn resources(
    store: &crate::store::Store,
    document: &crate::Document,
) -> crate::store::Result<std::collections::BTreeMap<String, String>> {
    let attachments = store.attachments()?;
    let ids: Vec<_> = attachments.iter().map(|item| item.id.clone()).collect();
    let referenced = document.attachment_references(&ids);
    let mut resources = std::collections::BTreeMap::new();
    let mut size = 0u64;
    for item in attachments {
        if !referenced.contains(&item.id)
            || !matches!(item.media_type.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp")
        {
            continue;
        }
        size += item.bytes;
        if size > 24 * 1024 * 1024 {
            return Err(crate::store::Error::Rejected(crate::err(
                crate::Code::TooLarge,
                "Capture images exceed the 24 MiB resource limit",
            )));
        }
        let bytes = store.attachment(&item.id)?;
        resources.insert(item.id, format!("data:{};base64,{}", item.media_type, data_encoding::BASE64.encode(&bytes)));
    }
    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_selection_is_explicit_and_refuses_unsupported_outputs() {
        assert_eq!(select_provider(CaptureCapability::VectorPdf, None).unwrap(), CaptureProvider::Takumi);
        assert_eq!(
            select_provider(CaptureCapability::ExportPng, Some(CaptureProvider::WebKit)).unwrap(),
            CaptureProvider::WebKit
        );
        assert!(select_provider(CaptureCapability::WidgetBatch, Some(CaptureProvider::WebKit)).is_err());
    }
}
