//! History maintenance for a local document. Loro keeps an open document's history in
//! memory, so a long session that edits a lot grows without bound even when its saved
//! checkpoint stays small. When a checkpoint measures more retained history than the
//! budget (`Store::owner_job`), the owner rebuilds its document from a shallow checkpoint
//! that keeps the supported undo window (`crate::maintenance`):
//!
//! - It waits for a quiet moment: editing paused for the budget's `rebuild_idle`, or 30 s
//!   after the rebuild fell due if edits keep coming, and every edit saved.
//! - While the persistence worker prepares, checks and writes the replacement, edits and
//!   closes wait behind the barrier; reads continue. Flushes and copies wait only when
//!   an earlier edit or command must finish first.
//! - The replacement is swapped in only after its checkpoint is written. A rebuild that
//!   fails changes nothing a person can see: the saved state already holds every edit,
//!   so it is not a save failure. The next attempt waits for more history.
//! - Discard cancels a rebuild in progress.
use super::*;

/// How long a due rebuild waits for editing to pause before it starts anyway.
const PATIENCE: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(super) struct Maintenance {
    /// When a rebuild fell due, and the retained history that made it due.
    due: Option<(Instant, usize)>,
    /// The retained history of the rebuild being prepared or written.
    running: Option<usize>,
    /// When the last edit was admitted.
    last_edit: Option<Instant>,
}

impl Actor {
    /// A checkpoint measured `size` bytes of retained history, past the budget.
    pub(super) fn rebuild_due(&mut self, size: usize) {
        if self.maintenance.due.is_none() && self.maintenance.running.is_none() {
            self.maintenance.due = Some((Instant::now(), size));
        }
    }
    pub(super) fn maintenance_edited(&mut self) {
        self.maintenance.last_edit = Some(Instant::now());
    }
    pub(super) fn maintenance_running(&self) -> bool {
        self.maintenance.running.is_some()
    }
    /// What a rebuild in progress holds back: edits, and a close, which would otherwise
    /// write its own checkpoint between the rebuild's write and its swap.
    pub(super) fn maintenance_holds(&self, request: &Request) -> bool {
        self.maintenance_running() && (barrier::edits(request) || matches!(request, Request::Close { .. }))
    }
    /// When a due rebuild may start, or none while something else must happen first; the
    /// message that ends that wait polls again.
    pub(super) fn maintenance_wake(&self) -> Option<Instant> {
        let (since, _) = self.maintenance.due?;
        let ready = self.maintenance.running.is_none()
            && self.mode == store::Mode::Document
            && self.lifecycle == Lifecycle::Open
            && !self.discarding
            && !self.invalidated
            && !self.writing
            && !self.evaluating
            && self.failure.is_none()
            && self.revision <= self.saved
            && self.session_rebuilds();
        if !ready {
            return None;
        }
        let quiet = self.maintenance.last_edit.map_or(since, |edit| edit + self.store.budget().rebuild_idle);
        Some(quiet.max(since).min(since + PATIENCE))
    }
    /// Starts a due rebuild once its wake time has passed.
    pub(super) fn maintenance_poll(&mut self) {
        if self.maintenance_wake().is_none_or(|wake| wake > Instant::now()) {
            return;
        }
        let Some((_, size)) = self.maintenance.due.take() else {
            return;
        };
        // Only immutable handles and history metadata: the expensive export, validation
        // and write run on the persistence worker while this core keeps serving reads.
        match contained(|| self.core.maintenance_seed().map_err(Failure::from)) {
            Ok(seed) => {
                self.maintenance.running = Some(size);
                let budget = self.store.budget().session_bytes;
                self.persist(Work::Maintain { generation: self.generation, seed, budget });
            }
            Err(error) => self.maintenance_failed(error, size),
        }
    }
    /// The persistence worker built, and tried to write, the replacement.
    pub(super) fn maintenance_finished(&mut self, generation: u64, result: Result<(Box<Document>, Result<()>)>) {
        if generation != self.generation {
            // A discard cancelled this rebuild and answered what it held.
            return;
        }
        let Some(size) = self.maintenance.running.take() else {
            return;
        };
        match result {
            Ok((rebuilt, Ok(()))) => {
                self.core = *rebuilt;
                self.sequence = self.core.sequence();
                self.refresh_undo();
            }
            Ok((rebuilt, Err(error))) => {
                self.core.keep_floor_of(&rebuilt);
                self.maintenance_failed(error, size);
            }
            Err(error) => self.maintenance_failed(error, size),
        }
        self.release_held();
    }
    /// A rebuild that could not be prepared or written leaves the live document as it was.
    /// Only a poisoned engine is a failure the window shows.
    fn maintenance_failed(&mut self, error: Failure, size: usize) {
        self.store.defer_rebuild(size);
        if error.kind == FailureKind::Invalidated {
            self.fail(error, u64::MAX);
        }
    }
    /// Discard drops a rebuild in progress; its result, if it arrives, is for an older
    /// generation. What it held was never applied.
    pub(super) fn maintenance_cancel(&mut self) {
        self.maintenance.due = None;
        self.maintenance.running = None;
        self.fail_held(replaced());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Budget, Mode};
    use std::sync::mpsc;

