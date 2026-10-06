uniffi::setup_scaffolding!();

use hitslop_core::{file, store};
use hitslop_core::Document as Core;
use std::path::Path;
use std::panic::{catch_unwind, AssertUnwindSafe};
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
fn invalidated(message: &str) -> CoreError {
    CoreError::Invalidated {
        message: message.into(),
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
/// A theme command; see `hitslop_core::theme::Change`.
#[derive(uniffi::Enum)]
pub enum ThemeChange {
    Get,
    Set { values_json: String },
    Reset { token: Option<String> },
    /// Replaces the overrides with a theme file made for this document's template.
    Import { file_json: String },
}
#[derive(uniffi::Record)]
pub struct ThemeState {
    pub defaults: String,
    pub overrides: String,
    pub effective: String,
    /// Whether the command changed the overrides.
    pub changed: bool,
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
#[derive(uniffi::Record)]
pub struct ApplyResult {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub publication: Option<String>,
}
impl From<hitslop_core::Applied> for ApplyResult {
    fn from(applied: hitslop_core::Applied) -> Self {
        Self { sequence: applied.sequence, ids: applied.ids, publication: applied.publication }
    }
}
/// Who made a change: the person in a window, or an agent through the CLI or socket.
#[derive(uniffi::Enum)]
pub enum EditOrigin {
    Page,
    Agent,
}
/// Whether Edit ▸ Undo and Redo have anything to do.
#[derive(uniffi::Record)]
pub struct UndoState {
    pub can_undo: bool,
    pub can_redo: bool,
}
#[derive(uniffi::Record)]
pub struct TextResult {
    pub sequence: u64,
    pub authored: String,
    pub selection_start: u32,
    pub selection_end: u32,
    pub publication: Option<String>,
}

/// The core build this library embeds; app and helper must report the same value.
#[uniffi::export]
pub fn core_build_id() -> String {
    hitslop_core::BUILD_ID.into()
}

#[derive(uniffi::Object)]
pub struct NativeDocument {
    inner: Mutex<Option<Core>>,
}
impl NativeDocument {
    /// Runs `f` on the core; a panic invalidates this owner.
    fn call<T>(&self, f: impl FnOnce(&mut Core) -> Result<T, CoreError>) -> Result<T, CoreError> {
        let mut guard = self.inner.lock().map_err(|_| invalidated("owner_poisoned"))?;
        let core = guard
            .as_mut()
            .ok_or_else(|| invalidated("owner_poisoned: reload durable state"))?;
        match catch_unwind(AssertUnwindSafe(|| f(core))) {
            Ok(result) => result,
            Err(_) => {
                *guard = None;
                Err(invalidated(
                    "engine_panic: owner invalidated; reload durable state",
                ))
            }
        }
    }
}
#[uniffi::export]
impl NativeDocument {
    pub fn sequence(&self) -> Result<u64, CoreError> {
        self.call(|d| Ok(d.sequence()))
    }
    pub fn apply_batch(&self, batch_json: String, origin: EditOrigin) -> Result<ApplyResult, CoreError> {
        let origin = match origin {
            EditOrigin::Page => hitslop_core::Origin::Page,
            EditOrigin::Agent => hitslop_core::Origin::Agent,
        };
        self.call(|d| Ok(d.apply_batch(&batch_json, origin)?.into()))
    }
    /// Reverts the person's last undo step; nothing to undo publishes nothing.
    pub fn undo(&self) -> Result<ApplyResult, CoreError> {
        self.call(|d| Ok(d.undo()?.into()))
    }
    pub fn redo(&self) -> Result<ApplyResult, CoreError> {
        self.call(|d| Ok(d.redo()?.into()))
    }
    pub fn undo_state(&self) -> Result<UndoState, CoreError> {
        self.call(|d| Ok(UndoState { can_undo: d.can_undo(), can_redo: d.can_redo() }))
    }
    pub fn edit_text(&self, request_json: String) -> Result<TextResult, CoreError> {
        self.call(|d| {
            let edit = d.edit_text(&request_json)?;
            Ok(TextResult {
                sequence: edit.sequence,
                authored: edit.authored,
                selection_start: edit.selection_start as u32,
                selection_end: edit.selection_end as u32,
                publication: edit.publication,
            })
        })
    }
    /// `{sequence, version, value, issues}` as JSON.
    pub fn state(&self) -> Result<String, CoreError> {
        self.call(|d| Ok(d.state()?))
    }
    /// The next write for `store`, or none when its durable state covers every edit and
    /// no checkpoint is requested. Runs on the edit queue; the bytes stay in Rust.
    pub fn save_job(&self, store: Arc<NativeStore>, force_checkpoint: bool) -> Result<Option<Arc<SaveJob>>, CoreError> {
        self.call(|d| Ok(store.0.job(d, force_checkpoint)?.map(|job| Arc::new(SaveJob(job)))))
    }
    /// The checkpoint to write as the owner closes, after its last save, or none. Runs on
    /// the edit queue once edits have stopped.
    pub fn close_job(&self, store: Arc<NativeStore>) -> Result<Option<Arc<SaveJob>>, CoreError> {
        self.call(|d| Ok(store.0.close_job(d)?.map(|job| Arc::new(SaveJob(job)))))
    }
}

/// Bytes exported for one write. Opaque to the host.
#[derive(uniffi::Object)]
pub struct SaveJob(store::SaveJob);
#[uniffi::export]
impl SaveJob {
    pub fn is_checkpoint(&self) -> bool {
        self.0.is_checkpoint()
    }
}

#[derive(uniffi::Enum)]
pub enum StoreMode {
    /// Owns the document: holds the writer lock and persists writes.
    Document,
    /// Reads the saved state without the lock and writes nothing.
    Snapshot,
}
/// A document file's storage. Every call but `NativeDocument::save_job` runs on the host's
/// storage queue.
#[derive(uniffi::Object)]
pub struct NativeStore(store::Store);
#[uniffi::export]
impl NativeStore {
    #[uniffi::constructor]
    pub fn open(path: String, mode: StoreMode) -> Result<Arc<Self>, CoreError> {
        let mode = match mode {
            StoreMode::Document => store::Mode::Document,
            StoreMode::Snapshot => store::Mode::Snapshot,
        };
        Ok(Arc::new(Self(store::Store::open(Path::new(&path), mode)?)))
    }
    /// The file's app as this open checked it: the host shows the document from it.
    pub fn app(&self) -> OpenedFile {
        self.0.app().into()
    }
    /// A reader of the app's assets for the document's pages, on its own connection to the
    /// file this open checked.
    pub fn asset_reader(&self) -> Result<Arc<AssetReader>, CoreError> {
        Ok(Arc::new(AssetReader(Mutex::new(self.0.asset_reader()?))))
    }
    /// The saved document, or the app's initial values saved as its first checkpoint, with
    /// its palette over the app's theme defaults. Also the reload after discarding unsaved
    /// edits.
    pub fn document(&self) -> Result<Arc<NativeDocument>, CoreError> {
        let core = catch_unwind(AssertUnwindSafe(|| self.0.document())).map_err(|_| invalidated("engine_panic: open failed"))??;
        Ok(Arc::new(NativeDocument { inner: Mutex::new(Some(core)) }))
    }
    /// Stores an attachment's bytes; storing the same bytes again returns the same reference.
    pub fn put_attachment(&self, bytes: Vec<u8>) -> Result<AttachmentRecord, CoreError> {
        Ok(self.0.put_attachment(&bytes)?.into())
    }
    /// An attachment's bytes, verified against its identity.
    pub fn attachment(&self, id: String) -> Result<Vec<u8>, CoreError> {
        Ok(self.0.attachment(&id)?)
    }
    /// One artwork image (`preview` or `icon`), through this store's own connection.
    pub fn artwork(&self, name: String) -> Result<Option<Vec<u8>>, CoreError> {
        Ok(self.0.artwork(&name)?)
    }
    pub fn attachments(&self) -> Result<Vec<AttachmentRecord>, CoreError> {
        Ok(self.0.attachments()?.into_iter().map(Into::into).collect())
    }
    /// Writes the document's artwork as its window closes, through the writer's connection.
    pub fn set_artwork(&self, preview: Option<Vec<u8>>, icon: Option<Vec<u8>>) -> Result<(), CoreError> {
        let artwork: Vec<(&str, &[u8])> =
            [("preview", &preview), ("icon", &icon)].into_iter().filter_map(|(name, png)| png.as_deref().map(|png| (name, png))).collect();
        Ok(self.0.set_artwork(&artwork)?)
    }
    /// Names this writer's live socket for clients (`live_discovery`).
    pub fn publish_discovery(&self, json: String) -> Result<(), CoreError> {
        Ok(self.0.publish_discovery(&json)?)
    }
    pub fn withdraw_discovery(&self) {
        self.0.withdraw_discovery()
    }
    /// Copies the open document to `destination` as a new logical document; saves queue
    /// behind the copy.
    pub fn copy_to(&self, destination: String) -> Result<(), CoreError> {
        Ok(self.0.copy_to(Path::new(&destination))?)
    }
    pub fn write(&self, job: Arc<SaveJob>) -> Result<(), CoreError> {
        Ok(self.0.write(&job.0)?)
    }
    /// Runs a theme command against the palette in memory. Runs on the edit queue; the
    /// next save job writes a change, and a snapshot refuses changes.
    pub fn theme(&self, change: ThemeChange) -> Result<ThemeState, CoreError> {
        use hitslop_core::theme::Change;
        let change = match &change {
            ThemeChange::Get => Change::Get,
            ThemeChange::Set { values_json } => Change::Set(values_json),
            ThemeChange::Reset { token } => Change::Reset(token.as_deref()),
            ThemeChange::Import { file_json } => Change::Import(file_json),
        };
        let (state, changed) = self.0.theme(change)?;
        Ok(ThemeState { defaults: state.defaults, overrides: state.overrides, effective: state.effective, changed })
    }
    /// The full palette as a theme file for this document's template.
    pub fn export_theme(&self) -> Result<String, CoreError> {
        Ok(self.0.export_theme()?)
    }
    /// Releases the database, then the writer lock. A failed close keeps ownership.
    pub fn close(&self) -> Result<(), CoreError> {
        Ok(self.0.close()?)
    }
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
            theme_tokens: p.theme_tokens.iter().map(|(name, value)| ThemeToken { name: name.clone(), value: value.clone() }).collect(),
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

#[cfg(test)]
mod tests {
    use super::*;
    // Gap: semantic errors do not prove FFI unwind containment. After a panic,
    // every call must refuse until a new owner is explicitly reloaded from storage.
    #[test]
    fn panic_invalidates_owner_and_durable_reload_uses_a_new_owner() {
        use_registry_folder(std::env::temp_dir().join("hitslop-test-registry").to_string_lossy().into()).unwrap();
        let dir = std::env::temp_dir().join(format!("hitslop-ffi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (stage, template, doc) = (dir.join("stage"), dir.join("Panic.template.slop"), dir.join("Panic.slop"));
        std::fs::create_dir_all(stage.join("assets")).unwrap();
        std::fs::write(stage.join("assets/app.js"), "export default {}").unwrap();
        let manifest = r#"{"author":{"name":"Fixture"},"slug":"panic","title":"Panic","description":"FFI unwind test.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#;
        let app = format!(
            r#"{{"packageFormat":{},"runtimeABI":{},"manifest":{manifest},"descriptor":{{"kind":"object","properties":{{"done":{{"kind":"boolean"}}}}}},"initial":{{"done":false}},"theme":{{}}}}"#,
            hitslop_core::PACKAGE_FORMAT,
            hitslop_core::RUNTIME_ABI
        );
        std::fs::write(stage.join("app.json"), app).unwrap();
        file::pack(&stage, &template).unwrap();
        file::create_document(&template, &doc).unwrap();
        let store = NativeStore::open(doc.to_string_lossy().into(), StoreMode::Document).unwrap();
        let owner = store.document().unwrap();
        owner.apply_batch(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#.into(), EditOrigin::Page).unwrap();
        let result: Result<(), CoreError> = owner.call(|_| panic!("injected unwind at the FFI boundary"));
        assert!(matches!(result, Err(CoreError::Invalidated { .. })));
        assert!(matches!(owner.state(), Err(CoreError::Invalidated { .. })));
        assert!(matches!(owner.save_job(store.clone(), true), Err(CoreError::Invalidated { .. })));
        // The reload reads durable state: the first checkpoint, before the unsaved edit.
        let restored = store.document().unwrap();
        assert!(restored.state().unwrap().contains("\"done\":false"));
        store.close().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
