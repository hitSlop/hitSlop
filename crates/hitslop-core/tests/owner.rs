#![cfg(feature = "storage")]
use hitslop_core::{
    Origin, file,
    owner::{Event, Failure, FailureKind, Owner, Reply, Request},
    store::{Mode, Store},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
mod support;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    support::isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/ui.js"), "export default {}").unwrap();
    support::write_app(
        &stage,
        support::App::new(r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#, r#"{"title":"Saved"}"#),
    );
    let template = dir.path().join("template.slop");
    support::pack(&stage, &template).unwrap();
    let path = dir.path().join("document.slop");
    file::create_document(&template, &path).unwrap();
    (dir, path)
}
fn open(path: &Path, mode: Mode) -> (Owner, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    (
        Owner::open(
            path,
            mode,
            Arc::new(move |event| {
                let _ = tx.send(event);
            }),
        )
        .unwrap(),
        rx,
    )
}
fn submit(owner: &Owner, request: Request) -> mpsc::Receiver<Result<Reply, Failure>> {
    let (tx, rx) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |result| {
            let _ = tx.send(result);
        }),
    );
    rx
}
fn call(owner: &Owner, request: Request) -> Result<Reply, Failure> {
    let operation = match &request {
        Request::Flush => "flush",
        Request::ExportTheme => "export theme",
        Request::Close { .. } => "close",
        Request::Discard => "discard",
        Request::State => "read state",
        Request::Apply { .. } => "apply",
        _ => "owner request",
    };
    submit(owner, request)
        .recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|error| panic!("owner did not complete {operation} within 30s: {error}"))
}
fn set(title: &str) -> Request {
    Request::Apply {
        batch: serde_json::from_value(json!({"intents":[{"type":"set","path":["title"],"value":title}]})).unwrap(),
        origin: Origin::Page,
    }
}
fn state(owner: &Owner) -> Value {
    let Reply::State { reading, .. } = call(owner, Request::State).unwrap() else { panic!("state") };
    let json = serde_json::to_string(&reading).unwrap();
    serde_json::from_str(&json).unwrap()
}
fn close(owner: &Owner) {
    call(owner, Request::Close { preview: None, icon: None }).unwrap();
}
/// A palette change to the accent, from the theme panel or an agent.
fn theme(color: &str, origin: Origin) -> Request {
    Request::Apply {
        batch: serde_json::from_value(json!({"intents":[{"type":"setTheme","values":{"accent":color}}]})).unwrap(),
        origin,
    }
}

