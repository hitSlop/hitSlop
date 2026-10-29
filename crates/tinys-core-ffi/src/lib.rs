//! The Swift adapter. Core types that reach the host unchanged cross as UniFFI remote
//! types: each is restated here once, checked against its definition at compile time, and
//! named for Swift in `uniffi.toml`.
uniffi::setup_scaffolding!();

use tinys_core::Origin;
use tinys_core::file::{self, Artwork, Kind, TemplateSource};
use tinys_core::owner::Failure;
use tinys_core::shape::{Length, Segment, Silhouette};
use tinys_core::store::{self, Attachment, Mode};

use std::path::Path;
use std::sync::{Arc, Mutex};

/// Every call fails with the owner's `Failure`, the one classification the host maps to its
/// own terms. A rejected request leaves the owner usable; an invalidated owner refuses every
/// call until the host reloads saved state into a new owner; every save failure keeps
/// ownership, the live state and all edits.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{failure}")]
    Failure { failure: Failure },
}
impl From<Failure> for CoreError {
    fn from(failure: Failure) -> Self {
        Self::Failure { failure }
    }
}
impl From<store::Error> for CoreError {
    fn from(e: store::Error) -> Self {
        Failure::from(e).into()
    }
}
impl From<tinys_core::Error> for CoreError {
    fn from(e: tinys_core::Error) -> Self {
        Failure::from(e).into()
    }
}

/// The native palette uses typed maps; Swift never interprets theme JSON.
#[derive(uniffi::Record)]
pub struct ThemeState {
    pub defaults: std::collections::HashMap<String, String>,
    pub overrides: std::collections::HashMap<String, String>,
    pub effective: std::collections::HashMap<String, String>,
}
impl From<tinys_core::theme::ThemeState> for ThemeState {
    fn from(state: tinys_core::theme::ThemeState) -> Self {
        Self {
            defaults: state.defaults.into_iter().collect(),
            overrides: state.overrides.into_iter().collect(),
            effective: state.effective.into_iter().collect(),
        }
    }
}

/// A window corner length: points, or a percentage of the window's width or height.
#[uniffi::remote(Record)]
pub struct Length {
    pub value: f64,
    pub percent: bool,
}
/// Absolute path commands in SVG's y-down coordinates.
#[uniffi::remote(Enum)]
pub enum Segment {
    Move { x: f64, y: f64 },
    Line { x: f64, y: f64 },
    Cubic { x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 },
    Close,
}
/// A parsed manifest window shape; see `tinys_core::shape`.
#[uniffi::remote(Enum)]
pub enum Silhouette {
    Radii { horizontal: Vec<Length>, vertical: Vec<Length> },
    Path { segments: Vec<Segment>, view_box_width: f64, view_box_height: f64, even_odd: bool },
}

/// Who made a change: the person, in the page or the window's own controls (the theme
/// panel), or an agent through the CLI or socket.
#[uniffi::remote(Enum)]
pub enum Origin {
    Page,
    Window,
    Agent,
}
/// Whether another writer holds the document's lock now.
#[uniffi::export]
pub fn writer_lock_held(path: String) -> Result<bool, CoreError> {
    match tinys_core::registry::Lease::acquire(Path::new(&path)) {
        Ok(_) => Ok(false),
        Err(store::Error::Locked) => Ok(true),
        Err(error) => Err(error.into()),
    }
}
/// The core build this library embeds; app and engine must report the same value.
#[uniffi::export]
pub fn core_build_id() -> String {
    tinys_core::BUILD_ID.into()
}

#[uniffi::remote(Enum)]
pub enum Mode {
    /// Owns the document: holds the writer lock and persists writes.
    Document,
    /// Reads the saved state without the lock and writes nothing.
    Snapshot,
}
/// A stored attachment: its identity (SHA-256, hex) and size in bytes.
#[uniffi::remote(Record)]
pub struct Attachment {
    pub media_type: String,
    pub id: String,
    pub bytes: u64,
}

