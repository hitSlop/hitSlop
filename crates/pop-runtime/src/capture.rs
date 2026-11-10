//! Saved-source artwork rendered in the restricted child, without a page or writer.
pub mod widgets;

use pop_contracts::capture::{CaptureFormat, CaptureOutput, ViewCaptureKind, ViewRenderRequest};
use pop_storage::store::{failed, invalid, rejected};

pub enum CaptureValue {
    Image { bytes: Option<Vec<u8>> },
    Widgets { capture: pop_contracts::widgets::WidgetCapture },
}

/// A rendering failure is local to its output, not a failure to save the document.
pub struct CaptureResult {
    pub output: CaptureOutput,
    pub result: pop_storage::store::Result<CaptureValue>,
}

/// The source must already have crossed the owner's page drain/save/copy boundary.
/// This function neither acquires a writer nor publishes artwork or widget caches.
pub fn render_batch(
    path: &std::path::Path,
    evaluator: &pop_runner::Evaluator,
    outputs: &[CaptureOutput],
) -> pop_storage::store::Result<Vec<CaptureResult>> {
    let store = pop_storage::store::Store::open(path, pop_storage::store::Mode::Snapshot)?;
    let document = store.document()?;
    let source = RenderSource::new(&store, &document);
    Ok(outputs
        .iter()
        .map(|&output| {
            let result = match output {
                CaptureOutput::Widgets => {
                    widgets::render_source(&source, evaluator).map(|capture| CaptureValue::Widgets { capture })
                }
                _ => render_source(&source, evaluator, output).map(|bytes| CaptureValue::Image { bytes }),
            };
            CaptureResult { output, result }
        })
        .collect())
}

/// A host's saved-source capture request. iOS renders in process and has no export view.
pub fn render_saved(
    path: &std::path::Path,
    evaluator: &pop_runner::Evaluator,
    outputs: &[CaptureOutput],
) -> pop_storage::store::Result<Vec<CaptureResult>> {
    #[cfg(target_os = "ios")]
    if outputs.iter().any(|output| matches!(output, CaptureOutput::Export { .. })) {
        return Err(failed("Export is unavailable on this host"));
    }
    render_batch(path, evaluator, outputs)
}

/// A document's preview and icon with the saved version they were rendered from.
pub struct SavedArtwork {
    pub source_version: String,
    pub preview: Option<Vec<u8>>,
    pub icon: Option<Vec<u8>>,
}

/// A complete pair from one immutable source; a partial failure returns an error, so the
/// caller keeps its old artwork.
pub fn render_artwork(
    path: &std::path::Path,
    evaluator: &pop_runner::Evaluator,
) -> pop_storage::store::Result<SavedArtwork> {
    let source_version = {
        let store = pop_storage::store::Store::open(path, pop_storage::store::Mode::Snapshot)?;
        store.document()?.reading().map_err(pop_storage::store::Error::Rejected)?.version
    };
    let mut artwork = SavedArtwork { source_version, preview: None, icon: None };
    for capture in render_batch(path, evaluator, &[CaptureOutput::Preview, CaptureOutput::Icon])? {
        let CaptureValue::Image { bytes } = capture.result? else {
            return Err(failed("Artwork capture returned a widget batch"));
        };
        match capture.output {
            CaptureOutput::Preview => artwork.preview = bytes,
            CaptureOutput::Icon => artwork.icon = bytes,
            _ => return Err(failed("Artwork capture returned an unrequested output")),
        }
    }
    Ok(artwork)
}

