//! Explicit epoch installation fences edits while the existing storage worker commits.
use super::*;

pub(in crate::owner) struct Installed {
    pub checkpoint: Vec<u8>,
    pub recovery_path: Option<store::EpochLocation>,
}
pub(in crate::owner) struct InstallFailure {
    pub failure: Failure,
    pub uncertain: bool,
}

impl Actor {
    pub(super) fn prepare_epoch(
        &mut self,
        destination: store::EpochLocation,
        expected: String,
        required_heads: Option<pop_sharing_protocol::lifecycle::WireVersion>,
        callback: &mut Option<Completion>,
    ) -> Result<()> {
        self.mutation()?;
        let source_version = self.core.version();
        if source_version != expected {
            return Err(Failure::rejected(Code::StaleBase, "The document changed before epoch preparation"));
        }
        let deep = self.core.supports_deep_compaction();
        if !deep && required_heads.is_some() {
            return Err(Failure::rejected(Code::InvalidRequest, "This layout preserves its existing relay history"));
        }
        if let Some(heads) = required_heads
            && !self.core.covers_wire_history(&heads)?
        {
            return Err(Failure::rejected(
                Code::MissingDependencies,
                "The candidate has not received the confirmed relay frontier",
            ));
        }
        let state = if deep { self.core.clean_checkpoint()? } else { self.core.current_history_checkpoint()? };
        self.retain_derived(state.capacity(), false, callback)?;
        self.storage(StorageAction::PrepareEpoch { destination, state, source_version }, take(callback));
        Ok(())
    }

    pub(super) fn adopt_epoch(
        &mut self,
        source: store::EpochLocation,
        expected: String,
        recovery_destination: Option<store::EpochLocation>,
        callback: &mut Option<Completion>,
    ) -> Result<()> {
        self.mutation()?;
        if self.evaluating || self.writing {
            return Err(busy());
        }
        let recovery = if self.core.version() != expected {
            let path = recovery_destination.ok_or_else(|| {
                Failure::rejected(
                    Code::StaleBase,
                    "Local edits need a recovery document before refreshing shared state",
                )
            })?;
            let state = self.core.clean_checkpoint()?;
            self.retain_derived(state.capacity(), true, callback)?;
            Some((path, state))
        } else {
            None
        };
        self.reject_waiters(Failure::new(
            FailureKind::Failed,
            "The shared authority is changing; refresh before another edit",
        ));
        self.generation += 1;
        self.lifecycle = Lifecycle::InstallingEpoch;
        self.deadline = None;
        self.unsaved_since = None;
        self.requested = false;
        self.persist(Work::AdoptEpoch { generation: self.generation, source, recovery, callback: take(callback) });
        Ok(())
    }

    pub(super) fn finish_epoch(
        &mut self,
        generation: u64,
        result: std::result::Result<Installed, InstallFailure>,
        callback: Completion,
    ) {
        if generation != self.generation {
            complete(callback, Err(replaced()));
            return;
        }
        self.lifecycle = Lifecycle::Open;
        let result = result.map_err(|failed| {
            self.invalidated |= failed.uncertain;
            failed.failure
        });
        let result = result.and_then(|installed| {
            let applied = self.core.adopt_checkpoint(&installed.checkpoint).map_err(|error| {
                self.invalidated = true;
                Failure::new(FailureKind::Invalidated, error.message)
            })?;
            self.sequence = applied.sequence;
            self.revision += 1;
            self.saved = self.revision;
            self.failure = None;
            self.refresh_undo();
            let replaced_history = applied.publication.is_some();
            if let Some(json) = applied.publication {
                self.emit(Event::Publication { json: crate::wire::page::PagePush::publication_json(json) });
                self.emit(Event::ThemeChanged);
            }
            self.status(SaveStatus::Saved);
            // A same-version acknowledgement preserves this session's undo. Its
            // deleted references still need their immutable blobs until close.
            if replaced_history {
                self.storage(StorageAction::ReclaimEpoch(self.core.version()), Box::new(|_| {}));
            }
            Ok(Reply::EpochAdopted { recovery_path: installed.recovery_path })
        });
        if let Err(error) = &result {
            self.fail(error.clone(), u64::MAX);
        }
        self.epoch_installed();
        complete(callback, result);
    }
}