#[test]
fn ordered_publications_flush_and_reopen_include_theme() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    let a = submit(&owner, set("First"));
    let b = submit(&owner, theme("#123456", Origin::Agent));
    let c = submit(&owner, set("Last"));
    let sequences: Vec<_> = [a, b, c]
        .into_iter()
        .map(|rx| match rx.recv().unwrap().unwrap() {
            Reply::Applied { sequence, .. } => sequence,
            _ => panic!("edit"),
        })
        .collect();
    assert_eq!(sequences, vec![1, 2, 3]);
    call(&owner, Request::Flush).unwrap();
    let published: Vec<Value> =
        events
            .try_iter()
            .filter_map(|e| {
                if let Event::Publication { json } = e { Some(serde_json::from_str(&json).unwrap()) } else { None }
            })
            .collect();
    assert_eq!(published.iter().map(|p| p["sequence"].as_u64().unwrap()).collect::<Vec<_>>(), sequences);
    assert_eq!(published[1]["ops"], json!([]));
    assert_eq!(published[1]["theme"]["accent"], "#123456");
    close(&owner);
    let (reopened, _) = open(&path, Mode::Document);
    let saved = state(&reopened);
    assert_eq!(saved["value"]["title"], "Last");
    assert_eq!(saved["theme"]["accent"], "#123456");
    close(&reopened);
}
#[test]
fn theme_and_content_share_undo_and_an_agent_color_ends_the_panel_run() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    for color in ["#111111", "#222222"] {
        call(
            &owner,
            hitslop_core::owner::theme_request(hitslop_core::owner::ThemeChange::Set {
                values: [("accent".into(), color.into())].into(),
            }),
        )
        .unwrap();
    }
    call(&owner, theme("#333333", Origin::Agent)).unwrap();
    call(&owner, set("Content after theme")).unwrap();
    call(&owner, Request::Undo { redo: false }).unwrap();
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    assert_eq!(state(&owner)["theme"]["accent"], "#333333");
    call(&owner, Request::Undo { redo: false }).unwrap();
    assert_eq!(state(&owner)["theme"]["accent"], "#222222");
    call(&owner, Request::Undo { redo: false }).unwrap();
    assert_eq!(state(&owner)["theme"]["accent"], "#335577");
    assert!(events.try_iter().filter(|e| matches!(e, Event::ThemeChanged)).count() >= 5);
    close(&owner);
}
#[test]
fn slow_persistence_does_not_block_edits_and_flush_waits_for_its_target() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    call(&owner, set("First")).unwrap();
    let first = submit(&owner, Request::Flush);
    // Whether the persistence worker has reached SQLite yet or not, the flush is
    // queued before this edit and cannot complete while the external lock is held.
    let second = submit(&owner, set("While saving"));
    assert!(second.recv_timeout(Duration::from_millis(250)).unwrap().is_ok());
    assert!(first.try_recv().is_err());
    let latest = submit(&owner, Request::Flush);
    lock.execute_batch("ROLLBACK").unwrap();
    first.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    latest.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    drop(lock);
    close(&owner);
    let (saved, _) = open(&path, Mode::Snapshot);
    assert_eq!(state(&saved)["value"]["title"], "While saving");
    close(&saved);
}
#[test]
fn failed_close_keeps_state_and_writer_lease_for_retry() {
    let (dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    call(&owner, set("Keep me")).unwrap();
    let moved = dir.path().join("moved.slop");
    std::fs::rename(&path, &moved).unwrap();
    assert_eq!(call(&owner, Request::Close { preview: None, icon: None }).unwrap_err().kind, FailureKind::Moved);
    assert!(matches!(Store::open(&moved, Mode::Document), Err(hitslop_core::store::Error::Locked)));
    assert_eq!(state(&owner)["value"]["title"], "Keep me");
    std::fs::rename(&moved, &path).unwrap();
    call(&owner, set("Retried")).unwrap();
    close(&owner);
    let (saved, _) = open(&path, Mode::Snapshot);
    assert_eq!(state(&saved)["value"]["title"], "Retried");
    close(&saved);
}
#[test]
fn discard_restores_data_and_theme_and_fences_the_old_view() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    owner.attach("page".into());
    call(&owner, set("Unsaved")).unwrap();
    call(&owner, theme("#123456", Origin::Window)).unwrap();
    call(&owner, Request::Discard).unwrap();
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    assert_eq!(state(&owner)["theme"]["accent"], "#335577");
    owner.attach("next page".into());
    let (tx, rx) = mpsc::channel();
    owner.submit(set("stale"), Some("page".into()), Box::new(move |r| tx.send(r).unwrap()));
    assert_eq!(rx.recv().unwrap().unwrap_err().kind, FailureKind::Replaced);
    close(&owner);
}
#[test]
fn discard_waits_for_inflight_save_and_ignores_its_old_completion() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    call(&owner, set("Captured for save")).unwrap();
    let flush = submit(&owner, Request::Flush);
    call(&owner, set("Discard me")).unwrap();
    let discard = submit(&owner, Request::Discard);
    assert_eq!(flush.recv_timeout(Duration::from_secs(2)).unwrap().unwrap_err().kind, FailureKind::Replaced);
    lock.execute_batch("ROLLBACK").unwrap();
    discard.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    drop(lock);
    assert_eq!(state(&owner)["value"]["title"], "Captured for save");
    call(&owner, set("After discard")).unwrap();
    close(&owner);
}
#[test]
fn snapshot_refuses_mutations_and_closed_owner_refuses_every_request() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Snapshot);
    for request in [
        set("No"),
        theme("#123456", Origin::Window),
        Request::Undo { redo: false },
        Request::PutAttachment { bytes: vec![1] },
    ] {
        assert_eq!(call(&owner, request).unwrap_err().kind, FailureKind::ReadOnly);
    }
    call(&owner, Request::Flush).unwrap();
    close(&owner);
    assert_eq!(call(&owner, Request::State).unwrap_err().kind, FailureKind::Closed);
}
#[test]
fn autosave_is_bounded_while_edits_continue() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    let start = Instant::now();
    use hitslop_core::owner::SaveStatus;
    // The owner states it is saved when it starts; only a save after an edit counts.
    let (mut saving, mut saved) = (false, false);
    let mut n = 0;
    while start.elapsed() < Duration::from_millis(1600) {
        call(&owner, set(&format!("{n}"))).unwrap();
        n += 1;
        for event in events.try_iter() {
            if let Event::SaveStatus { status, .. } = event {
                saving |= status == SaveStatus::Saving;
                saved |= saving && status == SaveStatus::Saved;
            }
        }
        if saved {
            break;
        }
        // Keep the stream below the idle deadline while allowing realistic processing.
        std::thread::sleep(Duration::from_millis(10));
    }
    // A saving event isn't evidence of persistence: inspect the saved document itself.
    let snapshot = Store::open(&path, Mode::Snapshot).unwrap();
    let durable: Value = serde_json::from_str(&snapshot.document().unwrap().value()).unwrap();
    assert_ne!(durable["title"], "Saved", "continuous edits never reached disk (saved event: {saved})");
    snapshot.close().unwrap();
    close(&owner);
}