/// Checks every declared view of a staged template in one batch from its initial values.
/// Only Preview/Icon PNGs requested by `store_roles` are returned for storage; supplied
/// artwork still wins, but never exempts its view from validation.
pub fn render_views(
    path: &std::path::Path,
    evaluator: &pop_runner::Evaluator,
    store_roles: &[pop_storage::file::Artwork],
) -> pop_storage::store::Result<Vec<(pop_storage::file::Artwork, Option<Vec<u8>>)>> {
    use pop_storage::file::Artwork;
    let outputs = {
        let store = pop_storage::store::Store::open(path, pop_storage::store::Mode::Snapshot)?;
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
        let value = capture.result.map_err(|error| invalid(format!("{file} could not render: {error}")))?;
        if let Some(role) = role.filter(|role| store_roles.contains(role)) {
            let CaptureValue::Image { bytes } = value else { unreachable!("artwork renders images") };
            artwork.push((role, bytes));
        }
    }
    Ok(artwork)
}

/// `pop export` of a closed document: its saved state, rendered without a lock, published
/// at `output` without replacing anything there under any spelling, including a file made
/// while it rendered. A template is refused, and a result after `deadline` is discarded.
pub fn export_saved(
    path: &std::path::Path,
    format: CaptureFormat,
    output: &std::path::Path,
    evaluator: &pop_runner::Evaluator,
    deadline: std::time::Instant,
) -> pop_storage::store::Result<()> {
    let output = pop_storage::file::new_output(output)?;
    // Before rendering: no file, folder or link (even a dangling one) is ever replaced.
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err(rejected(pop_contracts::wire::Code::Exists, "A file already exists there"));
    }
    if pop_storage::file::kind(path)? == pop_storage::file::Kind::Template {
        return Err(rejected(
            pop_contracts::wire::Code::IsTemplate,
            "This is a template; create a document from it first",
        ));
    }
    // `render_source` refuses an app without an Export view and checks the PNG or PDF.
    let rendered = render_batch(path, evaluator, &[CaptureOutput::Export { format }])?;
    let CaptureValue::Image { bytes: Some(bytes) } = rendered.into_iter().next().expect("one result").result? else {
        return Err(invalid("This pop has no native capture view; rebuild its template"));
    };
    publish_export(&bytes, &output, || std::time::Instant::now() < deadline)
}

/// Publishes rendered bytes for both live and closed exports. The caller's activity or
/// deadline is checked after the staged bytes are synced, immediately before the rename.
/// A failure after publication reports an unknown outcome and keeps the destination.
pub fn publish_export(
    bytes: &[u8],
    output: &std::path::Path,
    is_active: impl FnOnce() -> bool,
) -> pop_storage::store::Result<()> {
    pop_storage::file::publish_output(bytes, output, is_active)
}

