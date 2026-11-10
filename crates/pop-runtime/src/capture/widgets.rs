//! Disposable widget presentation contracts. These contain no document state.

mod fingerprint;

use crate::store::{failed, invalid};

pub use pop_contracts::widgets::*;

fn source_version(document: &crate::Document, renderer: &fingerprint::Fingerprint) -> String {
    use sha2::{Digest, Sha256};
    // An opaque presentation token, not a document format version. Include the
    // installed renderer/fonts and canvases so app updates refresh unchanged state.
    // Artwork writes and compaction still do not change the saved semantic version.
    let mut hash = Sha256::new();
    // The version hashes the evaluator binary, but core-side capture preparation
    // (image inventory, per-output budget) is outside that digest; changes there bump this tag.
    hash.update(b"pop-widget-capture-v3:takumi\0");
    hash.update(renderer.digest);
    hash.update(widget_limits().scale.to_le_bytes());
    hash.update(crate::encode(&widget_viewports()).as_bytes());
    hash.update(document.version().as_bytes());
    data_encoding::HEXLOWER.encode(&hash.finalize())
}

pub fn saved_version(path: &std::path::Path, evaluator: &pop_runner::Evaluator) -> crate::store::Result<String> {
    let renderer = fingerprint::renderer(evaluator.executable_path()).map_err(failed)?;
    let store = crate::store::Store::open(path, crate::store::Mode::Snapshot)?;
    let version = source_version(&store.document()?, &renderer);
    renderer.check(evaluator.executable_path()).map_err(failed)?;
    Ok(version)
}

/// The caller acquires a saved source through the owner before rendering it.
/// Only immutable state and the private program leave the core, into a restricted child.
pub(crate) fn render_source(
    source: &super::RenderSource<'_>,
    evaluator: &pop_runner::Evaluator,
) -> crate::store::Result<WidgetCapture> {
    let store = source.store;
    let document = source.document;
    let renderer = fingerprint::renderer(evaluator.executable_path()).map_err(failed)?;
    if store.app().app.views().widget.is_none() {
        return Err(invalid("This pop has no Widget.svelte"));
    }
    let program = store
        .app()
        .widget
        .as_deref()
        .ok_or_else(|| invalid("Rebuild this pop to include its widget render program"))?;
    let source_version = source_version(document, &renderer);
    let input = source.input()?;
    let request = WidgetRenderRequest {
        snapshot: &input.snapshot,
        theme: &input.theme,
        viewports: widget_viewports(),
        attachments: &input.attachments,
    };
    let batch = evaluator.markup(store.app().runtime_abi, program, &crate::encode(&request), true).map_err(invalid)?;
    let images = source.images(&batch, super::CaptureOutput::Widgets)?;
    let result =
        evaluator.raster(store.app().runtime_abi, batch, images, pop_runner::JobKind::Widget).map_err(invalid)?;
    let reply: pop_runner::RenderReply =
        serde_json::from_str(&result).map_err(|e| invalid(format!("Invalid capture reply: {e}")))?;
    let images = reply.into_images().map_err(invalid)?;
    if images.len() != request.viewports.len() {
        return Err(invalid("Widget renderer returned an incomplete batch"));
    }
    let images = images
        .into_iter()
        .zip(request.viewports)
        .map(|(image, viewport)| {
            let png =
                data_encoding::BASE64.decode(image.as_bytes()).map_err(|_| invalid("Invalid widget PNG encoding"))?;
            // The sandbox's output is untrusted. The cache independently validates PNGs too.
            let checked =
                crate::images::check(&png, crate::images::Purpose::Preview).map_err(crate::store::Error::Rejected)?;
            if checked.width != viewport.width * 2 || checked.height != viewport.height * 2 {
                return Err(invalid("Widget PNG does not match its viewport"));
            }
            Ok(WidgetImage { family: viewport.family, png })
        })
        .collect::<crate::store::Result<_>>()?;
    renderer.check(evaluator.executable_path()).map_err(failed)?;
    Ok(WidgetCapture { source_version, images })
}
