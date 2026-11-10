//! One document owner. Admission, publication order and save scheduling live on
//! the edit worker; SQLite work runs serially on the persistence worker. Callbacks run
//! without locks and may enqueue another request, but must not wait for its completion.
mod actor;
mod admission;
mod failure;
use actor::Message;
pub use actor::{SyncConnection, SyncEvent, SyncSink};
pub(crate) use admission::{Permit, serialized_size};
pub use failure::{Failure, FailureKind};
use pop_core::{Origin, theme};
pub use pop_runner::Evaluator;
use pop_storage::file::Artwork;
use pop_storage::{file, store};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Failure>;
/// Native theme controls use this typed adapter to the same batch path as page edits.
pub enum ThemeChange {
    Set { values: std::collections::HashMap<String, String> },
    ResetAll,
    ImportFile { file: String },
}
pub fn theme_request(change: ThemeChange) -> Request {
    let intent = match change {
        ThemeChange::Set { values } => pop_contracts::wire::Intent::SetTheme {
            values: values.into_iter().map(|(k, v)| (k, Some(v))).collect(),
            replace: None,
        },
        ThemeChange::ResetAll => {
            pop_contracts::wire::Intent::SetTheme { values: Default::default(), replace: Some(true) }
        }
        ThemeChange::ImportFile { file } => pop_contracts::wire::Intent::ImportTheme { file },
    };
    Request::Apply {
        batch: pop_contracts::wire::Batch { intents: vec![intent], if_version: None },
        origin: Origin::Window,
    }
}

