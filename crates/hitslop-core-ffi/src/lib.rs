uniffi::setup_scaffolding!();

use hitslop_core::{file, store};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A rejected request leaves the owner usable; an invalidated owner refuses every call
/// until the host reloads saved state into a new owner. The remaining cases are storage
/// failures; every save failure keeps ownership, the live state and all edits.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{code}: {message}")]
    Rejected {
        code: String,
        message: String,
        op_index: Option<u32>,
    },
    #[error("{message}")]
    Invalidated { message: String },
    /// Another process holds the writer lock.
    #[error("document has a live writer")]
    Locked,
    /// Another connection held the database; retrying can succeed.
    #[error("document database is busy")]
    Busy,
    /// The write would exceed the storage limits; saved state is intact.
    #[error("document is full")]
    Full,
    /// The document file was moved or replaced while open.
    #[error("document moved or replaced")]
    Moved,
    /// The store no longer owns the document.
    #[error("document is closed")]
    Closed,
    #[error("{message}")]
    Failed { message: String },
}
fn rejected(e: hitslop_core::Error) -> CoreError {
    CoreError::Rejected {
        code: e.code.as_str().into(),
        message: e.message,
        op_index: e.op_index.map(|i| i as u32),
    }
}
impl From<store::Error> for CoreError {
    fn from(e: store::Error) -> Self {
        match e {
            store::Error::Locked => Self::Locked,
            store::Error::Busy => Self::Busy,
            store::Error::Full => Self::Full,
            store::Error::Moved => Self::Moved,
            store::Error::Closed => Self::Closed,
            store::Error::Rejected(e) => rejected(e),
            store::Error::Failed(message) => Self::Failed { message },
        }
    }
}
impl From<hitslop_core::Error> for CoreError {
    fn from(e: hitslop_core::Error) -> Self {
        rejected(e)
    }
}

/// A declared theme color.
#[derive(uniffi::Record)]
pub struct ThemeToken {
    pub name: String,
    pub value: String,
}
#[derive(uniffi::Record)]
pub struct ThemeState {
    pub defaults: String,
    pub overrides: String,
    pub effective: String,
}

/// The platform envelopes whose generated contracts the core evaluates.
#[derive(uniffi::Enum)]
pub enum EnvelopeKind {
    SocketRequest,
    SocketReply,
    SocketDiscovery,
    PageRequest,
}
/// Whether `json` is a well-formed envelope of this kind.
#[uniffi::export]
pub fn envelope_is_valid(kind: EnvelopeKind, json: Vec<u8>) -> bool {
    use hitslop_core::envelope::Envelope;
    hitslop_core::envelope::is_valid(
        match kind {
            EnvelopeKind::SocketRequest => Envelope::SocketRequest,
            EnvelopeKind::SocketReply => Envelope::SocketReply,
            EnvelopeKind::SocketDiscovery => Envelope::SocketDiscovery,
            EnvelopeKind::PageRequest => Envelope::PageRequest,
        },
        &json,
    )
}

/// A window corner length: points, or a percentage of the window's width or height.
#[derive(uniffi::Record)]
pub struct SilhouetteLength {
    pub value: f64,
    pub percent: bool,
}
/// Absolute path commands in SVG's y-down coordinates.
#[derive(uniffi::Enum)]
pub enum SilhouetteSegment {
    Move { x: f64, y: f64 },
    Line { x: f64, y: f64 },
    Cubic { x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 },
    Close,
}
/// A parsed manifest window shape; see `hitslop_core::shape`.
#[derive(uniffi::Enum)]
pub enum WindowSilhouette {
    Radii { horizontal: Vec<SilhouetteLength>, vertical: Vec<SilhouetteLength> },
    Path { segments: Vec<SilhouetteSegment>, view_box_width: f64, view_box_height: f64, even_odd: bool },
}
fn window_silhouette(silhouette: hitslop_core::shape::Silhouette) -> WindowSilhouette {
    use hitslop_core::shape::{self, Segment, Silhouette};
    let length = |l: shape::Length| SilhouetteLength { value: l.value, percent: l.percent };
    match silhouette {
        Silhouette::Radii { horizontal, vertical } => WindowSilhouette::Radii {
            horizontal: horizontal.into_iter().map(length).collect(),
            vertical: vertical.into_iter().map(length).collect(),
        },
        Silhouette::Path { segments, view_box, even_odd } => WindowSilhouette::Path {
            segments: segments.into_iter().map(|segment| match segment {
                Segment::Move { x, y } => SilhouetteSegment::Move { x, y },
                Segment::Line { x, y } => SilhouetteSegment::Line { x, y },
                Segment::Cubic { x1, y1, x2, y2, x, y } => SilhouetteSegment::Cubic { x1, y1, x2, y2, x, y },
                Segment::Close => SilhouetteSegment::Close,
            }).collect(),
            view_box_width: view_box[0],
            view_box_height: view_box[1],
            even_odd,
        },
    }
}

/// `publication` is absent when the batch changed nothing.
/// Who made a change: the person, in the page or the window's own controls (the theme
/// panel), or an agent through the CLI or socket.
#[derive(uniffi::Enum)]
pub enum EditOrigin {
    Page,
    Window,
    Agent,
}
impl From<EditOrigin> for hitslop_core::Origin {
    fn from(origin: EditOrigin) -> Self {
        match origin {
            EditOrigin::Page => Self::Page,
            EditOrigin::Window => Self::Window,
            EditOrigin::Agent => Self::Agent,
        }
    }
}
/// Whether another writer holds the document's lock now.
#[uniffi::export]
pub fn writer_lock_held(path: String) -> Result<bool, CoreError> {
    match hitslop_core::registry::Lease::acquire(Path::new(&path)) {
        Ok(_) => Ok(false),
        Err(store::Error::Locked) => Ok(true),
        Err(error) => Err(error.into()),
    }
}
/// The core build this library embeds; app and helper must report the same value.
#[uniffi::export]
pub fn core_build_id() -> String {
    hitslop_core::BUILD_ID.into()
}

