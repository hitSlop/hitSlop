//! Pack the explicit Vite inventory. The app row is written last and seals its assets.
use super::assets::encode;
use super::build::{AcceptedBuild, accept};
use super::copy::Staged;
use super::{
    APPLICATION_ID, Artwork, Kind, SCHEMA, STORAGE_VERSION, begin_write, configure_writer, opened, pack_artwork,
    reader, rows, writer,
};
use crate::engine::{ArtworkSource, PackedArtwork};
use crate::error::{Error, Result, failed, invalid, sqlite};
use std::path::Path;

pub const APP_INPUT_BYTES: usize = crate::wire::SOCKET_ATTACHMENT;

fn write_template(path: &Path, build: &AcceptedBuild) -> Result<()> {
    let conn = writer(path, true)?;
    conn.execute_batch("PRAGMA page_size=4096; PRAGMA auto_vacuum=FULL;").map_err(sqlite("create"))?;
    configure_writer(&conn)?;
    let tx = super::initialize(&conn)?;
    tx.execute_batch(&format!(
        "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={STORAGE_VERSION}; {SCHEMA}"
    ))
    .map_err(sqlite("create"))?;
    rows::replace_history(&tx, &build.checkpoint)?;
    for asset in &build.assets {
        let (encoding, stored) = encode(&asset.key, &asset.bytes)?;
        rows::put_asset(&tx, &asset.key, asset.media_type, encoding, asset.bytes.len(), &stored)?;
    }
    for (name, png) in &build.artwork {
        rows::put_artwork(&tx, *name, png)?;
    }
    rows::put_app(&tx, &build.app, build.package_format, build.runtime_abi)?;
    tx.commit()?;
    conn.close().map_err(|(_, e)| sqlite("close")(e))
}

/// Writes artwork rendered for a staged template, in one transaction.
fn write_artwork(path: &Path, artwork: &[(Artwork, Vec<u8>)]) -> Result<()> {
    let conn = writer(path, false)?;
    configure_writer(&conn)?;
    let tx = begin_write(&conn, "write artwork")?;
    for (name, png) in artwork {
        rows::put_artwork(&tx, *name, png)?;
    }
    tx.commit()?;
    conn.close().map_err(|(_, e)| sqlite("close")(e))
}
fn check_template(path: &Path) -> Result<()> {
    if opened(&reader(path)?, path, true)?.kind != Kind::Template {
        return Err(failed("Packing produced document state"));
    }
    Ok(())
}

/// Checks the staged template's views and returns only artwork requested for storage,
/// from its initial values: a PNG per role, or `None` for an absent icon. Called even when
/// no artwork needs storing. A closure, so the file format does not depend on capture.
pub type RenderArtwork<'a> = &'a dyn Fn(&Path, &[Artwork]) -> Result<Vec<(Artwork, Option<Vec<u8>>)>>;

/// The compiler provides the declaration and inventory as JSON; resource files are
/// opened beneath stage exactly once. No directory discovery or app.json is accepted.
/// With `render`, every declared view is checked from the staged template. Only preview
/// and icon artwork the build does not supply is stored: a preview is required, an icon
/// only when the app draws one. Any failure leaves `dest` as it was.
pub fn pack(input: &str, stage: &Path, dest: &Path, render: Option<RenderArtwork>) -> Result<PackedArtwork> {
    let build = accept(input, stage)?;
    let staged = Staged::beside(dest)?;
    write_template(staged.path(), &build)?;
    check_template(staged.path())?;
    let supplied = |name| build.artwork.iter().any(|(supplied, _)| *supplied == name);
    let source = |name| if supplied(name) { ArtworkSource::Supplied } else { ArtworkSource::Absent };
    let mut packed = PackedArtwork { preview: source(Artwork::Preview), icon: source(Artwork::Icon) };
    let missing: Vec<_> = Artwork::ALL.into_iter().filter(|&name| !supplied(name)).collect();
    if let Some(render) = render {
        let mut rendered = Vec::new();
        for (name, png) in render(staged.path(), &missing)? {
            if let Some(png) = png {
                rendered.push((name, pack_artwork(name, png)?));
            }
        }
        if missing.contains(&Artwork::Preview) && !rendered.iter().any(|(name, _)| *name == Artwork::Preview) {
            return Err(invalid("The preview view rendered no image"));
        }
        if !rendered.is_empty() {
            write_artwork(staged.path(), &rendered)?;
            check_template(staged.path())?;
        }
        for (name, _) in &rendered {
            match name {
                Artwork::Preview => packed.preview = ArtworkSource::Rendered,
                Artwork::Icon => packed.icon = ArtworkSource::Rendered,
            }
        }
    }
    staged.publish_template(dest)?;
    Ok(packed)
}
/// The accepted window and declared capabilities, which the development preview applies.
pub fn validate_app(input: &str, stage: &Path) -> Result<(crate::app::WindowDefinition, Vec<crate::app::Capability>)> {
    accept(input, stage).map(|build| (build.app.window().clone(), build.app.capabilities().to_vec()))
}

pub(super) fn initial_checkpoint(app: &crate::AppSpec, initial: &str) -> Result<Vec<u8>> {
    let checkpoint = crate::Document::initial_checkpoint(app, initial).map_err(Error::Rejected)?;
    if checkpoint.len() + crate::wire::STORAGE_ROW_OVERHEAD > crate::STORAGE_BYTES {
        return Err(invalid("The app's initial values are too large"));
    }
    Ok(checkpoint)
}
