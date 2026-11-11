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
fn theme_batch(change: ThemeChange) -> pop_contracts::wire::Batch {
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
    pop_contracts::wire::Batch { intents: vec![intent], if_version: None }
}
/// One document reading and its publication sequence.
#[derive(Debug)]
pub struct Reading {
    pub reading: pop_contracts::wire::Reading,
    pub sequence: u64,
}
/// A version probe that does not materialize the document value.
#[derive(Debug)]
pub struct Version {
    pub version: String,
    pub sequence: u64,
}
/// An accepted edit and its publication sequence.
#[derive(Debug)]
pub struct Applied {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub text: Option<pop_core::TextEdit>,
}
/// An evaluated and accepted command.
#[derive(Debug)]
pub struct CommandResult {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub result_json: String,
}
/// The palette and its publication sequence.
#[derive(Debug)]
pub struct Theme {
    pub state: theme::ThemeState,
    pub sequence: u64,
}
/// Optional session fence and admission deadline for one operation.
#[derive(Debug, Default)]
pub struct CallOptions {
    pub view: Option<String>,
    pub deadline: Option<Instant>,
}
mod operation;
use operation::Operation;
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
pub type Completion<T> = Box<dyn FnOnce(Result<T>) + Send>;
pub type Listener = Arc<dyn Fn(Event) + Send + Sync>;
fn complete<T>(callback: Completion<T>, value: Result<T>) {
    let _ = catch_unwind(AssertUnwindSafe(|| callback(value)));
}
/// A queued request must settle even when shutdown drops its mailbox or worker job.
/// Taking the callback before invocation also prevents a panicking caller from answering twice.
struct PendingCompletion<T>(Option<Completion<T>>);
impl<T> PendingCompletion<T> {
    fn finish(mut self, value: Result<T>) {
        let callback = self.0.take().expect("pending completion is answered once");
        complete(callback, value);
    }
}
impl<T> Drop for PendingCompletion<T> {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            complete(callback, Err(closed()));
        }
    }
}
fn settle_on_drop<T: Send + 'static>(callback: Completion<T>) -> Completion<T> {
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
fn take<T>(callback: &mut Option<Completion<T>>) -> Completion<T> {
    callback.take().expect("a request's callback is taken once")
}

/// A queue handle, independent of any page lifetime. Its store retains the writer lock
/// until a successful close; a failed save never releases it.
pub struct Owner {
    admission: Arc<admission::Admission>,
    sync_admission: Arc<std::sync::Mutex<actor::Admission>>,
    sender: mpsc::Sender<Message>,
    app: Arc<file::OpenedApp>,
    resources: store::ResourceSource,
    thread: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
    path: PathBuf,
    started: Instant,
    evaluator: Option<Evaluator>,
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
        let store = store::Store::open(&path, mode)?;
        Self::from_store(path, store, mode, listener, evaluator)
    }
    fn from_store(
        path: PathBuf,
        store: store::Store,
        mode: store::Mode,
        listener: Listener,
        evaluator: Option<Evaluator>,
    ) -> Result<Self> {
        let app = store.opened_app();
        let resources = store.resource_source();
        let actor::Started { sender, started, admission, thread } =
            actor::spawn(store, mode, listener, evaluator.clone())?;
        Ok(Self {
            evaluator,
            sender,
            app,
            resources,
            thread: std::sync::Mutex::new(Some(thread)),
            path,
            started,
            sync_admission: Arc::default(),
            admission,
        })
    }
    pub(crate) fn evaluator(&self) -> Option<&Evaluator> {
        self.evaluator.as_ref()
    }
    fn storage_control<T>(
        &self,
        control: impl FnOnce(mpsc::Sender<Result<T>>) -> actor::persistence::Control,
    ) -> Result<T> {
        let (reply, result) = mpsc::channel();
        self.sender.send(Message::Control(control(reply))).map_err(|_| closed())?;
        result.recv().map_err(|_| closed())?
    }
    /// Serial admission for host actions, including immutable page configuration.
    /// The host still fences the WebView when the asynchronous result arrives.
    pub(crate) fn reserve_page(&self, view_bytes: usize, payload_bytes: usize) -> Result<Permit> {
        self.admission.reserve_page(view_bytes, payload_bytes).ok_or_else(busy)
    }
    pub(crate) fn admit_page(&self, view: String, permit: Permit, callback: Completion<()>) {
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
        &self.app
    }
    pub fn resource_reader(&self) -> Result<file::ResourceReader> {
        Ok(self.resources.reader()?)
    }
    pub fn attach(&self, view: String) {
        let _ = self.sender.send(Message::Attach(view));
    }
    pub fn publish_discovery(&self, json: &str) -> Result<()> {
        self.storage_control(|reply| actor::persistence::Control::Publish(json.to_owned(), reply))
    }
    pub fn withdraw_discovery(&self) {
        let _ = self.storage_control(actor::persistence::Control::Withdraw);
    }
    fn enqueue(&self, operation: Operation, options: CallOptions) {
        let bytes = operation.payload_bytes().saturating_add(options.view.as_ref().map_or(0, String::len));
        let Some(permit) = self.admission.reserve(bytes, operation.properties().control) else {
            operation.reject(busy());
            return;
        };
        let operation = operation.admitted(permit);
        let deadline = options.deadline.map(|d| d.saturating_duration_since(self.started));
        let _ = self.sender.send(Message::Request { operation, view: options.view, deadline });
    }
    /// Native palette changes follow the same edit path as page batches.
    pub fn change_theme(&self, change: ThemeChange, options: CallOptions, callback: Completion<Applied>) {
        self.apply(theme_batch(change), Origin::Window, options, callback);
    }
}
fn busy() -> Failure {
    Failure::new(FailureKind::Busy, "Pending document work exceeds its admission budget")
}
fn admitted_callback<T: Send + 'static>(callback: Completion<T>, permit: Permit) -> Completion<T> {
    settle_on_drop(Box::new(move |result| {
        drop(permit);
        callback(result);
    }))
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(thread) = crate::lock(&self.thread).take() {
            // A callback can release its final handle on the owner thread itself.
            if thread.thread().id() != std::thread::current().id() {
                let _ = thread.join();
            }
        }
    }
}
