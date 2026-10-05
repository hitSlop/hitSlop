//! One native document owner. Admission, publication order and save scheduling live on
//! the edit worker; SQLite work runs serially on the persistence worker. Callbacks run
//! without locks and may enqueue another request, but must not wait for its completion.
use crate::{file, store, theme, Document, Origin};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Rejected,
    Replaced,
    Closing,
    Closed,
    ReadOnly,
    Invalidated,
    Locked,
    Busy,
    Full,
    Moved,
    SaveFailed,
    Failed,
}
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
    pub reason: Option<String>,
    pub op_index: Option<u32>,
}
impl Failure {
    fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            reason: None,
            op_index: None,
        }
    }
}
impl From<crate::Error> for Failure {
    fn from(e: crate::Error) -> Self {
        Self {
            kind: FailureKind::Rejected,
            reason: Some(e.code.as_str().into()),
            message: e.message,
            op_index: e.op_index.map(|n| n as u32),
        }
    }
}
impl From<store::Error> for Failure {
    fn from(e: store::Error) -> Self {
        let kind = match &e {
            store::Error::Rejected(_) => {
                if let store::Error::Rejected(e) = e {
                    return e.into();
                }
                unreachable!()
            }
            store::Error::Locked => FailureKind::Locked,
            store::Error::Busy => FailureKind::Busy,
            store::Error::Full => FailureKind::Full,
            store::Error::Moved => FailureKind::Moved,
            store::Error::Closed => FailureKind::Closed,
            store::Error::Failed(_) => FailureKind::Failed,
        };
        Self::new(kind, e.to_string())
    }
}
type Result<T> = std::result::Result<T, Failure>;
pub enum Request {
    State,
    Apply {
        batch_json: String,
        origin: Origin,
    },
    Undo {
        redo: bool,
    },
    Flush,
    Discard,
    Close {
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    /// The saved document, copied after a flush: `durable` for a copy a person keeps,
    /// not for a capture's disposable source.
    Copy {
        destination: PathBuf,
        durable: bool,
    },
    Artwork {
        name: String,
    },
    Attachments,
    ReadAttachment {
        id: String,
    },
    PutAttachment {
        bytes: Vec<u8>,
    },
    /// The palette; changes are batch intents.
    Theme,
    ExportTheme,
}
#[derive(Debug)]
pub enum Reply {
    Unit,
    State {
        json: String,
    },
    /// `version` is the document's after the change; `text` answers the page's text edit.
    Applied {
        sequence: u64,
        ids: Vec<String>,
        version: String,
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
    Publication {
        json: String,
    },
    SaveStatus {
        status: SaveStatus,
        failure: Option<Failure>,
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
    Failure::new(
        FailureKind::Replaced,
        "The document or page was replaced; the request was not applied",
    )
}
fn closed() -> Failure {
    Failure::new(FailureKind::Closed, "Document is closed")
}
fn poisoned() -> Failure {
    Failure::new(
        FailureKind::Invalidated,
        "Engine panic: reload durable state",
    )
}

/// A queue handle, independent of any page lifetime. Its store retains the writer lock
/// until a successful close; a failed save never releases it.
pub struct Owner {
    sender: mpsc::Sender<Message>,
    store: Arc<store::Store>,
    path: PathBuf,
}
impl Owner {
    pub fn open(path: &Path, mode: store::Mode, listener: Listener) -> Result<Self> {
        let path = file::resolve(path)?;
        let store = Arc::new(store::Store::open(&path, mode)?);
        let core = store.document()?;
        let sequence = core.sequence();
        let (sender, receive) = mpsc::channel();
        let (persist, work) = mpsc::channel();
        let completion = sender.clone();
        let disk = store.clone();
        std::thread::Builder::new()
            .name("hitslop.persistence".into())
            .spawn(move || persistence(disk, work, completion))
            .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
        let actor = Actor {
            core,
            mode,
            store: store.clone(),
            generation: 0,
            persist,
            listener,
            view: None,
            lifecycle: Lifecycle::Open,
            invalidated: false,
            discarding: false,
            sequence,
            saved: sequence,
            writing: false,
            requested: false,
            deadline: None,
            unsaved_since: None,
            failure: None,
            waiters: vec![],
            undo: (false, false),
        };
        std::thread::Builder::new()
            .name("hitslop.owner".into())
            .spawn(move || actor.run(receive))
            .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
        Ok(Self {
            sender,
            store,
            path,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn app(&self) -> &file::OpenedApp {
        self.store.app()
    }
    pub fn asset_reader(&self) -> Result<file::AssetReader> {
        Ok(self.store.asset_reader()?)
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
    pub fn submit(&self, request: Request, view: Option<String>, callback: Completion) {
        self.enqueue(request, view, None, callback);
    }
    /// The deadline is checked on the edit worker, before the command is admitted.
    pub fn submit_until(&self, request: Request, view: Option<String>, deadline: Instant, callback: Completion) {
        self.enqueue(request, view, Some(deadline), callback);
    }
    fn enqueue(
        &self,
        request: Request,
        view: Option<String>,
        deadline: Option<Instant>,
        callback: Completion,
    ) {
        if let Err(mpsc::SendError(Message::Request { callback, .. })) =
            self.sender.send(Message::Request {
                request,
                view,
                deadline,
                callback,
            })
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
    Request {
        request: Request,
        view: Option<String>,
        deadline: Option<Instant>,
        callback: Completion,
    },
    Attach(String),
    Saved {
        generation: u64,
        target: u64,
        result: Result<()>,
    },
    Restored {
        generation: u64,
        result: Result<Document>,
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
enum Work {
    Save {
        generation: u64,
        target: u64,
        job: store::SaveJob,
    },
    Restore {
        generation: u64,
        callback: Completion,
    },
    Store {
        generation: u64,
        action: StorageAction,
        callback: Completion,
    },
    Close {
        generation: u64,
        job: Option<store::SaveJob>,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        callback: Completion,
    },
}
enum StorageAction {
    Copy(PathBuf, bool),
    Artwork(String),
    Attachments,
    ReadAttachment(String),
    PutAttachment(Vec<u8>),
}
fn persistence(
    store: Arc<store::Store>,
    work: mpsc::Receiver<Work>,
    sender: mpsc::Sender<Message>,
) {
    for work in work {
        let done = match work {
            Work::Save { generation, target, job } => Message::Saved {
                generation,
                target,
                result: catch_unwind(AssertUnwindSafe(|| {
                    store.write(&job).map_err(Failure::from)
                }))
                .unwrap_or_else(|_| Err(poisoned())),
            },
            Work::Restore { generation, callback } => Message::Restored {
                generation,
                callback,
                result: catch_unwind(AssertUnwindSafe(|| store.document().map_err(Failure::from)))
                    .unwrap_or_else(|_| Err(poisoned())),
            },
            Work::Store {
                generation,
                action,
                callback,
            } => {
                let result = catch_unwind(AssertUnwindSafe(|| -> Result<Reply> {
                    Ok(match action {
                        StorageAction::Copy(path, durable) => {
                            store.copy_to(&path, durable)?;
                            Reply::Unit
                        }
                        StorageAction::Artwork(name) => Reply::Bytes {
                            bytes: store.artwork(&name)?,
                        },
                        StorageAction::Attachments => Reply::Attachments {
                            items: store.attachments()?,
                        },
                        StorageAction::ReadAttachment(id) => Reply::Bytes {
                            bytes: Some(store.attachment(&id)?),
                        },
                        StorageAction::PutAttachment(bytes) => Reply::Attachment {
                            item: store.put_attachment(&bytes)?,
                        },
                    })
                }))
                .unwrap_or_else(|_| Err(poisoned()));
                Message::Stored {
                    generation,
                    result,
                    callback,
                    closing: false,
                }
            }
            Work::Close {
                generation,
                job,
                preview,
                icon,
                callback,
            } => {
                let result = catch_unwind(AssertUnwindSafe(|| -> Result<Reply> {
                    // Housekeeping cannot fail a close whose final save succeeded.
                    if let Some(job) = job {
                        let _ = store.write(&job);
                    }
                    let mut artwork = vec![];
                    if let Some(ref bytes) = preview {
                        artwork.push(("preview", bytes.as_slice()));
                    }
                    if let Some(ref bytes) = icon {
                        artwork.push(("icon", bytes.as_slice()));
                    }
                    if !artwork.is_empty() {
                        let _ = store.set_artwork(&artwork);
                    }
                    store.close()?;
                    Ok(Reply::Unit)
                }))
                .unwrap_or_else(|_| Err(poisoned()));
                Message::Stored {
                    generation,
                    result,
                    callback,
                    closing: true,
                }
            }
        };
        if let Err(mpsc::SendError(message)) = sender.send(done) {
            match message {
                Message::Stored { callback, .. } | Message::Restored { callback, .. } => {
                    complete(callback, Err(closed()))
                }
                _ => {}
            }
        }
    }
}
#[derive(PartialEq, Eq)]
enum Lifecycle {
    Open,
    Closing,
    Closed,
}
enum AfterSave {
    Reply(Reply),
    Copy(PathBuf, bool),
    Close {
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
}
struct Waiter {
    target: u64,
    callback: Completion,
    next: AfterSave,
}
struct Actor {
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
    sequence: u64,
    saved: u64,
    writing: bool,
    requested: bool,
    deadline: Option<Instant>,
    unsaved_since: Option<Instant>,
    failure: Option<Failure>,
    waiters: Vec<Waiter>,
    undo: (bool, bool),
}
impl Actor {
    fn emit(&self, event: Event) {
        let _ = catch_unwind(AssertUnwindSafe(|| (self.listener)(event)));
    }
    fn run(mut self, messages: mpsc::Receiver<Message>) {
        self.emit(Event::UndoState { can_undo: false, can_redo: false });
        self.status(SaveStatus::Saved);
        loop {
            if self
                .deadline
                .is_some_and(|deadline| deadline <= Instant::now())
            {
                self.deadline = None;
                self.requested = true;
                self.pump();
            }
            let message = match self.deadline {
                Some(deadline) => match messages
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                {
                    Ok(message) => message,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        self.deadline = None;
                        self.requested = true;
                        self.pump();
                        continue;
                    }
                    Err(_) => break,
                },
                None => match messages.recv() {
                    Ok(message) => message,
                    Err(_) => break,
                },
            };
            match message {
                Message::Stop => break,
                Message::Attach(view) => self.view = Some(view),
                Message::Request {
                    request,
                    view,
                    deadline,
                    callback,
                } => {
                    if deadline.is_some_and(|deadline| deadline <= Instant::now()) {
                        complete(
                            callback,
                            Err(Failure {
                                kind: FailureKind::Closing,
                                message: "Request expired before admission".into(),
                                reason: None,
                                op_index: None,
                            }),
                        );
                        continue;
                    }
                    let mut callback = Some(callback);
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        self.request(request, view, &mut callback)
                    }))
                    .unwrap_or_else(|_| {
                        self.invalidated = true;
                        self.fail(poisoned(), u64::MAX);
                        Err(poisoned())
                    });
                    if let Some(callback) = callback {
                        complete(callback, result.map(|value| value.unwrap_or(Reply::Unit)));
                    }
                }
                Message::Saved {
                    generation,
                    target,
                    result,
                } => {
                    if generation != self.generation {
                        continue;
                    }
                    self.writing = false;
                    match result {
                        Ok(()) => {
                            self.saved = self.saved.max(target);
                            self.failure = None;
                            self.status(if self.sequence > self.saved {
                                SaveStatus::Saving
                            } else {
                                SaveStatus::Saved
                            });
                            self.settle();
                        }
                        Err(error) => self.fail(error, target),
                    }
                    if self.requested || !self.waiters.is_empty() {
                        self.pump();
                    }
                }
                Message::Restored {
                    generation,
                    result,
                    callback,
                } => {
                    if generation != self.generation {
                        complete(callback, Err(replaced()));
                        continue;
                    }
                    self.discarding = false;
                    self.writing = false;
                    match result {
                        Ok(core) => {
                            self.core = core;
                            self.sequence = self.core.sequence();
                            self.saved = self.sequence;
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
                Message::Stored {
                    generation,
                    mut result,
                    callback,
                    closing,
                } => {
                    if closing {
                        if let Err(failure) = &mut result {
                            if failure.kind == FailureKind::Failed {
                                failure.kind = FailureKind::SaveFailed;
                            }
                        }
                    }
                    if generation != self.generation {
                        complete(callback, Err(replaced()));
                        continue;
                    }
                    if closing {
                        self.lifecycle = if result.is_ok() {
                            Lifecycle::Closed
                        } else {
                            Lifecycle::Open
                        };
                    }
                    complete(callback, result);
                }
            }
        }
        self.reject_waiters(closed());
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
        callback: &mut Option<Completion>,
    ) -> Result<Option<Reply>> {
        self.admit(matches!(request, Request::Discard), view.as_deref())?;
        let reply = match request {
            Request::State => Reply::State {
                json: self.core.state()?,
            },
            Request::Apply { batch_json, origin } => {
                self.mutation()?;
                let result = self.core.apply_batch(&batch_json, origin)?;
                self.accepted(result.sequence, result.publication, result.theme_changed);
                Reply::Applied {
                    sequence: result.sequence,
                    ids: result.ids,
                    version: self.core.version(),
                    text: result.text,
                }
            }
            Request::Undo { redo } => {
                self.mutation()?;
                let result = if redo {
                    self.core.redo()?
                } else {
                    self.core.undo()?
                };
                self.accepted(result.sequence, result.publication, result.theme_changed);
                Reply::Applied {
                    sequence: result.sequence,
                    ids: result.ids,
                    version: self.core.version(),
                    text: None,
                }
            }
            Request::Theme => Reply::Theme {
                state: self.core.theme_state()?,
                sequence: self.core.sequence(),
            },
            Request::Flush => {
                self.wait(callback, AfterSave::Reply(Reply::Unit))?;
                return Ok(None);
            }
            Request::ExportTheme => {
                let json = self.core.export_theme()?;
                self.wait(callback, AfterSave::Reply(Reply::State { json }))?;
                return Ok(None);
            }
            Request::Copy { destination, durable } => {
                self.wait(callback, AfterSave::Copy(destination, durable))?;
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
                let (preview, icon) = if self.mode == store::Mode::Document {
                    (preview, icon)
                } else {
                    (None, None)
                };
                self.wait(callback, AfterSave::Close { preview, icon })?;
                return Ok(None);
            }
            Request::Discard => {
                if self.discarding || self.lifecycle == Lifecycle::Closing {
                    return Err(replaced());
                }
                self.generation += 1;
                self.discarding = true;
                self.deadline = None;
                self.reject_waiters(replaced());
                self.persist(Work::Restore {
                    generation: self.generation,
                    callback: callback.take().unwrap(),
                });
                return Ok(None);
            }
            Request::Artwork { name } => {
                self.storage(StorageAction::Artwork(name), callback.take().unwrap());
                return Ok(None);
            }
            Request::Attachments => {
                self.storage(StorageAction::Attachments, callback.take().unwrap());
                return Ok(None);
            }
            Request::ReadAttachment { id } => {
                self.storage(StorageAction::ReadAttachment(id), callback.take().unwrap());
                return Ok(None);
            }
            Request::PutAttachment { bytes } => {
                self.mutation()?;
                self.storage(
                    StorageAction::PutAttachment(bytes),
                    callback.take().unwrap(),
                );
                return Ok(None);
            }
        };
        Ok(Some(reply))
    }
    fn accepted(&mut self, sequence: u64, publication: Option<String>, themed: bool) {
        self.refresh_undo();
        let Some(json) = publication else {
            return;
        };
        let was_saved = self.sequence <= self.saved;
        self.sequence = sequence;
        self.emit(Event::Publication { json });
        if themed {
            self.emit(Event::ThemeChanged);
        }
        if was_saved {
            self.status(SaveStatus::Saving);
        }
        let now = Instant::now();
        let since = *self.unsaved_since.get_or_insert(now);
        self.deadline =
            Some((now + Duration::from_millis(150)).min(since + Duration::from_millis(1000)));
    }
    fn refresh_undo(&mut self) {
        let next = (self.core.can_undo(), self.core.can_redo());
        if next != self.undo {
            self.undo = next;
            self.emit(Event::UndoState {
                can_undo: next.0,
                can_redo: next.1,
            });
        }
    }
    fn status(&self, status: SaveStatus) {
        self.emit(Event::SaveStatus {
            status: if self.failure.is_some() {
                SaveStatus::Failed
            } else {
                status
            },
            failure: self.failure.clone(),
        });
    }
    fn wait(&mut self, callback: &mut Option<Completion>, next: AfterSave) -> Result<()> {
        if self.discarding {
            return Err(replaced());
        }
        self.waiters.push(Waiter {
            target: self.sequence,
            callback: callback.take().unwrap(),
            next,
        });
        self.pump();
        Ok(())
    }
    fn pump(&mut self) {
        if self.writing
            || self.discarding
            || self.invalidated
            || self.lifecycle == Lifecycle::Closed
        {
            return;
        }
        if self.sequence <= self.saved {
            self.settle();
            return;
        }
        self.requested = false;
        self.unsaved_since = None;
        self.deadline = None;
        let job = catch_unwind(AssertUnwindSafe(|| {
            self.store.job(&mut self.core, false).map_err(Failure::from)
        }))
        .unwrap_or_else(|_| {
            self.invalidated = true;
            Err(poisoned())
        });
        match job {
            Ok(Some(job)) => {
                self.writing = true;
                self.persist(Work::Save {
                    generation: self.generation,
                    target: self.sequence,
                    job,
                });
            }
            Ok(None) => {
                self.saved = self.sequence;
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
                AfterSave::Copy(path, durable) => self.storage(StorageAction::Copy(path, durable), waiter.callback),
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
        self.persist(Work::Store {
            generation: self.generation,
            action,
            callback,
        });
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
                Work::Store { callback, .. }
                | Work::Restore { callback, .. }
                | Work::Close { callback, .. } => complete(callback, Err(poisoned())),
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
