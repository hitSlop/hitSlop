uniffi::setup_scaffolding!();

use hitslop_core::store;
use hitslop_core::Document as Core;
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
    /// The package directory was moved or replaced while open.
    #[error("document moved or replaced")]
    Moved,
    /// The store no longer owns the package.
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
impl From<CoreError> for store::Error {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Locked => Self::Locked,
            CoreError::Busy => Self::Busy,
            CoreError::Full => Self::Full,
            CoreError::Moved => Self::Moved,
            CoreError::Closed => Self::Closed,
            other => Self::Failed(other.to_string()),
        }
    }
}
impl From<uniffi::UnexpectedUniFFICallbackError> for CoreError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Failed { message: e.reason }
    }
}

/// Validates a package's descriptor and initial value; returns the schema key.
#[uniffi::export]
pub fn validate_document(schema_json: String, initial_json: String) -> Result<String, CoreError> {
    hitslop_core::validate(&schema_json, &initial_json).map_err(rejected)
}
#[uniffi::export]
pub fn validate_theme_defaults(json: String) -> Result<(), CoreError> {
    hitslop_core::theme::validate_defaults(&json).map_err(rejected)
}
/// A theme command; see `hitslop_core::theme::apply`.
#[derive(uniffi::Enum)]
pub enum ThemeChange {
    Get,
    Set { values_json: String },
    Reset { token: Option<String> },
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
    BridgeRequest,
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
            EnvelopeKind::BridgeRequest => Envelope::BridgeRequest,
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
/// Validates the manifest contract and returns its normalized window geometry.
#[uniffi::export]
pub fn validate_manifest(manifest_json: String) -> Result<WindowSilhouette, CoreError> {
    use hitslop_core::shape::{self, Segment, Silhouette};
    let length = |l: shape::Length| SilhouetteLength { value: l.value, percent: l.percent };
    Ok(match hitslop_core::manifest::validate(&manifest_json).map_err(rejected)? {
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
    })
}

/// `publication` is absent when the batch changed nothing.
#[derive(uniffi::Record)]
pub struct ApplyResult {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub publication: Option<String>,
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
    fn construct(
        f: impl FnOnce() -> Result<Core, hitslop_core::Error>,
    ) -> Result<Arc<Self>, CoreError> {
        let core = catch_unwind(AssertUnwindSafe(f))
            .map_err(|_| invalidated("engine_panic: creation failed"))?
            .map_err(rejected)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(core)),
        }))
    }
    fn call<T>(
        &self,
        f: impl FnOnce(&mut Core) -> Result<T, hitslop_core::Error>,
    ) -> Result<T, CoreError> {
        let mut guard = self.inner.lock().map_err(|_| invalidated("owner_poisoned"))?;
        let core = guard
            .as_mut()
            .ok_or_else(|| invalidated("owner_poisoned: reload durable state"))?;
        match catch_unwind(AssertUnwindSafe(|| f(core))) {
            Ok(result) => result.map_err(rejected),
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
    #[uniffi::constructor]
    pub fn create(schema_json: String, initial_json: String) -> Result<Arc<Self>, CoreError> {
        Self::construct(|| Core::create(&schema_json, &initial_json))
    }
    pub fn sequence(&self) -> Result<u64, CoreError> {
        self.call(|d| Ok(d.sequence()))
    }
    pub fn apply_batch(&self, batch_json: String) -> Result<ApplyResult, CoreError> {
        self.call(|d| {
            let applied = d.apply_batch(&batch_json)?;
            Ok(ApplyResult {
                sequence: applied.sequence,
                ids: applied.ids,
                publication: applied.publication,
            })
        })
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
        self.call(|d| d.state())
    }
    /// The application value as JSON.
    pub fn value(&self) -> Result<String, CoreError> {
        self.call(|d| d.value())
    }
    /// The next write for `store`, or none when its durable state covers every edit and
    /// no checkpoint is requested. Runs on the edit queue; the bytes stay in Rust.
    pub fn save_job(&self, store: Arc<NativeStore>, force_checkpoint: bool) -> Result<Option<Arc<SaveJob>>, CoreError> {
        let mut result = Ok(None);
        self.call(|d| {
            result = store.0.job(d, force_checkpoint);
            Ok(())
        })?;
        Ok(result?.map(|job| Arc::new(SaveJob(job))))
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

/// Fault injection for tests; see `hitslop_core::store::Phases`.
#[uniffi::export(with_foreign)]
pub trait StorePhases: Send + Sync {
    fn reached(&self, phase: String) -> Result<(), CoreError>;
}
struct ForeignPhases(Arc<dyn StorePhases>);
impl store::Phases for ForeignPhases {
    fn reached(&self, phase: &str) -> store::Result<()> {
        self.0.reached(phase.into()).map_err(Into::into)
    }
}

#[derive(uniffi::Enum)]
pub enum StoreMode {
    /// Owns the package: holds the writer lock and persists writes.
    Document,
    /// Reads the saved state once, without the lock; writes stay in memory.
    Snapshot,
}
#[derive(uniffi::Record)]
pub struct StoreMetadata {
    pub rows: u64,
    pub update_bytes: u64,
    pub checkpoint_bytes: u64,
}
/// A package's storage. Every call but `NativeDocument::save_job` runs on the host's
/// storage queue.
#[derive(uniffi::Object)]
pub struct NativeStore(store::Store);
#[uniffi::export]
impl NativeStore {
    #[uniffi::constructor]
    pub fn open(root: String, mode: StoreMode) -> Result<Arc<Self>, CoreError> {
        let mode = match mode {
            StoreMode::Document => store::Mode::Document,
            StoreMode::Snapshot => store::Mode::Snapshot,
        };
        Ok(Arc::new(Self(store::Store::open(std::path::Path::new(&root), mode)?)))
    }
    /// The logical document's identity; a duplicate gets a new one.
    pub fn doc_id(&self) -> String {
        self.0.doc_id().into()
    }
    /// The saved document, or `initial_json` saved as its first checkpoint. Also the
    /// reload after discarding unsaved edits.
    pub fn document(&self, schema_key: String, initial_json: String) -> Result<Arc<NativeDocument>, CoreError> {
        let core = catch_unwind(AssertUnwindSafe(|| self.0.document(&schema_key, &initial_json)))
            .map_err(|_| invalidated("engine_panic: open failed"))??;
        Ok(Arc::new(NativeDocument { inner: Mutex::new(Some(core)) }))
    }
    pub fn write(&self, job: Arc<SaveJob>) -> Result<(), CoreError> {
        Ok(self.0.write(&job.0)?)
    }
    /// Runs a theme command against the stored overrides; changes are saved under
    /// ownership, and a snapshot refuses them.
    pub fn theme(&self, defaults_json: String, change: ThemeChange) -> Result<ThemeState, CoreError> {
        use hitslop_core::theme::Change;
        let change = match &change {
            ThemeChange::Get => Change::Get,
            ThemeChange::Set { values_json } => Change::Set(values_json),
            ThemeChange::Reset { token } => Change::Reset(token.as_deref()),
        };
        let state = self.0.theme(&defaults_json, change)?;
        Ok(ThemeState { defaults: state.defaults, overrides: state.overrides, effective: state.effective })
    }
    /// Fails once the package moved, or, with `writable`, once the store owns nothing.
    pub fn check(&self, writable: bool) -> Result<(), CoreError> {
        Ok(self.0.check(writable)?)
    }
    /// Releases the database, then the writer lock. A failed close keeps ownership.
    pub fn close(&self) -> Result<(), CoreError> {
        Ok(self.0.close()?)
    }
    /// The stored sizes; benchmarks and tests compare them with the file.
    pub fn metadata(&self) -> Result<StoreMetadata, CoreError> {
        let meta = self.0.metadata()?;
        Ok(StoreMetadata { rows: meta.rows as u64, update_bytes: meta.update_bytes as u64, checkpoint_bytes: meta.checkpoint_bytes as u64 })
    }
    pub fn set_phases(&self, phases: Option<Arc<dyn StorePhases>>) {
        self.0.set_phases(phases.map(|p| Arc::new(ForeignPhases(p)) as Arc<dyn store::Phases>));
    }
}

/// A package's writer lock alone, for a closed-document command that reads without
/// editing.
#[derive(uniffi::Object)]
pub struct WriterLock(Mutex<Option<store::WriterLock>>);
#[uniffi::export]
impl WriterLock {
    #[uniffi::constructor]
    pub fn acquire(root: String) -> Result<Arc<Self>, CoreError> {
        Ok(Arc::new(Self(Mutex::new(Some(store::WriterLock::acquire(std::path::Path::new(&root))?)))))
    }
    pub fn release(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}

/// Copies a package's saved state into `destination_root` as a new logical document.
#[uniffi::export]
pub fn duplicate_document(source_root: String, destination_root: String) -> Result<(), CoreError> {
    Ok(store::duplicate(std::path::Path::new(&source_root), std::path::Path::new(&destination_root))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Gap: semantic errors do not prove FFI unwind containment. After a panic,
    // every call must refuse until a new owner is explicitly reloaded from storage.
    #[test]
    fn panic_invalidates_owner_and_durable_reload_uses_a_new_owner() {
        let schema = r#"{"kind":"object","properties":{"done":{"kind":"boolean"}}}"#;
        let root = std::env::temp_dir().join(format!("hitslop-ffi-{}.slop", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = NativeStore::open(root.to_string_lossy().into(), StoreMode::Document).unwrap();
        let owner = store.document(schema.into(), r#"{"done":false}"#.into()).unwrap();
        let result: Result<(), _> = owner.call(|_| panic!("injected unwind at the FFI boundary"));
        assert!(matches!(result, Err(CoreError::Invalidated { .. })));
        assert!(matches!(owner.state(), Err(CoreError::Invalidated { .. })));
        assert!(matches!(owner.save_job(store.clone(), true), Err(CoreError::Invalidated { .. })));
        let restored = store.document(schema.into(), r#"{"done":true}"#.into()).unwrap();
        assert!(restored.state().unwrap().contains("\"done\":false"));
        store.close().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
