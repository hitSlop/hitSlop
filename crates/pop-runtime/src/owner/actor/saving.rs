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
        self.emit(Event::Publication { json: pop_contracts::wire::page::PagePush::publication_json(json) });
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
            pressure: if self.failure.as_ref().is_some_and(|failure| failure.kind == FailureKind::Full) {
                store::Pressure::Full
            } else {
                self.save_state.pressure()
            },
            shared: self.save_state.is_shared(),
        });
    }
    pub(super) fn wait(&mut self, callback: &mut Option<Completion<()>>, next: AfterSave) -> Result<()> {
        self.can_wait()?;
        self.waiters.push(Waiter { target: self.revision, callback: take(callback), next });
        self.pump();
        Ok(())
    }
    pub(super) fn pump(&mut self) {
        if self.writing
            || self.discarding
            || self.invalidated
            || matches!(self.lifecycle, Lifecycle::Releasing | Lifecycle::Closed)
        {
            return;
        }
        if self.revision <= self.saved {
            self.settle();
            return;
        }
        self.requested = false;
        self.unsaved_since = None;
        self.deadline = None;
        let job = catch_unwind(AssertUnwindSafe(|| self.save_state.job(&mut self.core).map_err(Failure::from)))
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
                AfterSave::Reply => complete(waiter.callback, Ok(())),
                AfterSave::Storage(work) => work(self, waiter.callback),
                AfterSave::Close { preview, icon } => {
                    let job = if self.mode == store::Mode::Document {
                        catch_unwind(AssertUnwindSafe(|| {
                            self.save_state.close_job(&mut self.core, self.app.app.spec())
                        }))
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
    pub(super) fn wait_reply<T: Send + 'static>(
        &mut self,
        callback: &mut Option<Completion<T>>,
        value: T,
    ) -> Result<()> {
        self.can_wait()?;
        let callback = take(callback);
        let mut unit: Option<Completion<()>> = Some(Box::new(move |result| complete(callback, result.map(|()| value))));
        self.wait(&mut unit, AfterSave::Reply)
    }
    fn can_wait(&self) -> Result<()> {
        if self.discarding {
            return Err(replaced());
        }
        if self.lifecycle == Lifecycle::Releasing {
            return Err(Failure::new(FailureKind::Closing, "Document is closing"));
        }
        Ok(())
    }
    pub(super) fn wait_storage(
        &mut self,
        callback: &mut Option<Completion<()>>,
        work: impl FnOnce(&store::Store) -> Result<()> + Send + 'static,
    ) -> Result<()> {
        self.can_wait()?;
        self.wait(callback, AfterSave::Storage(Box::new(move |actor, callback| actor.storage(false, work, callback))))
    }

    pub(super) fn storage<T: Send + 'static>(
        &mut self,
        refresh_status: bool,
        work: impl FnOnce(&store::Store) -> Result<T> + Send + 'static,
        callback: Completion<T>,
    ) {
        self.pending_membership += usize::from(refresh_status);
        let job = persistence::StorageJob::new(refresh_status, work, callback);
        self.persist(Work::Store { generation: self.generation, job });
    }
    pub(super) fn persist(&mut self, work: Work) {
        if let Err(mpsc::SendError(work)) = self.persist.send(work) {
            self.writing = false;
            self.discarding = false;
            if matches!(self.lifecycle, Lifecycle::Closing | Lifecycle::Releasing) {
                self.lifecycle = Lifecycle::Open;
            }
            self.fail(poisoned(), u64::MAX);
            if matches!(&work, Work::Store { job, .. } if job.refresh_status) {
                self.pending_membership -= 1;
            }
            match work {
                Work::Restore { callback, .. } | Work::Close { callback, .. } => complete(callback, Err(poisoned())),
                Work::Handoff { callback, .. } => complete(callback, Err(poisoned())),
                Work::Store { job, .. } => job.reject(poisoned()),
                Work::Save { .. } | Work::Control(_) => {}
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