#[derive(uniffi::Enum)]
pub enum StoreMode {
    /// Owns the document: holds the writer lock and persists writes.
    Document,
    /// Reads the saved state without the lock and writes nothing.
    Snapshot,
}
/// A stored attachment: its identity (SHA-256, hex) and size in bytes.
#[derive(uniffi::Record)]
pub struct AttachmentRecord {
    pub id: String,
    pub byte_length: u64,
}
impl From<store::Attachment> for AttachmentRecord {
    fn from(a: store::Attachment) -> Self {
        Self { id: a.id, byte_length: a.bytes }
    }
}

#[derive(uniffi::Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    Template,
    Document,
}
/// A checked template or document: its markers and the app a host needs to show it.
#[derive(uniffi::Record)]
pub struct OpenedFile {
    pub kind: FileKind,
    pub runtime_abi: u64,
    /// The authored manifest (JSON).
    pub manifest_json: String,
    pub silhouette: WindowSilhouette,
    pub descriptor_json: String,
    /// The declared colors, in the order the author wrote them.
    pub theme_tokens: Vec<ThemeToken>,
    /// The window skin's PNG, when the manifest names one.
    pub skin_png: Option<Vec<u8>>,
    pub byte_count: u64,
}
impl From<file::Kind> for FileKind {
    fn from(kind: file::Kind) -> Self {
        match kind {
            file::Kind::Template => FileKind::Template,
            file::Kind::Document => FileKind::Document,
        }
    }
}
impl From<&file::OpenedApp> for OpenedFile {
    fn from(p: &file::OpenedApp) -> Self {
        OpenedFile {
            kind: p.kind.into(),
            runtime_abi: p.app.runtime_abi,
            manifest_json: p.app.manifest.clone(),
            silhouette: window_silhouette(p.silhouette.clone()),
            descriptor_json: p.app.descriptor.clone(),
            theme_tokens: p.spec.theme_tokens().iter().map(|(name, value)| ThemeToken { name: name.clone(), value: value.clone() }).collect(),
            skin_png: p.skin.clone(),
            byte_count: p.bytes,
        }
    }
}
/// Opens and checks a template or document file for display (the catalog, a template opened
/// from Finder), without its writer lock or SQLite's quick check. A document a host edits
/// opens through `NativeStore`, whose `app` is its one check.
#[uniffi::export]
pub fn open_file(path: String) -> Result<OpenedFile, CoreError> {
    Ok((&file::open(Path::new(&path), false)?).into())
}
/// A file's kind from its header checks alone, for a host deciding how to open it.
#[uniffi::export]
pub fn file_kind(path: String) -> Result<FileKind, CoreError> {
    Ok(file::kind(Path::new(&path))?.into())
}
/// A new document from a template; never replaces an existing file.
#[uniffi::export]
pub fn create_document(template: String, destination: String) -> Result<(), CoreError> {
    Ok(file::create_document(Path::new(&template), Path::new(&destination))?)
}
/// One of a file's artwork images, by name (`preview` or `icon`).
#[derive(uniffi::Record)]
pub struct ArtworkImage {
    pub name: String,
    pub png: Vec<u8>,
}
/// The first of `preferred` artwork the file holds, in one read. An error (busy, damaged,
/// mid-recovery) is never "no artwork".
#[uniffi::export]
pub fn file_artwork(path: String, preferred: Vec<String>) -> Result<Option<ArtworkImage>, CoreError> {
    let preferred: Vec<&str> = preferred.iter().map(String::as_str).collect();
    Ok(file::artwork(Path::new(&path), &preferred)?.map(|(name, png)| ArtworkImage { name, png }))
}
/// Uses `path` as this process's writer-lock registry: debug hosts only, for test runs
/// (`HITSLOP_TEST_REGISTRY`). Every process sharing documents must use the same folder.
#[uniffi::export]
pub fn use_registry_folder(path: String) -> Result<(), CoreError> {
    Ok(hitslop_core::registry::use_folder(Path::new(&path))?)
}
/// Removes discovery files a crashed owner left; returns how many. Run at app launch.
#[uniffi::export]
pub fn sweep_registry() -> Result<u32, CoreError> {
    Ok(hitslop_core::registry::sweep()? as u32)
}
/// The live owner's discovery for a document file, when one is published.
#[uniffi::export]
pub fn live_discovery(path: String) -> Result<Option<String>, CoreError> {
    Ok(hitslop_core::registry::discovery(Path::new(&path))?)
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
/// Serves a document's app assets: whole, or a byte range (`NativeStore::asset_reader`).
#[derive(uniffi::Object)]
pub struct AssetReader(Mutex<file::AssetReader>);
#[uniffi::export]
impl AssetReader {
    pub fn size(&self, key: String) -> Result<Option<u64>, CoreError> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).size(&key)?)
    }
    pub fn read_range(&self, key: String, offset: u64, length: u64) -> Result<Option<Vec<u8>>, CoreError> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).read_range(&key, offset, length)?)
    }
}

mod owner;
pub use owner::*;

mod command;
pub use command::*;