/// Inputs are prepared lazily after role admission, once for the entire batch. Even
/// a preparation failure is shared; unsupported/absent roles keep their own outcome.
pub(crate) struct RenderSource<'a> {
    pub store: &'a pop_storage::store::Store,
    pub document: &'a pop_core::Document,
    input: std::cell::OnceCell<pop_storage::store::Result<RenderInput>>,
}
pub(crate) struct RenderInput {
    pub attachments: std::collections::BTreeMap<String, String>,
    pub snapshot: serde_json::Value,
    pub theme: std::collections::BTreeMap<String, String>,
}
impl<'a> RenderSource<'a> {
    pub fn new(store: &'a pop_storage::store::Store, document: &'a pop_core::Document) -> Self {
        Self { store, document, input: std::cell::OnceCell::new() }
    }
    pub fn input(&self) -> pop_storage::store::Result<&RenderInput> {
        self.input
            .get_or_init(|| {
                let reading = self.document.reading().map_err(pop_storage::store::Error::Rejected)?;
                Ok(RenderInput {
                    attachments: attachment_inventory(self.store, self.document)?,
                    snapshot: reading.value,
                    theme: reading.theme,
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

/// A stored app without an icon produces no artwork. A missing native program is an error.
fn render_source(
    source: &RenderSource<'_>,
    evaluator: &pop_runner::Evaluator,
    output: CaptureOutput,
) -> pop_storage::store::Result<Option<Vec<u8>>> {
    let store = source.store;
    let (kind, format) = match output {
        CaptureOutput::Icon => (ViewCaptureKind::Icon, CaptureFormat::Png),
        CaptureOutput::Preview => (ViewCaptureKind::Preview, CaptureFormat::Png),
        CaptureOutput::Export { format } => (ViewCaptureKind::Export, format),
        CaptureOutput::Widgets => return Err(invalid("Widgets require a widget batch")),
    };
    let icon = kind == ViewCaptureKind::Icon;
    if kind == ViewCaptureKind::Export && !store.app().app.views().export {
        return Err(invalid("This pop does not support PNG or PDF export (no Export.svelte)"));
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
        return Err(invalid("Rebuild this pop with a synchronous native capture view"));
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
                CaptureFormat::Png => pop_contracts::evaluator::JobKind::Capture,
                CaptureFormat::Pdf => pop_contracts::evaluator::JobKind::Pdf,
            },
        )
        .map_err(invalid)?;
    let reply: pop_contracts::evaluator::RenderReply =
        serde_json::from_str(&result).map_err(|e| invalid(format!("Invalid capture reply: {e}")))?;
    let mut images = reply.into_images().map_err(invalid)?;
    if images.len() != 1 {
        return Err(invalid("Capture renderer returned an invalid batch"));
    }
    let bytes = data_encoding::BASE64
        .decode(images.pop().unwrap().as_bytes())
        .map_err(|_| invalid("Invalid capture output encoding"))?;
    match format {
        CaptureFormat::Png => {
            pop_storage::images::check(
                &bytes,
                if icon { pop_storage::images::Purpose::Icon } else { pop_storage::images::Purpose::Preview },
            )
            .map_err(pop_storage::store::Error::Rejected)?;
        }
        CaptureFormat::Pdf => {
            if bytes.len() > pop_contracts::evaluator::PDF_BYTES
                || !bytes.starts_with(b"%PDF-")
                || !bytes.ends_with(b"%%EOF") && !bytes.ends_with(b"%%EOF\n")
            {
                return Err(invalid("Invalid or oversized capture PDF"));
            }
        }
    }
    Ok(Some(bytes))
}

/// Listing metadata never reads blobs; one damaged unused image cannot poison a batch.
fn attachment_inventory(
    store: &pop_storage::store::Store,
    document: &pop_core::Document,
) -> pop_storage::store::Result<std::collections::BTreeMap<String, String>> {
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

impl RenderSource<'_> {
    pub fn images(
        &self,
        batch: &pop_contracts::evaluator::Batch,
        output: CaptureOutput,
    ) -> pop_storage::store::Result<std::collections::BTreeMap<String, String>> {
        let inventory = &self.input()?.attachments;
        let mut referenced = std::collections::BTreeSet::new();
        for frame in &batch.frames {
            for markup in [&frame.html, &frame.css] {
                for suffix in markup.split("pop-attachment:").skip(1) {
                    let Some(id) = suffix.get(..pop_contracts::wire::ATTACHMENT_ID_LENGTH) else {
                        continue;
                    };
                    if pop_contracts::wire::valid_attachment_id(id)
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
                return Err(rejected(
                    pop_contracts::wire::Code::InvalidRequest,
                    format!("{output:?} markup references attachment {id} outside its image inventory"),
                ));
            }
            size +=
                sizes.get(id).ok_or_else(|| failed(format!("{output:?} attachment {id} is no longer available")))?;
        }
        if size > pop_contracts::evaluator::IMAGE_BYTES as u64 {
            return Err(rejected(
                pop_contracts::wire::Code::TooLarge,
                format!("{output:?} images exceed the 24 MiB resource limit"),
            ));
        }
        referenced
            .into_iter()
            .map(|id| {
                let bytes = self
                    .store
                    .attachment(&id)
                    .map_err(|error| failed(format!("{output:?} attachment {id}: {error}")))?;
                let url = format!("data:{};base64,{}", inventory[&id], data_encoding::BASE64.encode(&bytes));
                Ok((id, url))
            })
            .collect()
    }
}
