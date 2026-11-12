//! Serial admission and dispatch. All sibling modules below operate on this one state.
use super::*;
use pop_contracts::wire::Code;
use pop_core::Document;
mod barrier;
mod commands;
pub(super) mod persistence;
mod saving;
mod sync;
use commands::{Evaluation, Invocation, Work as EvaluationWork};
use persistence::{Work, persistence};
pub(super) use sync::Admission;
use sync::{Link, SyncMessage};
pub use sync::{ReceivedFrame, SyncConnection, SyncEvent, SyncSink};

pub(super) struct Started {
    pub sender: mpsc::Sender<Message>,
    pub started: Instant,
    pub admission: Arc<super::admission::Admission>,
    pub thread: std::thread::JoinHandle<()>,
}

pub(super) fn spawn(
    store: store::Store,
    mode: store::Mode,
    listener: Listener,
    evaluator: Option<Evaluator>,
) -> Result<Started> {
    let (sender, receive) = mpsc::channel();
    let (persist, work) = mpsc::channel();
    let mut actor = Actor::new(&store, mode, listener, persist)?;
    let admission = actor.admission.clone();
    let completion = sender.clone();
    actor.workers.push(
        std::thread::Builder::new()
            .name("pop.persistence".into())
            .spawn(move || persistence(store, work, completion))
            .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?,
    );
    // Capture-only hosts never allocate an authored-command worker.
    if let Some(evaluator) = evaluator.filter(Evaluator::supports_commands) {
        let (evaluate, evaluations) = mpsc::channel();
        let completion = sender.clone();
        let worker = std::thread::Builder::new().name("pop.commands".into()).spawn(move || {
            commands::worker(|abi, bundle, input| evaluator.run(abi, bundle, input), evaluations, completion)
        });
        match worker {
            Ok(worker) => {
                actor.workers.push(worker);
                actor.evaluate = Some(evaluate);
            }
            Err(error) => {
                drop(actor);
                return Err(Failure::new(FailureKind::Failed, error.to_string()));
            }
        }
    }
    let started = Instant::now();
    let thread = std::thread::Builder::new()
        .name("pop.owner".into())
        .spawn(move || {
            let done = actor.run(receive, started);
            if let Some((callback, result)) = done {
                complete(callback, result);
            }
        })
        .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
    Ok(Started { sender, started, admission, thread })
}

