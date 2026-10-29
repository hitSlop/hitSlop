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
    evaluator: &tinys_runner::Evaluator,
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

/// Checks every declared view of a staged template in one batch from its initial values.
/// Only Preview/Icon PNGs requested by `store_roles` are returned for storage; supplied
/// artwork still wins, but never exempts its view from validation.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn render_views(
    path: &std::path::Path,
    evaluator: &tinys_runner::Evaluator,
    store_roles: &[crate::file::Artwork],
) -> crate::store::Result<Vec<(crate::file::Artwork, Option<Vec<u8>>)>> {
    use crate::file::Artwork;
    let outputs = {
        let store = crate::store::Store::open(path, crate::store::Mode::Snapshot)?;
        let views = store.app().app.views();
        let mut outputs = vec![CaptureOutput::Preview];
        if views.icon {
            outputs.push(CaptureOutput::Icon);
        }
        if views.export {
            outputs.push(CaptureOutput::Export { format: CaptureFormat::Png });
            outputs.push(CaptureOutput::Export { format: CaptureFormat::Pdf });
        }
        if views.widget.is_some() {
            outputs.push(CaptureOutput::Widgets);
        }
        outputs
    };
    let mut artwork = Vec::new();
    for capture in render_batch(path, evaluator, &outputs)? {
        let (file, role) = match capture.output {
            CaptureOutput::Preview => ("Preview.svelte", Some(Artwork::Preview)),
            CaptureOutput::Icon => ("Icon.svelte", Some(Artwork::Icon)),
            CaptureOutput::Export { .. } => ("Export.svelte", None),
            CaptureOutput::Widgets => ("Widget.svelte", None),
        };
        let value =
            capture.result.map_err(|error| crate::error::invalid(format!("{file} could not render: {error}")))?;
        if let Some(role) = role.filter(|role| store_roles.contains(role)) {
            let CaptureValue::Image { bytes } = value else { unreachable!("artwork renders images") };
            artwork.push((role, bytes));
        }
    }
    Ok(artwork)
}

/// `tiny export` of a closed document: its saved state, rendered without a lock, published
/// at `output` without replacing anything there under any spelling, including a file made
/// while it rendered. A template is refused, and a result after `deadline` is discarded.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn export_saved(
    path: &std::path::Path,
    format: CaptureFormat,
    output: &std::path::Path,
    evaluator: &tinys_runner::Evaluator,
    deadline: std::time::Instant,
) -> crate::store::Result<()> {
    use crate::error::{invalid, rejected};
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
        return Err(invalid("This tiny has no native capture view; rebuild its template"));
    };
    publish_export(&bytes, &output, || std::time::Instant::now() < deadline)
}

/// Publishes rendered bytes for both live and closed exports. The caller's activity or
/// deadline is checked after the staged bytes are synced, immediately before the rename.
/// A failure after publication reports an unknown outcome and keeps the destination.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub fn publish_export(
    bytes: &[u8],
    output: &std::path::Path,
    is_active: impl FnOnce() -> bool,
) -> crate::store::Result<()> {
    use crate::error::failed;
    use std::io::Write;
    let output = crate::file::resolve(output)?;
    let staged = crate::file::Staged::beside(&output)?;
    let write = || -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(staged.path())?;
        file.write_all(bytes)?;
        file.sync_all()
    };
    write().map_err(|e| failed(format!("Cannot write the export: {e}")))?;
    // Immediately before publishing: a late result is discarded with its staged file.
    if !is_active() {
        return Err(failed("Command timed out; output was not published"));
    }
    staged.publish_new(&output, true)?.confirmed()
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
    pub attachments: std::collections::BTreeMap<String, String>,
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
                    attachments: attachment_inventory(self.store, self.document)?,
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
    pub attachments: &'a std::collections::BTreeMap<String, String>,
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
    evaluator: &tinys_runner::Evaluator,
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
        return Err(invalid("This tiny does not support PNG or PDF export (no Export.svelte)".into()));
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
        return Err(invalid("Rebuild this pre-launch tiny with a synchronous native capture view".into()));
    };
    let input = source.input()?;
    let request = ViewRenderRequest {
        kind,
        format,
        attachments: &input.attachments,
        width: if icon { 512 } else { store.app().app.window().width },
        height: if icon { 512 } else { store.app().app.window().height },
        snapshot: &input.snapshot,
        theme: &input.theme,
    };
    let batch = evaluator.markup(store.app().runtime_abi, program, &crate::encode(&request), false).map_err(invalid)?;
    let images = source.images(&batch, output)?;
    let result = evaluator
        .raster(
            store.app().runtime_abi,
            batch,
            images,
            match format {
                CaptureFormat::Png => tinys_runner::JobKind::Capture,
                CaptureFormat::Pdf => tinys_runner::JobKind::Pdf,
            },
        )
        .map_err(invalid)?;
    let reply: tinys_runner::RenderReply =
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
            if bytes.len() > tinys_runner::PDF_BYTES
                || !bytes.starts_with(b"%PDF-")
                || !bytes.ends_with(b"%%EOF") && !bytes.ends_with(b"%%EOF\n")
            {
                return Err(invalid("Invalid or oversized capture PDF".into()));
            }
        }
    }
    Ok(Some(bytes))
}

