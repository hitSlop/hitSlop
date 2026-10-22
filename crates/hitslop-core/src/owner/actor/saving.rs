//! Save scheduling and completion barriers for the single owner actor.
use super::*;

impl Actor {
    pub(super) fn accepted(&mut self, sequence: u64, publication: Option<String>, themed: bool) {
        self.sync_push();
        self.refresh_undo();
        let Some(json) = publication else {
            return;
        };
        let was_saved = self.revision <= self.saved;
        self.revision += 1;
        self.sequence = sequence;
        self.emit(Event::Publication { json: crate::wire::page::PagePush::publication_json(json) });
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
    pub(super) fn refresh_undo(&mut self) {
        let next = (self.core.can_undo(), self.core.can_redo());
        if next != self.undo {
            self.undo = next;
            self.emit(Event::UndoState { can_undo: next.0, can_redo: next.1 });
        }
    }
    pub(super) fn status(&self, status: SaveStatus) {
        self.emit(Event::SaveStatus {
            status: if self.failure.is_some() { SaveStatus::Failed } else { status },
            failure: self.failure.clone(),
            pressure: self.store.pressure(),
            shared: self.store.is_shared(),
        });
    }
    pub(super) fn wait(&mut self, callback: &mut Option<Completion>, next: AfterSave) -> Result<()> {
        if self.discarding {
            return Err(replaced());
        }
        self.waiters.push(Waiter { target: self.revision, callback: take(callback), next });
        self.pump();
        Ok(())
    }
    pub(super) fn pump(&mut self) {
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
        let job = catch_unwind(AssertUnwindSafe(|| self.store.job(&mut self.core).map_err(Failure::from)))
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
    pub(super) fn settle(&mut self) {
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
    pub(super) fn storage(&mut self, action: StorageAction, callback: Completion) {
        self.persist(Work::Store { generation: self.generation, action, callback });
    }
    pub(super) fn persist(&mut self, work: Work) {
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
    pub(super) fn fail(&mut self, mut failure: Failure, target: u64) {
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
    pub(super) fn reject_waiters(&mut self, error: Failure) {
        for waiter in std::mem::take(&mut self.waiters) {
            complete(waiter.callback, Err(error.clone()));
        }
    }
}