pub(super) enum Message {
    Evaluated { invocation: Invocation, result: Result<Evaluation> },
    Request { operation: Operation, view: Option<String>, deadline: Option<Duration> },
    Attach(String),
    Sync(SyncMessage),
    PageAdmission { view: String, callback: Completion<()> },
    Persisted(persistence::Persisted),
    Control(persistence::Control),
    Stop,
}
#[derive(PartialEq, Eq)]
enum Lifecycle {
    Open,
    Closing,
    /// A handoff is writing its copy: nothing more is saved, and edits are refused.
    Releasing,
    Closed,
}
type SavedWork = Box<dyn FnOnce(&mut Actor, Completion<()>) + Send>;
enum AfterSave {
    Reply,
    Storage(SavedWork),
    Close { preview: Option<Vec<u8>>, icon: Option<Vec<u8>> },
}
struct Waiter {
    target: u64,
    callback: Completion<()>,
    next: AfterSave,
}
struct Actor {
    admission: Arc<super::admission::Admission>,
    /// Driver-supplied time; shared admission and save scheduling never read an OS clock.
    now: Duration,
    unix_ms: u64,
    evaluate: Option<mpsc::Sender<EvaluationWork>>,
    evaluating: bool,
    core: Document,
    workers: Vec<std::thread::JoinHandle<()>>,
    app: Arc<file::OpenedApp>,
    digest: pop_contracts::AppDigest,
    save_state: store::SaveState,
    finished: Option<(Completion<()>, Result<()>)>,
    mode: store::Mode,
    /// Fences session-bound work when Discard reloads saved state. Committed storage
    /// operations retain their actual outcomes across that boundary.
    generation: u64,
    persist: mpsc::Sender<Work>,
    listener: Listener,
    view: Option<String>,
    lifecycle: Lifecycle,
    invalidated: bool,
    discarding: bool,
    pending_membership: usize,
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
    fn new(store: &store::Store, mode: store::Mode, listener: Listener, persist: mpsc::Sender<Work>) -> Result<Self> {
        let core = store.open_document()?;
        let sequence = core.sequence();
        Ok(Self {
            admission: Arc::default(),
            now: Duration::ZERO,
            unix_ms: 0,
            evaluate: None,
            evaluating: false,
            core,
            workers: vec![],
            mode,
            app: store.opened_app(),
            digest: store.app_digest()?,
            save_state: store.save_state(),
            finished: None,
            generation: 0,
            persist,
            listener,
            view: None,
            lifecycle: Lifecycle::Open,
            invalidated: false,
            discarding: false,
            pending_membership: 0,
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
        self.begin_sync_step();
        if self.deadline.is_some_and(|deadline| deadline <= self.now) {
            self.deadline = None;
            self.requested = true;
            self.pump();
        }
        self.report_durable_sync();
    }
    fn wake(&self) -> Option<Duration> {
        self.deadline
    }
    fn step(&mut self, message: Message) -> bool {
        self.begin_sync_step();
        match message {
            Message::Stop => return false,
            Message::Control(control) => {
                let _ = self.persist.send(Work::Control(control));
            }
            Message::Evaluated { invocation, result } => self.command_finished(invocation, result),
            Message::Attach(view) => self.view = Some(view),
            Message::Sync(message) => self.sync(message),
            Message::PageAdmission { view, callback } => complete(callback, self.admit(false, Some(&view))),
            Message::Request { operation, view, deadline } => self.dispatch(operation, view, deadline),
            Message::Persisted(persistence::Persisted { generation, state, outcome }) => {
                self.save_state = state;
                match outcome {
                    persistence::Outcome::Saved { target, result } => {
                        if generation != self.generation {
                            return true;
                        }
                        self.writing = false;
                        match result {
                            Ok(()) => {
                                self.saved = self.saved.max(target);
                                self.failure = None;
                                self.status(if self.revision > self.saved {
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
                    persistence::Outcome::Restored { result, callback } => {
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
                                self.reconnect_after_reload();
                                complete(callback, Ok(()));
                            }
                            Err(error) => {
                                self.fail(error.clone(), u64::MAX);
                                complete(callback, Err(error));
                            }
                        }
                    }
                    persistence::Outcome::Stored { mut result, callback, closing, refresh_status } => {
                        if closing
                            && let Err(failure) = &mut result
                            && failure.kind == FailureKind::Failed
                        {
                            failure.kind = FailureKind::SaveFailed;
                        }
                        if refresh_status {
                            self.pending_membership -= 1;
                        }
                        if generation != self.generation {
                            // Storage has already completed. A view/session replacement cannot
                            // turn an applied attachment, copy or membership write into a refusal.
                            complete(callback, result);
                            return true;
                        }
                        if closing {
                            self.lifecycle = if result.is_ok() { Lifecycle::Closed } else { Lifecycle::Open };
                            if result.is_err() && self.revision > self.saved && !self.writing {
                                // Edits the refused request had fenced off are saved again, or fail visibly.
                                self.deadline = Some(self.now);
                            }
                        }
                        if refresh_status && result.is_ok() {
                            self.status(if self.revision > self.saved {
                                SaveStatus::Saving
                            } else {
                                SaveStatus::Saved
                            });
                        }
                        if closing && result.is_ok() {
                            self.finished = Some((callback, result));
                        } else {
                            complete(callback, result);
                        }
                    }
                }
            }
        }
        self.report_durable_sync();
        true
    }
    fn shutdown(&mut self) {
        self.reject_waiters(closed());
        self.fail_held(closed());
    }
    fn run(mut self, messages: mpsc::Receiver<Message>, started: Instant) -> Option<(Completion<()>, Result<()>)> {
        self.start();
        loop {
            self.clock(started.elapsed(), native_unix_ms());
            self.tick();
            let message = match self.wake() {
                Some(wake) => match messages.recv_timeout(wake.saturating_sub(started.elapsed())) {
                    Ok(message) => message,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                },
                None => match messages.recv() {
                    Ok(message) => message,
                    Err(_) => break,
                },
            };
            self.clock(started.elapsed(), native_unix_ms());
            if !self.step(message) || self.lifecycle == Lifecycle::Closed {
                break;
            }
        }
        // Disconnect first: queued completion guards settle, and callbacks that submit
        // more work get a closed reply instead of extending a shutdown drain.
        drop(messages);
        self.shutdown();
        self.finished.take()
    }
    fn dispatch(&mut self, operation: Operation, view: Option<String>, deadline: Option<Duration>) {
        let policy = operation.properties();
        let admission = if deadline.is_some_and(|deadline| deadline <= self.now) {
            Err(Failure::new(FailureKind::Closing, "Request expired before admission"))
        } else {
            self.admit(policy.recover, view.as_deref()).and_then(|()| {
                if policy.edits && self.barrier.closing() {
                    Err(Failure::new(FailureKind::Closing, "Document is closing"))
                } else {
                    Ok(())
                }
            })
        };
        if let Err(error) = admission {
            operation.reject(error);
            return;
        }
        if self.barrier.precedes(&operation) || (self.evaluating && policy.fence) {
            self.hold(operation, view, deadline);
            return;
        }
        self.request(operation, view, deadline);
    }
    fn answer<T>(
        &mut self,
        callback: Completion<T>,
        work: impl FnOnce(&mut Self, &mut Option<Completion<T>>) -> Result<Option<T>>,
    ) {
        let mut callback = Some(callback);
        let result = catch_unwind(AssertUnwindSafe(|| work(self, &mut callback))).unwrap_or_else(|_| {
            self.invalidated = true;
            self.fail(poisoned(), u64::MAX);
            Err(poisoned())
        });
        if let Some(callback) = callback {
            complete(callback, result.map(|value| value.expect("immediate operation has a result")));
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
    fn retain_derived<T: Send + 'static>(
        &self,
        bytes: usize,
        control: bool,
        callback: &mut Option<Completion<T>>,
    ) -> Result<()> {
        let lease = self.admission.reserve_bytes(bytes, control).ok_or_else(busy)?;
        let original = take(callback);
        *callback = Some(Box::new(move |result| {
            drop(lease);
            original(result);
        }));
        Ok(())
    }
    fn request(&mut self, operation: Operation, view: Option<String>, deadline: Option<Duration>) {
        match operation {
            Operation::Command { name, args_json, origin, callback } => {
                self.answer(callback, move |actor, callback| {
                    actor.begin_command(name, args_json, origin, view, deadline, callback)?;
                    Ok(None)
                })
            }
            Operation::State { callback } => self.answer(callback, move |actor, _| {
                Ok(Some(Reading { reading: actor.core.reading()?, sequence: actor.core.sequence() }))
            }),
            Operation::Apply { batch, origin, callback } => self.answer(callback, move |actor, _| {
                actor.mutation()?;
                let result = actor.edited(|core| core.apply_batch(batch, origin))?;
                actor.accepted(result.sequence, result.publication, result.theme_changed);
                Ok(Some(Applied { sequence: result.sequence, ids: result.ids, text: result.text }))
            }),
            Operation::Undo { callback } => self.answer(callback, move |actor, _| {
                actor.mutation()?;
                let result = actor.edited(|core| core.undo())?;
                actor.accepted(result.sequence, result.publication, result.theme_changed);
                Ok(Some(Applied { sequence: result.sequence, ids: result.ids, text: None }))
            }),
            Operation::Redo { callback } => self.answer(callback, move |actor, _| {
                actor.mutation()?;
                let result = actor.edited(|core| core.redo())?;
                actor.accepted(result.sequence, result.publication, result.theme_changed);
                Ok(Some(Applied { sequence: result.sequence, ids: result.ids, text: None }))
            }),
            Operation::Theme { callback } => self.answer(callback, move |actor, _| {
                Ok(Some(Theme { state: actor.core.theme_state()?, sequence: actor.core.sequence() }))
            }),
            Operation::Flush { callback } => self.answer(callback, move |actor, callback| {
                actor.wait(callback, AfterSave::Reply)?;
                Ok(None)
            }),
            Operation::ExportTheme { callback } => self.answer(callback, move |actor, callback| {
                let json = actor.core.export_theme()?;
                actor.wait_reply(callback, json)?;
                Ok(None)
            }),
            Operation::Copy { destination, preview, icon, callback } => {
                self.answer(callback, move |actor, callback| {
                    if actor.discarding {
                        return Err(replaced());
                    }
                    let state = actor.core.current_history_checkpoint()?;
                    actor.retain_derived(state.len(), false, callback)?;
                    actor.storage(
                        false,
                        move |store| {
                            store.copy_clean(&destination, &state, &persistence::named_artwork(&preview, &icon))?;
                            Ok(())
                        },
                        take(callback),
                    );
                    Ok(None)
                })
            }
            Operation::CaptureSource { destination, callback } => self.answer(callback, move |actor, callback| {
                actor.wait_storage(callback, move |store| {
                    store.capture_source(&destination)?;
                    Ok(())
                })?;
                Ok(None)
            }),
            Operation::Backup { destination, callback } => self.answer(callback, move |actor, callback| {
                actor.wait_storage(callback, move |store| {
                    store.backup(&destination)?;
                    Ok(())
                })?;
                Ok(None)
            }),
            Operation::StageSeed { destination, callback } => self.answer(callback, move |actor, callback| {
                actor.mutation()?;
                actor.wait_storage(callback, move |store| {
                    store.stage_seed(&destination)?;
                    Ok(())
                })?;
                Ok(None)
            }),
            Operation::SetArtwork { source_version, preview, icon, callback } => {
                self.answer(callback, move |actor, callback| {
                    actor.mutation()?;
                    if actor.core.version() != source_version {
                        return Err(Failure::new(FailureKind::Replaced, "Artwork source changed"));
                    }
                    actor.wait_storage(callback, move |store| {
                        store.set_artwork(&persistence::named_artwork(&preview, &icon))?;
                        Ok(())
                    })?;
                    Ok(None)
                })
            }
            Operation::Close { preview, icon, callback } => self.answer(callback, move |actor, callback| {
                if actor.lifecycle != Lifecycle::Open {
                    return Err(Failure::new(FailureKind::Closing, "Document is closing"));
                }
                if actor.discarding {
                    return Err(replaced());
                }
                actor.lifecycle = Lifecycle::Closing;
                actor.deadline = None;
                let (preview, icon) = if actor.mode == store::Mode::Document { (preview, icon) } else { (None, None) };
                actor.wait(callback, AfterSave::Close { preview, icon })?;
                Ok(None)
            }),
            Operation::Handoff { dest, preview, icon, callback } => self.answer(callback, move |actor, callback| {
                actor.mutation()?;
                let reading = actor.core.reading()?;
                let state = actor.core.current_history_checkpoint()?;
                let bytes = state
                    .len()
                    .saturating_add(serialized_size(&reading.value))
                    .saturating_add(serialized_size(&reading.theme));
                actor.retain_derived(bytes, true, callback)?;
                actor.lifecycle = Lifecycle::Releasing;
                actor.deadline = None;
                actor.requested = false;
                actor.persist(Work::Handoff {
                    generation: actor.generation,
                    dest,
                    state,
                    preview,
                    icon,
                    value: reading.value,
                    theme: reading.theme,
                    callback: take(callback),
                });
                Ok(None)
            }),
            Operation::Discard { callback } => self.answer(callback, move |actor, callback| {
                if actor.discarding || actor.lifecycle != Lifecycle::Open {
                    return Err(replaced());
                }
                if actor.pending_membership != 0 {
                    return Err(busy());
                }
                // Recovery reloads durable state after a fault; it never retracts edits
                // already replicated. Ordinary shared documents still cannot discard.
                if actor.save_state.is_shared() && !actor.invalidated {
                    return Err(Failure::rejected(
                        Code::InvalidRequest,
                        "A shared document keeps its edits: other copies may already have them",
                    ));
                }
                actor.generation += 1;
                actor.discarding = true;
                actor.clear_sync_for_restore();
                actor.deadline = None;
                actor.reject_waiters(replaced());
                actor.persist(Work::Restore { generation: actor.generation, callback: take(callback) });
                Ok(None)
            }),
            Operation::Artwork { name, callback } => self.answer(callback, move |actor, callback| {
                actor.storage(false, move |store| Ok(store.artwork(name)?), take(callback));
                Ok(None)
            }),
            Operation::Attachments { callback } => self.answer(callback, move |actor, callback| {
                actor.storage(false, move |store| Ok(store.attachments()?), take(callback));
                Ok(None)
            }),
            Operation::ReadAttachment { id, callback } => self.answer(callback, move |actor, callback| {
                actor.storage(false, move |store| Ok(store.attachment(&id)?), take(callback));
                Ok(None)
            }),
            Operation::PutAttachment { bytes, callback } => self.answer(callback, move |actor, callback| {
                actor.mutation()?;
                actor.storage(false, move |store| Ok(store.put_attachment(&bytes)?), take(callback));
                Ok(None)
            }),
            Operation::Share { share, callback } => self.answer(callback, move |actor, callback| {
                actor.mutation()?;
                actor.storage(
                    true,
                    move |store| {
                        store.set_share(&share)?;
                        Ok(())
                    },
                    take(callback),
                );
                Ok(None)
            }),
            Operation::Unshare { callback } => self.answer(callback, move |actor, callback| {
                actor.mutation()?;
                actor.storage(
                    true,
                    move |store| {
                        store.clear_share()?;
                        Ok(())
                    },
                    take(callback),
                );
                Ok(None)
            }),
            Operation::Version { callback } => self.answer(callback, |actor, _| {
                Ok(Some(Version { version: actor.core.version(), sequence: actor.core.sequence() }))
            }),
        }
    }
}

fn native_unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

impl Drop for Actor {
    fn drop(&mut self) {
        self.evaluate = None;
        let (unused, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.persist, unused));
        // Input channels close before joining, so workers drain and release the store.
        self.workers.drain(..).for_each(|worker| {
            let _ = worker.join();
        });
    }
}