pub enum Request {
    /// The document as it reads (`Document::reading`), and its publication sequence.
    State,
    Apply {
        batch: pop_contracts::wire::Batch,
        origin: Origin,
    },
    Undo {
        redo: bool,
    },
    Command {
        name: String,
        args_json: String,
        origin: Origin,
    },
    Flush,
    /// Reload durable state and replace the page session. Shared documents refuse
    /// ordinary discard; an invalidated owner may explicitly recover its saved state.
    Discard,
    Close {
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// Moves a document that cannot save (a shared one that is full) to a new file: writes
    /// the accepted live state beside `dest` as `Copy` does, checks that it opens as a
    /// document whose state and palette equal the live ones, publishes it at `dest`
    /// without replacing anything, then releases the writer lock without saving. The host
    /// fences edits first (the close barrier), so the state cannot move under the copy. Any
    /// failure removes only the temporary file this request made and leaves the owner open
    /// with every edit.
    Handoff {
        dest: PathBuf,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// The accepted live state as a document of its own, without saving the source's
    /// history first (`Store::copy_clean`), with the artwork its window rendered for it.
    Copy {
        destination: PathBuf,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// The saved document as stored, copied after a flush for a capture to render once.
    CaptureSource {
        destination: PathBuf,
    },
    /// The saved document as stored, copied after a flush: the same document, with its
    /// identity, history and artwork (`Store::backup`). Native hosts only.
    Backup {
        destination: PathBuf,
    },
    /// The saved document as a sharing seed, copied after a flush: its identity, its
    /// history as one checkpoint, only the attachments it references and no artwork
    /// (`Store::stage_seed`). The live document and its file are untouched. Native hosts only.
    StageSeed {
        destination: PathBuf,
    },
    Artwork {
        name: Artwork,
    },
    /// Derived images are admitted only for the saved state they were rendered from.
    SetArtwork {
        source_version: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    Attachments,
    ReadAttachment {
        id: String,
    },
    PutAttachment {
        bytes: Vec<u8>,
    },
    /// Shares the document through a relay endpoint: from then on it keeps its whole
    /// history and every attachment, and the host may attach a connection to that relay.
    Share {
        share: store::Share,
    },
    Unshare,
    /// The palette; changes are batch intents.
    Theme,
    ExportTheme,
}
#[derive(Debug)]
pub enum Reply {
    Command {
        sequence: u64,
        ids: Vec<String>,
        result_json: String,
    },
    Unit,
    /// One reading and its publication sequence.
    State {
        reading: pop_contracts::wire::Reading,
        sequence: u64,
    },
    /// A theme file, the bytes every export writes.
    ThemeFile {
        json: String,
    },
    /// `text` answers the page's text edit.
    Applied {
        sequence: u64,
        ids: Vec<String>,
        text: Option<pop_core::TextEdit>,
    },
    Theme {
        state: theme::ThemeState,
        sequence: u64,
    },
    Bytes {
        bytes: Option<Vec<u8>>,
    },
    Attachments {
        items: Vec<store::Attachment>,
    },
    Attachment {
        item: store::Attachment,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStatus {
    Saved,
    Saving,
    Failed,
}
/// What the owner tells its listener, in order. The listener learns every fact this way:
/// the owner states its undo state and save status when it starts, then each change.
#[derive(Debug)]
pub enum Event {
    /// An encoded PagePush publication item, ready for the page queue.
    Publication {
        json: String,
    },
    SaveStatus {
        status: SaveStatus,
        failure: Option<Failure>,
        pressure: store::Pressure,
        shared: bool,
    },
    UndoState {
        can_undo: bool,
        can_redo: bool,
    },
    ThemeChanged,
}
pub type Completion = Box<dyn FnOnce(Result<Reply>) + Send>;
pub type Listener = Arc<dyn Fn(Event) + Send + Sync>;
fn complete(callback: Completion, value: Result<Reply>) {
    let _ = catch_unwind(AssertUnwindSafe(|| callback(value)));
}
/// A queued request must settle even when shutdown drops its mailbox or worker job.
/// Taking the callback before invocation also prevents a panicking caller from answering twice.
struct PendingCompletion(Option<Completion>);
impl PendingCompletion {
    fn finish(mut self, value: Result<Reply>) {
        let callback = self.0.take().expect("pending completion is answered once");
        complete(callback, value);
    }
}
impl Drop for PendingCompletion {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            complete(callback, Err(closed()));
        }
    }
}
fn settle_on_drop(callback: Completion) -> Completion {
    let pending = PendingCompletion(Some(callback));
    Box::new(move |value| pending.finish(value))
}
fn replaced() -> Failure {
    Failure::new(FailureKind::Replaced, "The document or page was replaced; the request was not applied")
}
fn closed() -> Failure {
    Failure::new(FailureKind::Closed, "Document is closed")
}
fn poisoned() -> Failure {
    Failure::new(FailureKind::Invalidated, "Engine panic: reload durable state")
}
/// Runs storage work; a panic in it fails as a poisoned engine.
fn contained<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| Err(poisoned()))
}
/// The request's callback, which the request answers or hands on exactly once.
fn take(callback: &mut Option<Completion>) -> Completion {
    callback.take().expect("a request's callback is taken once")
}

/// A queue handle, independent of any page lifetime. Its store retains the writer lock
/// until a successful close; a failed save never releases it.
pub struct Owner {
    admission: Arc<admission::Admission>,
    sync_admission: Arc<std::sync::Mutex<actor::Admission>>,
    sender: mpsc::Sender<Message>,
    store: Arc<store::Store>,
    path: PathBuf,
    started: Instant,
}
impl Owner {
    pub fn open(path: &Path, mode: store::Mode, listener: Listener) -> Result<Self> {
        Self::open_with_evaluator(path, mode, listener, None)
    }
    pub fn open_with_evaluator(
        path: &Path,
        mode: store::Mode,
        listener: Listener,
        evaluator: Option<Evaluator>,
    ) -> Result<Self> {
        let path = file::resolve(path)?;
        let store = Arc::new(store::Store::open(&path, mode)?);
        Self::from_store(path, store, mode, listener, evaluator)
    }
    pub(crate) fn from_store(
        path: PathBuf,
        store: Arc<store::Store>,
        mode: store::Mode,
        listener: Listener,
        evaluator: Option<Evaluator>,
    ) -> Result<Self> {
        let (sender, started, admission) = match actor::spawn(store.clone(), mode, listener, evaluator) {
            Ok(started) => started,
            Err(failure) => {
                // No actor admitted work. A worker may still hold an Arc while its
                // channel shuts down; release ownership before returning the failure.
                store.close()?;
                return Err(failure);
            }
        };
        Ok(Self { sender, store, path, started, sync_admission: Arc::default(), admission })
    }
    /// Serial admission for host actions, including immutable page configuration.
    /// The host still fences the WebView when the asynchronous result arrives.
    pub(crate) fn reserve_page(&self, view_bytes: usize, payload_bytes: usize) -> Result<Permit> {
        self.admission.reserve_page(view_bytes, payload_bytes).ok_or_else(busy)
    }
    pub(crate) fn admit_page(&self, view: String, permit: Permit, callback: Completion) {
        let callback = admitted_callback(callback, permit);
        if let Err(mpsc::SendError(Message::PageAdmission { callback, .. })) =
            self.sender.send(Message::PageAdmission { view, callback })
        {
            complete(callback, Err(closed()));
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn app(&self) -> &file::OpenedApp {
        self.store.app()
    }
    pub fn resource_reader(&self) -> Result<file::ResourceReader> {
        Ok(self.store.resource_reader()?)
    }
    pub fn attach(&self, view: String) {
        let _ = self.sender.send(Message::Attach(view));
    }
    pub fn publish_discovery(&self, json: &str) -> Result<()> {
        Ok(self.store.publish_discovery(json)?)
    }
    pub fn withdraw_discovery(&self) {
        self.store.withdraw_discovery();
    }
    /// Enqueue without waiting. Exhausted request count or payload budgets answer Busy
    /// without applying the request; lifecycle requests have reserved admission capacity.
    pub fn submit(&self, request: Request, view: Option<String>, callback: Completion) {
        self.enqueue(request, view, None, callback);
    }
    /// The deadline is checked on the edit worker, before the command is admitted.
    pub fn submit_until(&self, request: Request, view: Option<String>, deadline: Instant, callback: Completion) {
        self.enqueue(request, view, Some(deadline.saturating_duration_since(self.started)), callback);
    }
    fn enqueue(&self, request: Request, view: Option<String>, deadline: Option<Duration>, callback: Completion) {
        let Some(permit) = self.admission.reserve(&request, &view) else {
            complete(callback, Err(busy()));
            return;
        };
        let callback = admitted_callback(callback, permit);
        if let Err(mpsc::SendError(Message::Request { callback, .. })) =
            self.sender.send(Message::Request { request, view, deadline, callback })
        {
            complete(callback, Err(closed()));
        }
    }
}
fn busy() -> Failure {
    Failure::new(FailureKind::Busy, "Pending document work exceeds its admission budget")
}
fn admitted_callback(callback: Completion, permit: Permit) -> Completion {
    settle_on_drop(Box::new(move |result| {
        drop(permit);
        callback(result);
    }))
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
    }
}
