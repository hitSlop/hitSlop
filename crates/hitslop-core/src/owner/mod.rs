//! One document owner. Admission, publication order and save scheduling live on
//! the edit worker; SQLite work runs serially on the persistence worker. Callbacks run
//! without locks and may enqueue another request, but must not wait for its completion.
mod actor;
mod failure;
use crate::file::{self, Artwork};
use crate::{Origin, store, theme};
#[cfg(target_arch = "wasm32")]
pub use actor::BrowserDriver;
use actor::Message;
pub use actor::{SyncConnection, SyncEvent, SyncSink};
pub use failure::{Failure, FailureKind};
#[cfg(not(target_arch = "wasm32"))]
pub use hitslop_runner::Evaluator;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

type Result<T> = std::result::Result<T, Failure>;
/// Native theme controls use this typed adapter to the same batch path as page edits.
pub enum ThemeChange {
    Set { values: std::collections::HashMap<String, String> },
    ResetAll,
    ImportFile { file: String },
}
pub fn theme_request(change: ThemeChange) -> Request {
    let intent = match change {
        ThemeChange::Set { values } => {
            crate::Intent::SetTheme { values: values.into_iter().map(|(k, v)| (k, Some(v))).collect(), replace: None }
        }
        ThemeChange::ResetAll => crate::Intent::SetTheme { values: Default::default(), replace: Some(true) },
        ThemeChange::ImportFile { file } => crate::Intent::ImportTheme { file },
    };
    Request::Apply { batch: crate::Batch { intents: vec![intent], if_version: None }, origin: Origin::Window }
}

pub enum Request {
    /// The document as it reads (`Document::reading`), and its publication sequence.
    State,
    #[cfg(not(target_arch = "wasm32"))]
    MergeSnapshot {
        bytes: Vec<u8>,
    },
    Apply {
        batch: crate::Batch,
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
    Discard,
    Close {
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
    Artwork {
        name: Artwork,
    },
    /// Derived images are admitted only for the saved state they were rendered from.
    #[cfg(not(target_arch = "wasm32"))]
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
    /// Shares the document through a room: from then on it keeps its whole history and
    /// every attachment, and the host may attach a connection to the room's relay.
    Share {
        share: store::Share,
    },
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
        reading: crate::Reading,
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
        text: Option<crate::TextEdit>,
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
    sync_admission: Arc<std::sync::Mutex<actor::Admission>>,
    sender: mpsc::Sender<Message>,
    store: Arc<store::Store>,
    path: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    epoch: Instant,
}
impl Owner {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path, mode: store::Mode, listener: Listener) -> Result<Self> {
        Self::open_with_evaluator(path, mode, listener, None)
    }
    #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_store(
        path: PathBuf,
        store: Arc<store::Store>,
        mode: store::Mode,
        listener: Listener,
        evaluator: Option<Evaluator>,
    ) -> Result<Self> {
        let (sender, epoch) = match actor::spawn(store.clone(), mode, listener, evaluator) {
            Ok(started) => started,
            Err(failure) => {
                // No actor admitted work. A worker may still hold an Arc while its
                // channel shuts down; release ownership before returning the failure.
                store.close()?;
                return Err(failure);
            }
        };
        Ok(Self { sender, store, path, epoch, sync_admission: Arc::default() })
    }
    /// Serial admission for host actions, including immutable page configuration.
    /// The host still fences the WebView when the asynchronous result arrives.
    pub(crate) fn admit_page(&self, view: String, callback: Completion) {
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
    #[cfg(not(target_arch = "wasm32"))]
    pub fn resource_reader(&self) -> Result<file::ResourceReader> {
        Ok(self.store.resource_reader()?)
    }
    pub fn attach(&self, view: String) {
        let _ = self.sender.send(Message::Attach(view));
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn publish_discovery(&self, json: &str) -> Result<()> {
        Ok(self.store.publish_discovery(json)?)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn withdraw_discovery(&self) {
        self.store.withdraw_discovery();
    }
    pub fn submit(&self, request: Request, view: Option<String>, callback: Completion) {
        self.enqueue(request, view, None, callback);
    }
    /// The deadline is checked on the edit worker, before the command is admitted.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn submit_until(&self, request: Request, view: Option<String>, deadline: Instant, callback: Completion) {
        self.enqueue(request, view, Some(deadline.saturating_duration_since(self.epoch)), callback);
    }
    fn enqueue(&self, request: Request, view: Option<String>, deadline: Option<Duration>, callback: Completion) {
        if let Err(mpsc::SendError(Message::Request { callback, .. })) =
            self.sender.send(Message::Request { request, view, deadline, callback })
        {
            complete(callback, Err(closed()));
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
    }
}