#[uniffi::remote(Enum)]
pub enum Kind {
    Template,
    Document,
}
/// Complete acceptance, for rendering or editing the embedded app.
#[derive(uniffi::Record)]
pub struct OpenedFile {
    pub kind: Kind,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub app: AppDefinition,
    pub skin_png: Option<Vec<u8>>,
    pub byte_count: u64,
}
impl From<&file::OpenedApp> for OpenedFile {
    fn from(p: &file::OpenedApp) -> Self {
        Self {
            kind: p.kind,
            package_format: p.package_format,
            runtime_abi: p.runtime_abi,
            app: AppDefinition::from(&p.app),
            skin_png: p.skin.clone(),
            byte_count: p.bytes,
        }
    }
}
#[uniffi::export]
pub fn open_file(path: String) -> Result<OpenedFile, CoreError> {
    Ok((&file::open(Path::new(&path), false)?).into())
}
/// Cheap display metadata. This is not a validity certificate for the embedded app.
#[uniffi::export]
pub fn file_summary(path: String) -> Result<file::Summary, CoreError> {
    Ok(file::summary(Path::new(&path))?)
}
/// Catalog templates additionally have a filename matching their slug.
#[uniffi::export]
pub fn open_template(path: String) -> Result<file::Summary, CoreError> {
    Ok(file::open_template(Path::new(&path))?)
}
/// Where a listed template comes from.
#[uniffi::remote(Enum)]
pub enum TemplateSource {
    Bundled,
    Installed,
}
/// A template folder the catalog lists.
#[derive(uniffi::Record)]
pub struct TemplateRoot {
    pub source: TemplateSource,
    pub path: String,
}
/// The template folders the catalog lists, the bundled starters first: the core's, so the
/// app and the CLI list the same templates and keep new documents out of them.
#[uniffi::export]
pub fn template_roots() -> Vec<TemplateRoot> {
    file::template_roots()
        .into_iter()
        .map(|(source, path)| TemplateRoot { source, path: path.to_string_lossy().into_owned() })
        .collect()
}
/// A file's kind from its header checks alone, for a host deciding how to open it.
#[uniffi::export]
pub fn file_kind(path: String) -> Result<Kind, CoreError> {
    Ok(file::kind(Path::new(&path))?)
}
/// A new document from a template; never replaces an existing file.
#[uniffi::export]
pub fn create_document(template: String, destination: String) -> Result<(), CoreError> {
    Ok(file::create_document(Path::new(&template), Path::new(&destination))?)
}
/// A Files import: source stays untouched; destination has its own identity and state.
#[uniffi::export]
pub fn import_document(source: String, destination: String) -> Result<(), CoreError> {
    Ok(store::Store::import_copy(Path::new(&source), Path::new(&destination))?)
}
/// Deletes a closed local document under its writer lock.
#[uniffi::export]
pub fn delete_document(path: String) -> Result<(), CoreError> {
    Ok(file::delete_document(Path::new(&path))?)
}
#[derive(uniffi::Record)]
pub struct RelocatedDocument {
    pub path: String,
    pub cleanup_error: Option<String>,
}
/// An explicit local move, preserving document identity and all retained history.
#[uniffi::export]
pub fn relocate_document(source: String, folder: String) -> Result<RelocatedDocument, CoreError> {
    #[cfg(target_os = "macos")]
    {
        let moved = file::relocate_document(Path::new(&source), Path::new(&folder))?;
        Ok(RelocatedDocument { path: moved.path.to_string_lossy().into_owned(), cleanup_error: moved.cleanup_error })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (source, folder);
        Err(store::Error::Failed("Moving a cloud document is unavailable on this platform".into()).into())
    }
}
/// A tiny's preview or icon artwork.
#[uniffi::remote(Enum)]
pub enum Artwork {
    Preview,
    Icon,
}
/// One of a file's artwork images.
#[derive(uniffi::Record)]
pub struct ArtworkImage {
    pub name: Artwork,
    pub png: Vec<u8>,
}
/// The first of `preferred` artwork the file holds, in one read. An error (busy, damaged,
/// mid-recovery) is never "no artwork".
#[uniffi::export]
pub fn file_artwork(path: String, preferred: Vec<Artwork>) -> Result<Option<ArtworkImage>, CoreError> {
    Ok(file::artwork(Path::new(&path), &preferred)?.map(|(name, png)| ArtworkImage { name, png }))
}
/// Uses `path` as this process's writer-lock registry: debug hosts only, for test runs
/// (`TINYS_TEST_REGISTRY`). Every process sharing documents must use the same folder.
#[uniffi::export]
pub fn use_registry_folder(path: String) -> Result<(), CoreError> {
    Ok(tinys_core::registry::use_folder(Path::new(&path))?)
}
/// Removes discovery files a crashed owner left; returns how many. Run at app launch.
#[uniffi::export]
pub fn sweep_registry() -> Result<u32, CoreError> {
    Ok(tinys_core::registry::sweep()? as u32)
}
/// The live owner's discovery for a document file, when one is published.
#[uniffi::export]
pub fn live_discovery(path: String) -> Result<Option<String>, CoreError> {
    Ok(tinys_core::registry::discovery(Path::new(&path))?)
}
/// Whether `path` is an app asset's key a page may name: relative, at most 240 bytes, with
/// no empty, dot or parent segments.
#[uniffi::export]
pub fn valid_asset_path(path: String) -> bool {
    file::valid_asset_path(&path)
}
/// The content type a served file carries, by its extension.
#[uniffi::export]
pub fn content_type(path: String) -> String {
    file::content_type(&path).into()
}
/// Serves package assets and attachments through disjoint checked routes.
#[derive(uniffi::Object)]
pub struct ResourceReader(Mutex<file::ResourceReader>);
#[uniffi::export]
impl ResourceReader {
    pub fn info(&self, route: file::ResourceRoute, key: String) -> Result<Option<file::ResourceInfo>, CoreError> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).info(route, &key)?)
    }
    pub fn read_range(
        &self,
        route: file::ResourceRoute,
        key: String,
        offset: u64,
        length: u64,
    ) -> Result<Option<Vec<u8>>, CoreError> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).read_range(route, &key, offset, length)?)
    }
}

mod app;
pub use app::*;

mod owner;
pub use owner::*;

mod command;
pub use command::*;

mod page;
mod widgets;

mod library;

mod sharing;
pub use sharing::*;