/// Listing metadata never reads blobs; one damaged unused image cannot poison a batch.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
fn attachment_inventory(
    store: &crate::store::Store,
    document: &crate::Document,
) -> crate::store::Result<std::collections::BTreeMap<String, String>> {
    let attachments = store.attachments()?;
    let ids: Vec<_> = attachments.iter().map(|item| item.id.clone()).collect();
    let referenced = document.attachment_references(&ids);
    Ok(attachments
        .into_iter()
        .filter(|item| {
            referenced.contains(&item.id)
                && matches!(item.media_type.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp")
        })
        .map(|item| (item.id, item.media_type))
        .collect())
}

#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
impl RenderSource<'_> {
    pub fn images(
        &self,
        batch: &tinys_runner::Batch,
        output: CaptureOutput,
    ) -> crate::store::Result<std::collections::BTreeMap<String, String>> {
        let inventory = &self.input()?.attachments;
        let mut referenced = std::collections::BTreeSet::new();
        for frame in &batch.frames {
            for markup in [&frame.html, &frame.css] {
                for suffix in markup.split("tiny-attachment:").skip(1) {
                    let Some(id) = suffix.get(..crate::wire::ATTACHMENT_ID_LENGTH) else {
                        continue;
                    };
                    if crate::wire::valid_attachment_id(id)
                        && !suffix.as_bytes().get(id.len()).is_some_and(u8::is_ascii_hexdigit)
                    {
                        referenced.insert(id.to_owned());
                    }
                }
            }
        }
        // Admission and the entire size check precede reads, even if an earlier blob
        // is damaged. Budget refusals have a stable outcome independent of ID order.
        let sizes: std::collections::BTreeMap<_, _> =
            self.store.attachments()?.into_iter().map(|item| (item.id, item.bytes)).collect();
        let mut size = 0u64;
        for id in &referenced {
            if !inventory.contains_key(id) {
                return Err(crate::error::rejected(
                    crate::Code::InvalidRequest,
                    format!("{output:?} markup references attachment {id} outside its image inventory"),
                ));
            }
            size += sizes
                .get(id)
                .ok_or_else(|| crate::error::failed(format!("{output:?} attachment {id} is no longer available")))?;
        }
        if size > tinys_runner::IMAGE_BYTES as u64 {
            return Err(crate::error::rejected(
                crate::Code::TooLarge,
                format!("{output:?} images exceed the 24 MiB resource limit"),
            ));
        }
        referenced
            .into_iter()
            .map(|id| {
                let bytes = self
                    .store
                    .attachment(&id)
                    .map_err(|error| crate::error::failed(format!("{output:?} attachment {id}: {error}")))?;
                let url = format!("data:{};base64,{}", inventory[&id], data_encoding::BASE64.encode(&bytes));
                Ok((id, url))
            })
            .collect()
    }
}
