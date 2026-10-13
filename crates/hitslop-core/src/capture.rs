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

/// Outputs requested together use the same saved document and accepted app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureOutput {
    Preview,
    Icon,
    Export { format: CaptureFormat },
    Widgets,
}

#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub enum CaptureValue {
    Image { bytes: Option<Vec<u8>> },
    Widgets { capture: crate::widgets::WidgetCapture },
}

/// A rendering failure is local to its output, not a failure to save the document.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub struct CaptureResult {
    pub output: CaptureOutput,
    pub result: crate::store::Result<CaptureValue>,
}

/// The source must already have crossed the owner's page drain/save/copy boundary.
/// This function neither acquires a writer nor publishes artwork or widget caches.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn render_batch(
    path: &std::path::Path,
    evaluator: &hitslop_runner::Evaluator,
    outputs: &[CaptureOutput],
) -> crate::store::Result<Vec<CaptureResult>> {
    let store = crate::store::Store::open(path, crate::store::Mode::Snapshot)?;
    let document = store.document()?;
    let source = RenderSource::new(&store, &document);
    Ok(outputs
        .iter()
        .map(|&output| {
            let result = match output {
                CaptureOutput::Widgets => {
                    crate::widgets::render_source(&source, evaluator).map(|capture| CaptureValue::Widgets { capture })
                }
                _ => render_source(&source, evaluator, output).map(|bytes| CaptureValue::Image { bytes }),
            };
            CaptureResult { output, result }
        })
        .collect())
}

/// A staged template's artwork for `file::pack`: the `roles`, rendered in one batch from its
/// initial values, as raw PNGs; an icon is `None` when the app draws none. Any role's failure
/// fails the batch.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn render_artwork(
    path: &std::path::Path,
    evaluator: &hitslop_runner::Evaluator,
    roles: &[crate::file::Artwork],
) -> crate::store::Result<Vec<(crate::file::Artwork, Option<Vec<u8>>)>> {
    use crate::file::Artwork;
    let outputs: Vec<_> = roles
        .iter()
        .map(|role| match role {
            Artwork::Preview => CaptureOutput::Preview,
            Artwork::Icon => CaptureOutput::Icon,
        })
        .collect();
    render_batch(path, evaluator, &outputs)?
        .into_iter()
        .zip(roles)
        .map(|(capture, &role)| match capture.result? {
            CaptureValue::Image { bytes } => Ok((role, bytes)),
            CaptureValue::Widgets { .. } => unreachable!("artwork renders images"),
        })
        .collect()
}

/// `slop export` of a closed document: its saved state, rendered without a lock, published
/// at `output` without replacing anything there under any spelling, including a file made
/// while it rendered. A template is refused, and a result after `deadline` is discarded.
/// Closed export runs on macOS only, for now.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn export_saved(
    path: &std::path::Path,
    format: CaptureFormat,
    output: &std::path::Path,
    evaluator: &hitslop_runner::Evaluator,
    deadline: std::time::Instant,
) -> crate::store::Result<()> {
    use crate::error::{failed, invalid, rejected};
    use std::io::Write;
    if !cfg!(target_os = "macos") {
        return Err(invalid("Export requires macOS; document edits and authoring run anywhere"));
    }
    let output = crate::file::resolve(output)?;
    // Before rendering: no file, folder or link (even a dangling one) is ever replaced.
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err(rejected(crate::Code::Exists, "A file already exists there"));
    }
    if crate::file::kind(path)? == crate::file::Kind::Template {
        return Err(rejected(crate::Code::IsTemplate, "This is a template; create a document from it first"));
    }
    // `render_source` refuses an app without an Export view and checks the PNG or PDF.
    let rendered = render_batch(path, evaluator, &[CaptureOutput::Export { format }])?;
    let CaptureValue::Image { bytes: Some(bytes) } = rendered.into_iter().next().expect("one result").result? else {
        return Err(invalid("This slop has no native capture view; rebuild its template"));
    };
    let staged = crate::file::Staged::beside(&output)?;
    let write = || -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(staged.path())?;
        file.write_all(&bytes)?;
        file.sync_all()
    };
    write().map_err(|e| failed(format!("Cannot write the export: {e}")))?;
    // Immediately before publishing: a late result is discarded with its staged file.
    if std::time::Instant::now() >= deadline {
        return Err(failed("Command timed out; output was not published"));
    }
    staged.publish_new(&output, true)
}

/// Inputs are prepared lazily after role admission, once for the entire batch. Even
/// a preparation failure is shared; unsupported/absent roles keep their own outcome.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub(crate) struct RenderSource<'a> {
    pub store: &'a crate::store::Store,
    pub document: &'a crate::Document,
    input: std::cell::OnceCell<crate::store::Result<RenderInput>>,
}
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub(crate) struct RenderInput {
    pub resources: std::collections::BTreeMap<String, String>,
    pub snapshot: serde_json::Value,
    pub theme: std::collections::BTreeMap<String, String>,
}
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
impl<'a> RenderSource<'a> {
    pub fn new(store: &'a crate::store::Store, document: &'a crate::Document) -> Self {
        Self { store, document, input: std::cell::OnceCell::new() }
    }
    pub fn input(&self) -> crate::store::Result<&RenderInput> {
        self.input
            .get_or_init(|| {
                Ok(RenderInput {
                    resources: resources(self.store, self.document)?,
                    snapshot: serde_json::from_str(&self.document.value()).map_err(crate::error::invalid)?,
                    theme: self
                        .document
                        .theme_state()
                        .map_err(crate::store::Error::Rejected)?
                        .effective
                        .into_iter()
                        .collect(),
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "page.generated.ts"))]
pub struct ViewRenderRequest<'a> {
    pub kind: ViewCaptureKind,
    pub format: CaptureFormat,
    pub resources: &'a std::collections::BTreeMap<String, String>,
    pub width: u32,
    pub height: u32,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub snapshot: &'a serde_json::Value,
    pub theme: &'a std::collections::BTreeMap<String, String>,
}

/// A stored app without an icon produces no artwork. A missing native program is an error.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
fn render_source(
    source: &RenderSource<'_>,
    evaluator: &hitslop_runner::Evaluator,
    output: CaptureOutput,
) -> crate::store::Result<Option<Vec<u8>>> {
    let invalid = |message: String| crate::store::Error::Rejected(crate::err(crate::Code::InvalidRequest, message));
    let store = source.store;
    let (kind, format) = match output {
        CaptureOutput::Icon => (ViewCaptureKind::Icon, CaptureFormat::Png),
        CaptureOutput::Preview => (ViewCaptureKind::Preview, CaptureFormat::Png),
        CaptureOutput::Export { format } => (ViewCaptureKind::Export, format),
        CaptureOutput::Widgets => return Err(invalid("Widgets require a widget batch".into())),
    };
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
    let input = source.input()?;
    let request = ViewRenderRequest {
        kind,
        format,
        resources: &input.resources,
        width: if icon { 512 } else { store.app().app.window().width },
        height: if icon { 512 } else { store.app().app.window().height },
        snapshot: &input.snapshot,
        theme: &input.theme,
    };
    let result = match format {
        CaptureFormat::Png => evaluator.capture(store.app().runtime_abi, program, &crate::encode(&request)),
        CaptureFormat::Pdf => evaluator.pdf(store.app().runtime_abi, program, &crate::encode(&request)),
    }
    .map_err(invalid)?;
    let reply: hitslop_runner::RenderReply =
        serde_json::from_str(&result).map_err(|e| invalid(format!("Invalid capture reply: {e}")))?;
    let mut images = reply.into_images().map_err(invalid)?;
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
