//! Serial admission and dispatch. All sibling modules below operate on this one state.
use super::*;
use crate::{Code, Document};
mod barrier;
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod commands;
mod persistence;
mod saving;
mod sync;
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserDriver;
#[cfg(not(target_arch = "wasm32"))]
use commands::{Evaluation, Invocation, Work as EvaluationWork};
#[cfg(target_arch = "wasm32")]
use persistence::perform;
#[cfg(not(target_arch = "wasm32"))]
use persistence::persistence;
use persistence::{StorageAction, Work};
use sync::{Link, SyncMessage};
pub use sync::{SyncEvent, SyncSink};

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn spawn(
    store: Arc<store::Store>,
    mode: store::Mode,
    listener: Listener,
    evaluator: Option<Evaluator>,
) -> Result<(mpsc::Sender<Message>, Instant)> {
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
    let mut actor = Actor::new(store, mode, listener, persist)?;
    actor.evaluate = evaluate;
    std::thread::Builder::new()
        .name("hitslop.owner".into())
        .spawn(move || actor.run(receive, epoch))
        .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
    Ok((sender, epoch))
}

pub(super) enum Message {
    #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(not(target_arch = "wasm32"))]
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
    ) -> Result<Self> {
        let core = store.document()?;
        let sequence = core.sequence();
        Ok(Self {
            now: Duration::ZERO,
            unix_ms: 0,
            #[cfg(not(target_arch = "wasm32"))]
            evaluate: None,
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
            #[cfg(not(target_arch = "wasm32"))]
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
            #[cfg(target_arch = "wasm32")]
            Request::Command { .. } => {
                return Err(Failure::rejected(Code::EngineError, "This hitSlop has no command evaluator"));
            }
            #[cfg(not(target_arch = "wasm32"))]
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
}

#[cfg(not(target_arch = "wasm32"))]
fn native_unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