#[test]
fn an_admitted_copy_finishes_before_close_and_contains_its_flushed_edits() {
    let (dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let destination = dir.path().join("capture.slop");
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    call(&owner, set("Captured")).unwrap();
    let copied = submit(&owner, Request::Copy { destination: destination.clone(), preview: None, icon: None });
    let closed = submit(&owner, Request::Close { preview: None, icon: None });
    assert_eq!(call(&owner, set("Too late")).unwrap_err().kind, FailureKind::Closing);
    lock.execute_batch("ROLLBACK").unwrap();
    copied.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    closed.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    drop(lock);
    let (snapshot, _) = open(&destination, Mode::Snapshot);
    assert_eq!(state(&snapshot)["value"]["title"], "Captured");
    close(&snapshot);
}
#[test]
fn expired_command_does_not_mutate_or_publish() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    let (tx, rx) = mpsc::channel();
    owner.submit_until(
        set("Expired"),
        None,
        Instant::now() - Duration::from_millis(1),
        Box::new(move |result| tx.send(result).unwrap()),
    );
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap_err().kind, FailureKind::Closing);
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    assert!(!events.try_iter().any(|e| matches!(e, Event::Publication { .. })));
    close(&owner);
}

#[test]
fn owner_preserves_store_refusal_of_symbolic_link_documents() {
    let (dir, path) = fixture();
    let alias = dir.path().join("alias.slop");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    for mode in [Mode::Document, Mode::Snapshot] {
        assert!(Owner::open(&alias, mode, Arc::new(|_| {})).is_err());
    }
}

/// The saved title, read without the owner (a snapshot takes no lock).
fn saved_title(path: &Path) -> String {
    let snapshot = Store::open(path, Mode::Snapshot).unwrap();
    let value: Value = serde_json::from_str(&snapshot.document().unwrap().value()).unwrap();
    snapshot.close().unwrap();
    value["title"].as_str().unwrap().to_owned()
}
fn saving(events: &mpsc::Receiver<Event>) -> Vec<hitslop_core::owner::SaveStatus> {
    events
        .try_iter()
        .filter_map(|event| match event {
            Event::SaveStatus { status, .. } => Some(status),
            _ => None,
        })
        .collect()
}

// Failure: an undo changed the document without saving it, or undid only the person's
// edits. Oracle: the saved bytes and the undo states the owner published.
#[test]
fn undo_saves_like_an_edit_and_reaches_agent_edits() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    let agent = Request::Apply {
        batch: serde_json::from_value(json!({"intents":[{"type":"set","path":["title"],"value":"Agent"}]})).unwrap(),
        origin: Origin::Agent,
    };
    call(&owner, agent).unwrap();
    call(&owner, set("Person")).unwrap();
    call(&owner, Request::Flush).unwrap();
    assert_eq!(saved_title(&path), "Person");
    for expected in ["Agent", "Saved"] {
        call(&owner, Request::Undo { redo: false }).unwrap();
        call(&owner, Request::Flush).unwrap();
        assert_eq!(saved_title(&path), expected);
    }
    let undo: Vec<_> = events
        .try_iter()
        .filter_map(|event| match event {
            Event::UndoState { can_undo, can_redo } => Some((can_undo, can_redo)),
            _ => None,
        })
        .collect();
    assert_eq!(undo, [(false, false), (true, false), (true, true), (false, true)]);
    close(&owner);
    // A document opens with nothing to undo: the next session starts where this one saved.
    let (reopened, events) = open(&path, Mode::Document);
    let Reply::Applied { sequence, .. } = call(&reopened, Request::Undo { redo: false }).unwrap() else { panic!() };
    assert_eq!(sequence, 0, "nothing to undo publishes nothing");
    close(&reopened);
    assert!(!events.try_iter().any(|e| matches!(e, Event::UndoState { can_undo: true, .. })));
}