    fn wait<T>(receive: mpsc::Receiver<T>) -> T {
        receive.recv_timeout(Duration::from_secs(20)).expect("owner completion")
    }
    fn pair() -> (Completion, mpsc::Receiver<Result<Reply>>) {
        let (send, receive) = mpsc::channel();
        (
            Box::new(move |r| {
                let _ = send.send(r);
            }),
            receive,
        )
    }
    fn call(owner: &Owner, request: Request) -> Result<Reply> {
        let (callback, receive) = pair();
        owner.submit(request, None, callback);
        wait(receive)
    }
    fn set_text(text: &str) -> Request {
        Request::Apply {
            origin: Origin::Page,
            batch: crate::Batch::decode(
                &serde_json::json!({"intents":[{"type":"set","path":["text"],"value":text}]}).to_string(),
            )
            .unwrap(),
        }
    }
    /// Text that compresses poorly, so its history grows the snapshot.
    fn noise(seed: u64) -> String {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        (0..2000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                char::from(b'a' + (state % 26) as u8)
            })
            .collect()
    }
    fn value(owner: &Owner) -> serde_json::Value {
        let Reply::State { reading, .. } = call(owner, Request::State).unwrap() else { panic!() };
        serde_json::to_value(reading).unwrap()["value"].clone()
    }
    fn rebuilt_size(owner: &Owner) -> usize {
        owner.store.rebuilt_size()
    }
    /// A document whose every save checkpoints and whose history budget is tiny, so a few
    /// edits make a rebuild due.
    fn fixture(idle: Duration) -> (tempfile::TempDir, PathBuf, Owner, mpsc::Receiver<Event>) {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::testing::document(
            dir.path(),
            r#"{"kind":"object","properties":{"text":{"kind":"text"},"hits":{"kind":"counter"}}}"#,
            r#"{"text":"","hits":0}"#,
        );
        let (send, events) = mpsc::channel();
        let owner = Owner::open(
            &path,
            Mode::Document,
            Arc::new(move |event| {
                let _ = send.send(event);
            }),
        )
        .unwrap();
        owner.store.set_budget(Budget {
            checkpoint_rows: 1,
            checkpoint_bytes: 1,
            session_bytes: 4096,
            trim_bytes: 0,
            rebuild_idle: idle,
        });
        owner.attach("editor".into());
        (dir, path, owner, events)
    }
    /// Edits and saves until a rebuild has run.
    fn churn_until_rebuilt(owner: &Owner) {
        for round in 0..200 {
            call(owner, set_text(&noise(round))).unwrap();
            call(owner, Request::Flush).unwrap();
            if rebuilt_size(owner) > 0 {
                return;
            }
        }
        panic!("no rebuild ran");
    }
    fn failed_saves(events: &mpsc::Receiver<Event>) -> usize {
        events.try_iter().filter(|e| matches!(e, Event::SaveStatus { status: SaveStatus::Failed, .. })).count()
    }

    struct RebuildGate(mpsc::Sender<()>);
    impl Drop for RebuildGate {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    /// Stop a real rebuild before preparation or after its write. Dropping the gate
    /// always releases the worker, including when an assertion fails.
    fn pause_rebuild(owner: &Owner, after_write: bool, failure: Option<Failure>) -> RebuildGate {
        let (started, running) = mpsc::channel();
        let (resume, proceed) = mpsc::channel();
        *crate::lock(&owner.store.rebuild_hook) = Some(Box::new(move |written| {
            if written == after_write {
                let _ = started.send(());
                let _ = proceed.recv();
                if let Some(error) = &failure {
                    return Err(error.clone());
                }
            }
            Ok(())
        }));
        for round in 0..6 {
            call(owner, set_text(&noise(round))).unwrap();
            call(owner, Request::Flush).unwrap();
        }
        let mut budget = owner.store.budget();
        budget.rebuild_idle = Duration::ZERO;
        owner.store.set_budget(budget);
        call(owner, Request::State).unwrap(); // Wake the actor after changing its test budget.
        wait(running);
        RebuildGate(resume)
    }

    #[test]
    fn fences_include_edits_queued_during_a_rebuild() {
        let (_dir, path, owner, _events) = fixture(Duration::from_secs(60));
        let gate = pause_rebuild(&owner, false, None);
        // With no earlier edits a flush does not depend on optional maintenance.
        call(&owner, Request::Flush).unwrap();
        let (callback, edited) = pair();
        owner.submit(set_text("included"), None, callback);
        let (callback, flushed) = pair();
        owner.submit(Request::Flush, None, callback);
        let copy = path.with_file_name("ordered.slop");
        let (callback, copied) = pair();
        owner.submit(Request::Copy { destination: copy.clone(), preview: None, icon: None }, None, callback);
        call(&owner, Request::State).unwrap(); // Both requests have reached the owner.
        assert!(flushed.try_recv().is_err(), "flush must cover the earlier held edit");
        drop(gate);
        wait(edited).unwrap();
        wait(flushed).unwrap();
        wait(copied).unwrap();
        let saved = crate::store::Store::open(&copy, Mode::Snapshot).unwrap().document().unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&saved.value()).unwrap()["text"], "included");
        call(&owner, Request::Close { preview: None, icon: None }).unwrap();
    }

    #[test]
    fn a_rebuild_keeps_the_value_and_editing_and_undo_work_after_it() {
        let (_dir, path, owner, events) = fixture(Duration::ZERO);
        churn_until_rebuilt(&owner);
        let before = value(&owner);
        call(&owner, set_text("after")).unwrap();
        call(&owner, Request::Undo { redo: false }).unwrap();
        assert_eq!(value(&owner), before);
        call(&owner, Request::Undo { redo: true }).unwrap();
        assert_eq!(value(&owner)["text"], "after");
        assert_eq!(failed_saves(&events), 0);
        call(&owner, Request::Close { preview: None, icon: None }).unwrap();
        drop(owner);
        let saved = crate::store::Store::open(&path, Mode::Snapshot).unwrap().document().unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&saved.value()).unwrap()["text"], "after");
    }

    #[test]
    fn a_rebuild_waits_for_editing_to_pause() {
        let (_dir, _path, owner, _events) = fixture(Duration::from_millis(400));
        for round in 0..40 {
            call(&owner, set_text(&noise(round))).unwrap();
            call(&owner, Request::Flush).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(rebuilt_size(&owner), 0, "no rebuild while edits keep arriving");
        let deadline = Instant::now() + Duration::from_secs(10);
        while rebuilt_size(&owner) == 0 {
            assert!(Instant::now() < deadline, "the rebuild runs once editing pauses");
            std::thread::sleep(Duration::from_millis(20));
        }
        call(&owner, Request::Close { preview: None, icon: None }).unwrap();
    }

    #[test]
    fn a_failed_rebuild_is_not_a_save_failure_and_holds_nothing() {
        let (_dir, path, owner, events) = fixture(Duration::from_millis(300));
        // Lock the file, then make a rebuild due: its write fails once SQLite gives up
        // waiting, and the store records the deferral.
        let lock = rusqlite::Connection::open(&path).unwrap();
        for round in 0..6 {
            call(&owner, set_text(&noise(round))).unwrap();
            call(&owner, Request::Flush).unwrap();
        }
        lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while rebuilt_size(&owner) == 0 {
            assert!(Instant::now() < deadline, "the rebuild ran and failed");
            std::thread::sleep(Duration::from_millis(20));
        }
        lock.execute_batch("ROLLBACK").unwrap();
        drop(lock);
        // Nothing waits behind it, and no save failure was shown.
        call(&owner, set_text("still editing")).unwrap();
        call(&owner, Request::Flush).unwrap();
        assert_eq!(failed_saves(&events), 0);
        let copy = path.with_file_name("copy.slop");
        call(&owner, Request::Copy { destination: copy.clone(), preview: None, icon: None }).unwrap();
        call(&owner, Request::Close { preview: None, icon: None }).unwrap();
        drop(owner);
        for file in [&path, &copy] {
            let saved = crate::store::Store::open(file, Mode::Snapshot).unwrap().document().unwrap();
            assert_eq!(serde_json::from_str::<serde_json::Value>(&saved.value()).unwrap()["text"], "still editing");
        }
    }

    #[test]
    fn discard_cancels_an_active_rebuild_before_or_after_its_write() {
        for after_write in [false, true] {
            let (_dir, path, owner, _events) = fixture(Duration::from_secs(60));
            let gate = pause_rebuild(&owner, after_write, None);
            let before = value(&owner);
            let (callback, edited) = pair();
            owner.submit(set_text("never accepted"), Some("editor".into()), callback);
            let (callback, discarded) = pair();
            owner.submit(Request::Discard, None, callback);
            assert_eq!(wait(edited).unwrap_err().kind, FailureKind::Replaced);
            drop(gate);
            wait(discarded).unwrap();
            assert_eq!(value(&owner), before);
            call(&owner, set_text("kept")).unwrap();
            call(&owner, Request::Close { preview: None, icon: None }).unwrap();
            drop(owner);
            let saved = crate::store::Store::open(&path, Mode::Snapshot).unwrap().document().unwrap();
            assert_eq!(serde_json::from_str::<serde_json::Value>(&saved.value()).unwrap()["text"], "kept");
        }
    }

    #[test]
    fn failed_preparation_and_ambiguous_commit_release_queued_work() {
        for after_write in [false, true] {
            let (_dir, path, owner, events) = fixture(Duration::from_secs(60));
            let gate =
                pause_rebuild(&owner, after_write, Some(Failure::new(FailureKind::Failed, "injected rebuild failure")));
            let (callback, edited) = pair();
            owner.submit(set_text("after failure"), None, callback);
            let (callback, flushed) = pair();
            owner.submit(Request::Flush, None, callback);
            call(&owner, Request::State).unwrap();
            assert!(flushed.try_recv().is_err());
            drop(gate);
            wait(edited).unwrap();
            wait(flushed).unwrap();
            assert_eq!(failed_saves(&events), 0);
            call(&owner, Request::Close { preview: None, icon: None }).unwrap();
            drop(owner);
            let saved = crate::store::Store::open(&path, Mode::Snapshot).unwrap().document().unwrap();
            assert_eq!(serde_json::from_str::<serde_json::Value>(&saved.value()).unwrap()["text"], "after failure");
        }
    }

    #[test]
    fn an_invalidated_rebuild_answers_held_requests_and_allows_discard() {
        let (_dir, _path, owner, _events) = fixture(Duration::from_secs(60));
        let gate = pause_rebuild(&owner, false, Some(poisoned()));
        let (callback, edited) = pair();
        owner.submit(set_text("not applied"), None, callback);
        let (callback, flushed) = pair();
        owner.submit(Request::Flush, None, callback);
        call(&owner, Request::State).unwrap();
        drop(gate);
        assert_eq!(wait(edited).unwrap_err().kind, FailureKind::Invalidated);
        assert_eq!(wait(flushed).unwrap_err().kind, FailureKind::Invalidated);
        call(&owner, Request::Discard).unwrap();
        call(&owner, set_text("recovered")).unwrap();
        call(&owner, Request::Close { preview: None, icon: None }).unwrap();
    }

    #[test]
    fn close_right_after_a_rebuild_still_trims_history() {
        let (_dir, path, owner, _events) = fixture(Duration::ZERO);
        // A budget the rebuild's undo window fits in, so its checkpoint keeps history.
        owner.store.set_budget(Budget {
            checkpoint_rows: 1,
            checkpoint_bytes: 1,
            session_bytes: 16 * 1024,
            trim_bytes: 0,
            rebuild_idle: Duration::ZERO,
        });
        let short = |seed: u64| noise(seed)[..300].to_owned();
        let mut round = 0;
        while owner.store.rebuilt_size() == 0 {
            assert!(round < 400, "no rebuild ran");
            call(&owner, set_text(&short(round))).unwrap();
            call(&owner, Request::Flush).unwrap();
            round += 1;
        }
        // One more edit makes another rebuild due; close once it ran, with no edit after.
        let first = owner.store.rebuilt_size();
        while owner.store.rebuilt_size() == first {
            assert!(round < 800, "no second rebuild ran");
            call(&owner, set_text(&short(round))).unwrap();
            call(&owner, Request::Flush).unwrap();
            round += 1;
            std::thread::sleep(Duration::from_millis(20));
        }
        call(&owner, Request::Close { preview: None, icon: None }).unwrap();
        drop(owner);
        // A rebuild once made close think the session had not edited, keeping its history.
        let saved = crate::store::Store::open(&path, Mode::Snapshot).unwrap().document().unwrap();
        assert_eq!(saved.doc.shallow_since_frontiers(), saved.doc.oplog_frontiers(), "no history kept");
    }
}
