//! Pack the explicit Vite inventory. The app row is written last and seals its assets.
use super::assets::encode;
use super::build::{AcceptedBuild, accept};
use super::copy::Staged;
use super::{APPLICATION_ID, Kind, SCHEMA, STORAGE_VERSION, configure_writer, opened, reader, rows, writer};
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
    rows::put_checkpoint(&tx, &build.checkpoint)?;
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

/// The compiler provides the declaration and inventory as JSON; resource files are
/// opened beneath stage exactly once. No directory discovery or app.json is accepted.
pub fn pack(input: &str, stage: &Path, dest: &Path) -> Result<()> {
    let build = accept(input, stage)?;
    let staged = Staged::beside(dest)?;
    write_template(staged.path(), &build)?;
    if opened(&reader(staged.path())?, staged.path(), true)?.kind != Kind::Template {
        return Err(failed("Packing produced document state"));
    }
    staged.publish_template(dest)
}
pub fn validate_app(input: &str, stage: &Path) -> Result<()> {
    accept(input, stage).map(|_| ())
}

pub(super) fn initial_checkpoint(app: &crate::AppSpec, initial: &str) -> Result<Vec<u8>> {
    let checkpoint = crate::Document::initial_checkpoint(app, initial).map_err(Error::Rejected)?;
    if checkpoint.len() + 512 > crate::STORAGE_BYTES {
        return Err(invalid("The app's initial values are too large"));
    }
    Ok(checkpoint)
}
