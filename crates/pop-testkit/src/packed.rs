//! Packed title-app fixtures shared by file and saved-source capture tests.
use crate::{App, add_asset, isolate_registry, media_key, pack, write_app};
use pop_contracts::wire::Code;
use pop_storage::file;
use pop_storage::file::{Artwork, Kind};
use pop_storage::store::Error;
use rusqlite::Connection;
use rusqlite::config::DbConfig;
use std::fs;
use std::path::{Path, PathBuf};

/// The packed title-app descriptor.
pub const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#;
/// The title-app initial state.
pub const INITIAL: &str = r#"{"title":"Initial"}"#;
/// The title-app declaration.
pub const APP: App = App::new(SCHEMA, INITIAL);

/// Builds a stage with script, style and font assets.
pub fn stage(dir: &Path) -> PathBuf {
    isolate_registry();
    let stage = dir.join("stage");
    fs::create_dir_all(stage.join("assets")).unwrap();
    write_app(&stage, APP);
    fs::write(stage.join("assets/ui.js"), "export default { mount() { return {}; } };\n".repeat(64)).unwrap();
    add_asset(&stage, "ui.css", "text/css", b"body { color: var(--pop-accent); }");
    add_asset(&stage, &font_key(), "font/woff2", &font());
    stage
}

/// Returns deliberately synthetic WOFF2 bytes for resource tests.
pub fn font() -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    bytes[..4].copy_from_slice(b"wOF2");
    bytes
}

/// Returns the content-addressed fixture font key.
pub fn font_key() -> String {
    media_key(&font(), "woff2")
}

/// Packs the shared title-app template.
pub fn template(dir: &Path) -> PathBuf {
    let out = dir.join("Template.pop");
    pack(&stage(dir), &out).unwrap();
    out
}

/// Creates an editable copy of the title-app template.
pub fn document(dir: &Path) -> PathBuf {
    let doc = dir.join("Doc.pop");
    file::create_document(&template(dir), &doc).unwrap();
    doc
}

/// Opens a connection with triggers disabled for hostile-file fixtures.
pub fn raw(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false).unwrap();
    conn
}

/// Extracts the expected storage rejection code.
pub fn code(error: Error) -> Code {
    match error {
        Error::Rejected(e) => e.code,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Lists unpublished temporary output files.
pub fn temporaries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

/// Artwork supplied by a fixture renderer.
pub type RenderedArtwork = Result<Vec<(Artwork, Option<Vec<u8>>)>, Error>;
/// A stand-in for the capture renderer: it draws each role asked for with `draw` and notes
/// each batch it was asked for, from the staged template.
pub fn renders<'a>(
    asked: &'a std::cell::RefCell<Vec<Vec<Artwork>>>,
    draw: impl Fn(Artwork) -> Option<Vec<u8>> + 'a,
) -> impl Fn(&Path, &[Artwork]) -> RenderedArtwork + 'a {
    move |template, roles| {
        assert_eq!(file::open(template, true).unwrap().kind, Kind::Template, "renders the staged template");
        asked.borrow_mut().push(roles.to_vec());
        Ok(roles.iter().map(|&name| (name, draw(name))).collect())
    }
}