// Failure: close released the writer lock or accepted edits before its final write
// committed, letting another writer in or losing the late edit.
#[test]
fn close_refuses_edits_and_holds_the_lock_until_its_final_write() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    call(&owner, set("Before close")).unwrap();
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let closing = submit(&owner, Request::Close { preview: None, icon: None });
    let refused = call(&owner, set("Too late")).unwrap_err();
    assert_eq!(refused.kind, FailureKind::Closing);
    assert!(hitslop_core::registry::Lease::acquire(&path).is_err(), "the lock is held until the final write");
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    closing.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    assert!(hitslop_core::registry::Lease::acquire(&path).is_ok());
    assert_eq!(saved_title(&path), "Before close");
}

// Failure: another connection holding the database (a backup during Duplicate) must be a
// definite, retryable failure, never mistaken for a lost reply or a conflict.
#[test]
fn a_busy_database_fails_a_flush_that_a_retry_completes() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    call(&owner, set("Retried")).unwrap();
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert_eq!(call(&owner, Request::Flush).unwrap_err().kind, FailureKind::Busy);
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    call(&owner, Request::Flush).unwrap();
    assert_eq!(saved_title(&path), "Retried");
    close(&owner);
}

// Failure: discard fenced an active save, then a failed reload (a moved file) left writing
// latched forever and published nothing. Oracle: the failure is published, and once the
// file is back another edit becomes durable and close releases ownership.
#[test]
fn a_failed_discard_publishes_its_failure_and_saving_recovers() {
    let (dir, path) = fixture();
    let moved = dir.path().join("Moved.slop");
    let (owner, events) = open(&path, Mode::Document);
    call(&owner, set("Unsaved")).unwrap();
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let flush = submit(&owner, Request::Flush);
    let discard = submit(&owner, Request::Discard);
    // Discard rejects the old save's waiter before waiting for the persistence queue.
    assert_eq!(flush.recv_timeout(Duration::from_secs(30)).unwrap().unwrap_err().kind, FailureKind::Replaced);
    std::fs::rename(&path, &moved).unwrap();
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    assert_eq!(discard.recv_timeout(Duration::from_secs(30)).unwrap().unwrap_err().kind, FailureKind::Moved);
    assert!(saving(&events).contains(&hitslop_core::owner::SaveStatus::Failed));
    std::fs::rename(&moved, &path).unwrap();
    call(&owner, Request::Discard).unwrap();
    call(&owner, set("After")).unwrap();
    call(&owner, Request::Flush).unwrap();
    assert_eq!(saved_title(&path), "After");
    close(&owner);
    assert!(hitslop_core::registry::Lease::acquire(&path).is_ok());
}

#[test]
fn a_discard_publishes_no_save_failure() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    call(&owner, set("Unsaved")).unwrap();
    call(&owner, Request::Discard).unwrap();
    assert!(!saving(&events).contains(&hitslop_core::owner::SaveStatus::Failed));
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    close(&owner);
}

// Failure: a flush admitted while discard reloaded saved bytes waited for the old
// publication sequence, which the reloaded document (sequence 0) never reaches, so the
// page hung. Oracle: the flush settles as replaced, and saving still works.
#[test]
fn a_flush_during_a_discard_reload_settles_as_replaced() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    owner.attach("page".into());
    call(&owner, set("One")).unwrap();
    call(&owner, set("Two")).unwrap();
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let discard = submit(&owner, Request::Discard);
    let (tx, rx) = mpsc::channel();
    owner.submit(Request::Flush, Some("page".into()), Box::new(move |r| tx.send(r).unwrap()));
    assert_eq!(rx.recv_timeout(Duration::from_secs(30)).unwrap().unwrap_err().kind, FailureKind::Replaced);
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    discard.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    call(&owner, set("Three")).unwrap();
    call(&owner, Request::Flush).unwrap();
    assert_eq!(saved_title(&path), "Three");
    close(&owner);
}

// A theme change is an edit: accepted in memory, saved by the owner's jobs, and waited for
// by export, which fails while its save cannot complete. Oracle: the saved palette.
#[test]
fn a_theme_export_waits_for_the_save_of_its_palette() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    call(&owner, theme("#111111", Origin::Window)).unwrap();
    assert_eq!(state(&owner)["theme"]["accent"], "#111111");
    assert_eq!(call(&owner, Request::ExportTheme).unwrap_err().kind, FailureKind::Busy);
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    let Reply::ThemeFile { json } = call(&owner, Request::ExportTheme).unwrap() else { panic!() };
    assert!(json.contains("#111111"));
    close(&owner);
    let (saved, _) = open(&path, Mode::Snapshot);
    assert_eq!(state(&saved)["theme"]["accent"], "#111111");
    close(&saved);
}
