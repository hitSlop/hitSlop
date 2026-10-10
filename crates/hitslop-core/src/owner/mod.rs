//! One document owner. Admission, publication order and save scheduling live on
//! the edit worker; SQLite work runs serially on the persistence worker. Callbacks run
//! without locks and may enqueue another request, but must not wait for its completion.
use crate::file::{self, Artwork};
mod failure;
pub use failure::{Failure, FailureKind};
mod persistence;
use crate::{Code, Document, Origin, store, theme};
#[cfg(target_arch = "wasm32")]
use persistence::perform;
#[cfg(not(target_arch = "wasm32"))]
use persistence::persistence;
use persistence::{StorageAction, Work};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserDriver;
mod barrier;
mod commands;
mod sync;
use commands::{Evaluation, Invocation, Work as EvaluationWork};
#[cfg(not(target_arch = "wasm32"))]
pub use hitslop_runner::Evaluator;
use sync::{Link, SyncMessage};
pub use sync::{SyncEvent, SyncSink};

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
    /// The saved document, copied after a flush as a document of its own
    /// (`Store::copy_clean`), with the artwork its window rendered for it.
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
    Publication { json: String },
    SaveStatus { status: SaveStatus, failure: Option<Failure> },
    UndoState { can_undo: bool, can_redo: bool },
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
    mode: store::Mode,
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
        let (sender, receive) = mpsc::channel();
        let (persist, work) = mpsc::channel();
        let completion = sender.clone();
        let disk = store.clone();
        std::thread::Builder::new()
            .name("hitslop.persistence".into())
            .spawn(move || persistence(disk, work, completion))
            .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
        // Without an evaluator there is no worker: commands are refused when called.
        let evaluate = match evaluator {
            Some(evaluator) => {
                let (evaluate, evaluations) = mpsc::channel();
                let completion = sender.clone();
                std::thread::Builder::new()
                    .name("hitslop.commands".into())
                    .spawn(move || commands::worker(evaluator, evaluations, completion))
                    .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
                Some(evaluate)
            }
            None => None,
        };
        let epoch = Instant::now();
        let actor = Actor::new(store.clone(), mode, listener, persist, evaluate)?;
        std::thread::Builder::new()
            .name("hitslop.owner".into())
            .spawn(move || actor.run(receive, epoch))
            .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
        Ok(Self { sender, store, path, mode, epoch })
    }
    pub fn mode(&self) -> store::Mode {
        self.mode
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

enum Message {
    Evaluated {
        invocation: Invocation,
        result: Result<Evaluation>,
    },
    Request {
        request: Request,
        view: Option<String>,
        deadline: Option<Duration>,
        callback: Completion,
    },
    Attach(String),
    Sync(SyncMessage),
    PageAdmission {
        view: String,
        callback: Completion,
    },
    Saved {
        generation: u64,
        target: u64,
        result: Result<()>,
    },
    /// Boxed: a document is far larger than any other message.
    Restored {
        generation: u64,
        result: Result<Box<Document>>,
        callback: Completion,
    },
    Stored {
        generation: u64,
        result: Result<Reply>,
        callback: Completion,
        closing: bool,
    },
    Stop,
}
#[derive(PartialEq, Eq)]
enum Lifecycle {
    Open,
    Closing,
    Closed,
}
enum AfterSave {
    Reply(Reply),
    Storage(StorageAction),
    Close { preview: Option<Vec<u8>>, icon: Option<Vec<u8>> },
}
struct Waiter {
    target: u64,
    callback: Completion,
    next: AfterSave,
}
struct Actor {
    /// Driver-supplied time; shared admission and save scheduling never read an OS clock.
    now: Duration,
    unix_ms: u64,
    evaluate: Option<mpsc::Sender<EvaluationWork>>,
    evaluating: bool,
    core: Document,
    store: Arc<store::Store>,
    mode: store::Mode,
    /// Advances when a discard reloads saved state, so work begun before it is dropped.
    generation: u64,
    persist: mpsc::Sender<Work>,
    listener: Listener,
    view: Option<String>,
    lifecycle: Lifecycle,
    invalidated: bool,
    discarding: bool,
    /// The publication sequence the page orders its stream by.
    sequence: u64,
    /// Save progress, independent of the sequence: `revision` counts accepted changes
    /// that need saving (a publication, an installed checkpoint) and `saved` the revision
    /// the file covers. Waiters wait for `saved` to reach the revision they were admitted at.
    saved: u64,
    revision: u64,
    writing: bool,
    requested: bool,
    deadline: Option<Duration>,
    unsaved_since: Option<Duration>,
    failure: Option<Failure>,
    waiters: Vec<Waiter>,
    undo: (bool, bool),
    barrier: barrier::Barrier,
    link: Link,
}
impl Actor {
    fn new(
        store: Arc<store::Store>,
        mode: store::Mode,
        listener: Listener,
        persist: mpsc::Sender<Work>,
        evaluate: Option<mpsc::Sender<EvaluationWork>>,
    ) -> Result<Self> {
        let core = store.document()?;
        let sequence = core.sequence();
        Ok(Self {
            now: Duration::ZERO,
            unix_ms: 0,
            evaluate,
            evaluating: false,
            core,
            mode,
            store,
            generation: 0,
            persist,
            listener,
            view: None,
            lifecycle: Lifecycle::Open,
            invalidated: false,
            discarding: false,
            sequence,
            saved: 0,
            revision: 0,
            writing: false,
            requested: false,
            deadline: None,
            unsaved_since: None,
            failure: None,
            waiters: vec![],
            undo: (false, false),
            barrier: barrier::Barrier::default(),
            link: Link::default(),
        })
    }
    fn emit(&self, event: Event) {
        let _ = catch_unwind(AssertUnwindSafe(|| (self.listener)(event)));
    }
    fn start(&self) {
        self.emit(Event::UndoState { can_undo: false, can_redo: false });
        self.status(SaveStatus::Saved);
    }
    fn clock(&mut self, now: Duration, unix_ms: u64) {
        self.now = self.now.max(now);
        self.unix_ms = unix_ms;
    }
    fn tick(&mut self) {
        if self.deadline.is_some_and(|deadline| deadline <= self.now) {
            self.deadline = None;
            self.requested = true;
            self.pump();
        }
    }
    fn wake(&self) -> Option<Duration> {
        self.deadline
    }
    fn step(&mut self, message: Message) -> bool {
        match message {
            Message::Stop => return false,
            Message::Evaluated { invocation, result } => self.command_finished(invocation, result),
            Message::Attach(view) => self.view = Some(view),
            Message::Sync(message) => self.sync(message),
            Message::PageAdmission { view, callback } => {
                complete(callback, self.admit(false, Some(&view)).map(|()| Reply::Unit))
            }
            Message::Request { request, view, deadline, callback } => self.dispatch(request, view, deadline, callback),
            Message::Saved { generation, target, result } => {
                if generation != self.generation {
                    return true;
                }
                self.writing = false;
                match result {
                    Ok(()) => {
                        self.saved = self.saved.max(target);
                        self.failure = None;
                        self.status(if self.revision > self.saved { SaveStatus::Saving } else { SaveStatus::Saved });
                        self.settle();
                    }
                    Err(error) => self.fail(error, target),
                }
                if self.requested || !self.waiters.is_empty() {
                    self.pump();
                }
            }
            Message::Restored { generation, result, callback } => {
                if generation != self.generation {
                    complete(callback, Err(replaced()));
                    return true;
                }
                self.discarding = false;
                self.writing = false;
                match result {
                    Ok(core) => {
                        self.core = *core;
                        self.sequence = self.core.sequence();
                        self.revision += 1;
                        self.saved = self.revision;
                        self.invalidated = false;
                        self.view = None;
                        self.requested = false;
                        self.unsaved_since = None;
                        self.failure = None;
                        self.status(SaveStatus::Saved);
                        self.refresh_undo();
                        self.emit(Event::ThemeChanged);
                        complete(callback, Ok(Reply::Unit));
                    }
                    Err(error) => {
                        self.fail(error.clone(), u64::MAX);
                        complete(callback, Err(error));
                    }
                }
            }
            Message::Stored { generation, mut result, callback, closing } => {
                if closing
                    && let Err(failure) = &mut result
                    && failure.kind == FailureKind::Failed
                {
                    failure.kind = FailureKind::SaveFailed;
                }
                if generation != self.generation {
                    complete(callback, Err(replaced()));
                    return true;
                }
                if closing {
                    self.lifecycle = if result.is_ok() { Lifecycle::Closed } else { Lifecycle::Open };
                }
                complete(callback, result);
            }
        }
        true
    }
    fn shutdown(&mut self) {
        self.reject_waiters(closed());
        self.fail_held(closed());
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn run(mut self, messages: mpsc::Receiver<Message>, epoch: Instant) {
        self.start();
        loop {
            self.clock(epoch.elapsed(), native_unix_ms());
            self.tick();
            let message = match self.wake() {
                Some(wake) => match messages.recv_timeout(wake.saturating_sub(epoch.elapsed())) {
                    Ok(message) => message,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                },
                None => match messages.recv() {
                    Ok(message) => message,
                    Err(_) => break,
                },
            };
            self.clock(epoch.elapsed(), native_unix_ms());
            if !self.step(message) {
                break;
            }
        }
        self.shutdown();
    }
    /// Admits one request, answering its callback unless the request handed it on. A
    /// panic poisons the owner, as any engine panic does.
    fn dispatch(&mut self, request: Request, view: Option<String>, deadline: Option<Duration>, callback: Completion) {
        if deadline.is_some_and(|deadline| deadline <= self.now) {
            complete(callback, Err(Failure::new(FailureKind::Closing, "Request expired before admission")));
            return;
        }
        let mut callback = Some(callback);
        let result = catch_unwind(AssertUnwindSafe(|| self.request(request, view, deadline, &mut callback)))
            .unwrap_or_else(|_| {
                self.invalidated = true;
                self.fail(poisoned(), u64::MAX);
                Err(poisoned())
            });
        if let Some(callback) = callback {
            complete(callback, result.map(|value| value.unwrap_or(Reply::Unit)));
        }
    }
    fn admit(&self, recover: bool, view: Option<&str>) -> Result<()> {
        if self.lifecycle == Lifecycle::Closed {
            return Err(closed());
        }
        if !recover && self.invalidated {
            return Err(poisoned());
        }
        if view.is_some_and(|v| Some(v) != self.view.as_deref()) {
            return Err(replaced());
        }
        Ok(())
    }
    /// An edit whose refusal could not be rolled back poisons the owner like a panic.
    pub(crate) fn edited<T>(&mut self, edit: impl FnOnce(&mut Document) -> crate::Result<T>) -> Result<T> {
        edit(&mut self.core).map_err(|e| {
            if self.core.is_broken() {
                self.invalidated = true;
                poisoned()
            } else {
                e.into()
            }
        })
    }
    fn mutation(&self) -> Result<()> {
        if self.mode != store::Mode::Document {
            return Err(Failure::new(FailureKind::ReadOnly, "Read-only document"));
        }
        if self.lifecycle != Lifecycle::Open {
            return Err(Failure::new(FailureKind::Closing, "Document is closing"));
        }
        if self.discarding {
            return Err(replaced());
        }
        Ok(())
    }
    fn request(
        &mut self,
        request: Request,
        view: Option<String>,
        deadline: Option<Duration>,
        callback: &mut Option<Completion>,
    ) -> Result<Option<Reply>> {
        self.admit(matches!(request, Request::Discard), view.as_deref())?;
        if barrier::edits(&request) && self.barrier.closing() {
            return Err(Failure::new(FailureKind::Closing, "Document is closing"));
        }
        if self.barrier.precedes(&request) || (self.evaluating && barrier::fence(&request)) {
            self.hold(request, view, deadline, callback)?;
            return Ok(None);
        }
        let reply = match request {
            Request::Command { name, args_json, origin } => {
                self.begin_command(name, args_json, origin, view, deadline, callback)?;
                return Ok(None);
            }
            Request::State => Reply::State { reading: self.core.reading()?, sequence: self.core.sequence() },
            Request::Apply { batch, origin } => {
                self.mutation()?;
                let result = self.edited(|core| core.apply_batch(batch, origin))?;
                self.accepted(result.sequence, result.publication, result.theme_changed);
                Reply::Applied { sequence: result.sequence, ids: result.ids, text: result.text }
            }
            Request::Undo { redo } => {
                self.mutation()?;
                let result = self.edited(|core| if redo { core.redo() } else { core.undo() })?;
                self.accepted(result.sequence, result.publication, result.theme_changed);
                Reply::Applied { sequence: result.sequence, ids: result.ids, text: None }
            }
            Request::Theme => Reply::Theme { state: self.core.theme_state()?, sequence: self.core.sequence() },
            Request::Flush => {
                self.wait(callback, AfterSave::Reply(Reply::Unit))?;
                return Ok(None);
            }
            Request::ExportTheme => {
                let json = self.core.export_theme()?;
                self.wait(callback, AfterSave::Reply(Reply::ThemeFile { json }))?;
                return Ok(None);
            }
            Request::Copy { destination, preview, icon } => {
                self.wait(callback, AfterSave::Storage(StorageAction::Copy { path: destination, preview, icon }))?;
                return Ok(None);
            }
            Request::CaptureSource { destination } => {
                self.wait(callback, AfterSave::Storage(StorageAction::CaptureSource(destination)))?;
                return Ok(None);
            }
            Request::Backup { destination } => {
                self.wait(callback, AfterSave::Storage(StorageAction::Backup(destination)))?;
                return Ok(None);
            }
            Request::Close { preview, icon } => {
                if self.lifecycle != Lifecycle::Open {
                    return Err(Failure::new(FailureKind::Closing, "Document is closing"));
                }
                if self.discarding {
                    return Err(replaced());
                }
                self.lifecycle = Lifecycle::Closing;
                self.deadline = None;
                let (preview, icon) = if self.mode == store::Mode::Document { (preview, icon) } else { (None, None) };
                self.wait(callback, AfterSave::Close { preview, icon })?;
                return Ok(None);
            }
            Request::Discard => {
                if self.discarding || self.lifecycle == Lifecycle::Closing {
                    return Err(replaced());
                }
                if self.store.is_shared() {
                    return Err(Failure::rejected(
                        Code::InvalidRequest,
                        "A shared document keeps its edits: other copies may already have them",
                    ));
                }
                self.generation += 1;
                self.discarding = true;
                self.deadline = None;
                self.reject_waiters(replaced());
                self.persist(Work::Restore { generation: self.generation, callback: take(callback) });
                return Ok(None);
            }
            Request::Artwork { name } => {
                self.storage(StorageAction::Artwork(name), take(callback));
                return Ok(None);
            }
            Request::Attachments => {
                self.storage(StorageAction::Attachments, take(callback));
                return Ok(None);
            }
            Request::ReadAttachment { id } => {
                self.storage(StorageAction::ReadAttachment(id), take(callback));
                return Ok(None);
            }
            Request::PutAttachment { bytes } => {
                self.mutation()?;
                self.storage(StorageAction::PutAttachment(bytes), take(callback));
                return Ok(None);
            }
            Request::Share { share } => {
                self.mutation()?;
                self.storage(StorageAction::Share(share), take(callback));
                return Ok(None);
            }
        };
        Ok(Some(reply))
    }
    fn accepted(&mut self, sequence: u64, publication: Option<String>, themed: bool) {
        self.sync_push();
        self.refresh_undo();
        let Some(json) = publication else {
            return;
        };
        let was_saved = self.revision <= self.saved;
        self.revision += 1;
        self.sequence = sequence;
        self.emit(Event::Publication { json });
        if themed {
            self.emit(Event::ThemeChanged);
        }
        if was_saved {
            self.status(SaveStatus::Saving);
        }
        let now = self.now;
        let since = *self.unsaved_since.get_or_insert(now);
        self.deadline = Some((now + Duration::from_millis(150)).min(since + Duration::from_millis(1000)));
    }
    fn refresh_undo(&mut self) {
        let next = (self.core.can_undo(), self.core.can_redo());
        if next != self.undo {
            self.undo = next;
            self.emit(Event::UndoState { can_undo: next.0, can_redo: next.1 });
        }
    }
    fn status(&self, status: SaveStatus) {
        self.emit(Event::SaveStatus {
            status: if self.failure.is_some() { SaveStatus::Failed } else { status },
            failure: self.failure.clone(),
        });
    }
    fn wait(&mut self, callback: &mut Option<Completion>, next: AfterSave) -> Result<()> {
        if self.discarding {
            return Err(replaced());
        }
        self.waiters.push(Waiter { target: self.revision, callback: take(callback), next });
        self.pump();
        Ok(())
    }
    fn pump(&mut self) {
        if self.writing || self.discarding || self.invalidated || self.lifecycle == Lifecycle::Closed {
            return;
        }
        if self.revision <= self.saved {
            self.settle();
            return;
        }
        self.requested = false;
        self.unsaved_since = None;
        self.deadline = None;
        let job = catch_unwind(AssertUnwindSafe(|| self.store.job(&mut self.core, false).map_err(Failure::from)))
            .unwrap_or_else(|_| {
                self.invalidated = true;
                Err(poisoned())
            });
        match job {
            Ok(Some(job)) => {
                self.writing = true;
                self.persist(Work::Save { generation: self.generation, target: self.revision, job });
            }
            Ok(None) => {
                self.saved = self.revision;
                self.failure = None;
                self.status(SaveStatus::Saved);
                self.settle();
            }
            Err(error) => self.fail(error, u64::MAX),
        }
    }
    fn settle(&mut self) {
        let waiters = std::mem::take(&mut self.waiters);
        for waiter in waiters {
            if waiter.target > self.saved {
                self.waiters.push(waiter);
                continue;
            }
            match waiter.next {
                AfterSave::Reply(reply) => complete(waiter.callback, Ok(reply)),
                AfterSave::Storage(action) => self.storage(action, waiter.callback),
                AfterSave::Close { preview, icon } => {
                    let job = if self.mode == store::Mode::Document {
                        catch_unwind(AssertUnwindSafe(|| self.store.close_job(&mut self.core)))
                            .ok()
                            .and_then(|r| r.ok())
                            .flatten()
                    } else {
                        None
                    };
                    self.persist(Work::Close {
                        generation: self.generation,
                        job,
                        preview,
                        icon,
                        callback: waiter.callback,
                    });
                }
            }
        }
    }
    fn storage(&mut self, action: StorageAction, callback: Completion) {
        self.persist(Work::Store { generation: self.generation, action, callback });
    }
    fn persist(&mut self, work: Work) {
        if let Err(mpsc::SendError(work)) = self.persist.send(work) {
            self.writing = false;
            self.discarding = false;
            if self.lifecycle == Lifecycle::Closing {
                self.lifecycle = Lifecycle::Open;
            }
            self.fail(poisoned(), u64::MAX);
            match work {
                Work::Store { callback, .. } | Work::Restore { callback, .. } | Work::Close { callback, .. } => {
                    complete(callback, Err(poisoned()))
                }
                Work::Save { .. } => {}
            }
        }
    }
    fn fail(&mut self, mut failure: Failure, target: u64) {
        if failure.kind == FailureKind::Failed {
            failure.kind = FailureKind::SaveFailed;
        }
        if failure.kind == FailureKind::Invalidated {
            self.invalidated = true;
        }
        self.failure = Some(failure.clone());
        self.status(SaveStatus::Failed);
        let waiters = std::mem::take(&mut self.waiters);
        for waiter in waiters {
            if waiter.target > target {
                self.waiters.push(waiter);
                continue;
            }
            if matches!(waiter.next, AfterSave::Close { .. }) {
                self.lifecycle = Lifecycle::Open;
            }
            complete(waiter.callback, Err(failure.clone()));
        }
    }
    fn reject_waiters(&mut self, error: Failure) {
        for waiter in std::mem::take(&mut self.waiters) {
            complete(waiter.callback, Err(error.clone()));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn native_unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
