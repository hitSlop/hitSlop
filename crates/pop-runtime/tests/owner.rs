use crate::support;
use pop_runtime::{
    Origin, file,
    owner::{Event, Failure, FailureKind, Owner, Reply, Request, SyncConnection},
    store::{Mode, Store},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

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
    let template = dir.path().join("template.pop");
    support::pack(&stage, &template).unwrap();
    let path = dir.path().join("document.pop");
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

// A seed reaches every member: the saved document under its own identity, with no
// attachment its state does not reference. The owner keeps every blob for its undo.
#[test]
fn a_sharing_seed_is_the_saved_document_with_only_what_it_references() {
    let (dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let attachment = |bytes: &[u8]| {
        let Reply::Attachment { item } = call(&owner, Request::PutAttachment { bytes: bytes.to_vec() }).unwrap() else {
            panic!("attachment")
        };
        item.id
    };
    let removed = attachment(b"removed before sharing");
    let kept = attachment(b"kept for members");
    call(&owner, set(&removed)).unwrap();
    call(&owner, set(&kept)).unwrap();
    let seed = dir.path().join("seed.pop");
    call(&owner, Request::StageSeed { destination: seed.clone() }).unwrap();
    let staged = Store::open(&seed, Mode::Snapshot).unwrap();
    assert_eq!(staged.app().document_uuid, owner.app().document_uuid);
    assert_eq!(staged.document().unwrap().reading().unwrap().value["title"], kept);
    let ids: Vec<_> = staged.attachments().unwrap().into_iter().map(|item| item.id).collect();
    assert_eq!(ids, vec![kept], "a seed carries no unreferenced attachment");
    staged.close().unwrap();
    assert!(matches!(call(&owner, Request::ReadAttachment { id: removed }).unwrap(), Reply::Bytes { bytes: Some(_) }));
    close(&owner);
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
    let published: Vec<Value> = events
        .try_iter()
        .filter_map(|e| {
            if let Event::Publication { json } = e {
                Some(serde_json::from_str::<Value>(&json).unwrap()["publication"].clone())
            } else {
                None
            }
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
            pop_runtime::owner::theme_request(pop_runtime::owner::ThemeChange::Set {
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
    let moved = dir.path().join("moved.pop");
    std::fs::rename(&path, &moved).unwrap();
    assert_eq!(call(&owner, Request::Close { preview: None, icon: None }).unwrap_err().kind, FailureKind::Moved);
    assert!(matches!(Store::open(&moved, Mode::Document), Err(pop_runtime::store::Error::Locked)));
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
    use pop_runtime::owner::SaveStatus;
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

/// A shared document whose saved history has reached the storage limit, so a save fails.
fn full_shared_document() -> (tempfile::TempDir, PathBuf, Owner, mpsc::Receiver<Event>) {
    let (dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    call(&owner, Request::Share { share: pop_runtime::store::Share { endpoint: "wss://relay.example".into() } })
        .unwrap();
    let mut seed = 5u64;
    let mut full = false;
    for _ in 0..64 {
        let title: String = (0..1024 * 1024)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (b'!' + (seed % 90) as u8) as char
            })
            .collect();
        call(&owner, set(&title)).unwrap();
        if let Err(error) = call(&owner, Request::Flush) {
            assert_eq!(error.kind, FailureKind::Full);
            full = true;
        }
    }
    assert!(full, "shared history must reach the storage limit");
    (dir, path, owner, events)
}

#[test]
fn a_full_shared_document_copies_its_unsaved_live_state() {
    let (dir, path, owner, _) = full_shared_document();
    let Reply::Attachment { item } =
        call(&owner, Request::PutAttachment { bytes: b"live reference".to_vec() }).unwrap()
    else {
        panic!("attachment")
    };
    call(&owner, Request::PutAttachment { bytes: b"unreferenced".to_vec() }).unwrap();
    let title = format!("Unsaved escape attachment/{}", item.id);
    call(&owner, set(&title)).unwrap();
    assert_eq!(call(&owner, Request::Flush).unwrap_err().kind, FailureKind::Full);
    assert_eq!(state(&owner)["value"]["title"], title);
    assert_ne!(saved_title(&path), title);
    let destination = dir.path().join("Unshared.pop");
    call(&owner, Request::Copy { destination: destination.clone(), preview: None, icon: None }).unwrap();
    let copy = Store::open(&destination, Mode::Snapshot).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&copy.document().unwrap().value()).unwrap()["title"], title);
    assert!(copy.share().unwrap().is_none());
    assert_eq!(copy.attachments().unwrap(), vec![item.clone()]);
    assert_eq!(copy.attachment(&item.id).unwrap(), b"live reference");
    assert_ne!(copy.app().document_uuid, Store::open(&path, Mode::Snapshot).unwrap().app().document_uuid);
    copy.close().unwrap();
}

/// A full shared document holding an edit it could not save.
fn full_with_unsaved_edit(title: &str) -> (tempfile::TempDir, PathBuf, Owner, mpsc::Receiver<Event>) {
    let (dir, path, owner, events) = full_shared_document();
    call(&owner, set(title)).unwrap();
    assert_eq!(call(&owner, Request::Flush).unwrap_err().kind, FailureKind::Full);
    assert!(
        events.try_iter().any(|event| matches!(
            event,
            Event::SaveStatus { pressure: pop_runtime::store::Pressure::Full, failure: Some(_), .. }
        )),
        "a failed save reports the document as full"
    );
    (dir, path, owner, events)
}
fn handoff(dest: &Path) -> Request {
    Request::Handoff { dest: dest.to_path_buf(), preview: None, icon: None }
}
fn leftovers(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".pop"))
        .collect();
    names.sort();
    names
}

#[test]
fn a_full_shared_document_hands_off_its_live_state_to_a_new_file() {
    let (dir, path, owner, _) = full_with_unsaved_edit("Unsaved");
    // A close must save first, so it cannot leave.
    assert_eq!(call(&owner, Request::Close { preview: None, icon: None }).unwrap_err().kind, FailureKind::Full);
    let dest = dir.path().join("Unshared.pop");
    call(&owner, handoff(&dest)).unwrap();
    assert_eq!(call(&owner, Request::State).unwrap_err().kind, FailureKind::Closed);
    // The new file holds what the owner could not save, as a document of its own.
    assert_eq!(saved_title(&dest), "Unsaved");
    let copy = Store::open(&dest, Mode::Snapshot).unwrap();
    assert!(copy.share().unwrap().is_none());
    assert_ne!(copy.app().document_uuid, Store::open(&path, Mode::Snapshot).unwrap().app().document_uuid);
    copy.close().unwrap();
    // The original never received the edit, and its writer lock is free.
    assert_ne!(saved_title(&path), "Unsaved");
    Store::open(&path, Mode::Document).unwrap().close().unwrap();
    assert!(leftovers(dir.path()).is_empty(), "the handoff leaves no temporary file");
}

#[test]
fn a_refused_handoff_keeps_the_owner_open_and_never_touches_the_destination() {
    let (dir, path, owner, _) = full_with_unsaved_edit("Unsaved");
    // Another owner already has a document at the destination, with an edit of its own.
    let dest = dir.path().join("Taken.pop");
    call(&owner, Request::Copy { destination: dest.clone(), preview: None, icon: None }).unwrap();
    let (other, _) = open(&dest, Mode::Document);
    call(&other, set("Other owner")).unwrap();
    call(&other, Request::Flush).unwrap();
    let failure = call(&owner, handoff(&dest)).unwrap_err();
    assert_eq!((failure.kind, failure.reason), (FailureKind::Rejected, Some(pop_runtime::Code::Exists)));
    // The destination and its owner are untouched: still locked, still the other owner's.
    assert!(Store::open(&dest, Mode::Document).is_err());
    assert_eq!(state(&other)["value"]["title"], "Other owner");
    call(&other, set("Still editing")).unwrap();
    call(&other, Request::Flush).unwrap();
    assert_eq!(saved_title(&dest), "Still editing");
    close(&other);
    assert!(leftovers(dir.path()).is_empty(), "a refusal leaves no temporary file");
    // The source stays open with its edit, still holding its lock, and can still be handed off.
    assert_eq!(state(&owner)["value"]["title"], "Unsaved");
    assert!(Store::open(&path, Mode::Document).is_err(), "a refused handoff keeps the writer lock");
    call(&owner, set("Newer")).unwrap();
    let fresh = dir.path().join("Fresh.pop");
    call(&owner, handoff(&fresh)).unwrap();
    assert_eq!(saved_title(&fresh), "Newer");
    assert_eq!(saved_title(&dest), "Still editing");
    Store::open(&path, Mode::Document).unwrap().close().unwrap();
}

#[test]
fn an_admitted_copy_finishes_before_close_and_contains_its_flushed_edits() {
    let (dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let destination = dir.path().join("capture.pop");
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
    let alias = dir.path().join("alias.pop");
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
fn saving(events: &mpsc::Receiver<Event>) -> Vec<pop_runtime::owner::SaveStatus> {
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
    assert!(pop_runtime::registry::Lease::acquire(&path).is_err(), "the lock is held until the final write");
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    closing.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    assert!(pop_runtime::registry::Lease::acquire(&path).is_ok());
    assert_eq!(saved_title(&path), "Before close");
}

// Failure: discard fenced an active save, then a failed reload (a moved file) left writing
// latched forever and published nothing. Oracle: the failure is published, and once the
// file is back another edit becomes durable and close releases ownership.
#[test]
fn a_failed_discard_publishes_its_failure_and_saving_recovers() {
    let (dir, path) = fixture();
    let moved = dir.path().join("Moved.pop");
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
    assert!(saving(&events).contains(&pop_runtime::owner::SaveStatus::Failed));
    std::fs::rename(&moved, &path).unwrap();
    call(&owner, Request::Discard).unwrap();
    call(&owner, set("After")).unwrap();
    call(&owner, Request::Flush).unwrap();
    assert_eq!(saved_title(&path), "After");
    close(&owner);
    assert!(pop_runtime::registry::Lease::acquire(&path).is_ok());
}

#[test]
fn a_discard_publishes_no_save_failure() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    call(&owner, set("Unsaved")).unwrap();
    call(&owner, Request::Discard).unwrap();
    assert!(!saving(&events).contains(&pop_runtime::owner::SaveStatus::Failed));
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

// Another connection holding the database (a backup during Duplicate) is a definite,
// retryable failure, never mistaken for a lost reply or a conflict: a flush fails and its
// retry saves the edit; a theme export waits for its palette's save and fails with it.
// Both run at once on their own documents, so the test waits out the busy timeout once.
#[test]
fn a_busy_database_fails_a_flush_or_theme_export_that_a_retry_completes() {
    let hold = |path: &std::path::Path| {
        let lock = rusqlite::Connection::open(path).unwrap();
        lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
        lock
    };
    let flush = || {
        let (_dir, path) = fixture();
        let (owner, _) = open(&path, Mode::Document);
        call(&owner, set("Retried")).unwrap();
        let lock = hold(&path);
        assert_eq!(call(&owner, Request::Flush).unwrap_err().kind, FailureKind::Busy);
        lock.execute_batch("ROLLBACK").unwrap();
        drop(lock);
        call(&owner, Request::Flush).unwrap();
        assert_eq!(saved_title(&path), "Retried");
        close(&owner);
    };
    let theme_export = || {
        let (_dir, path) = fixture();
        let (owner, _) = open(&path, Mode::Document);
        let lock = hold(&path);
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
    };
    std::thread::scope(|scope| {
        for run in [scope.spawn(flush), scope.spawn(theme_export)] {
            if let Err(panic) = run.join() {
                std::panic::resume_unwind(panic);
            }
        }
    });
}

/// A connection's sink, and what the owner tells it.
fn sync_sink() -> (pop_runtime::owner::SyncSink, mpsc::Receiver<pop_runtime::owner::SyncEvent>) {
    let (tx, rx) = mpsc::channel();
    (
        Arc::new(move |event| {
            let _ = tx.send(event);
        }),
        rx,
    )
}

#[test]
fn pending_copies_bound_derived_work_until_completion_and_preserve_flush_capacity() {
    let (dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let mut accepted =
        vec![submit(&owner, Request::Copy { destination: dir.path().join("first.pop"), preview: None, icon: None })];
    let (entered, blocked) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::channel();
    owner.submit(
        Request::State,
        None,
        Box::new(move |_| {
            entered.send(()).unwrap();
            let _ = gate.recv();
        }),
    );
    blocked.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut refused = None;
    for index in 0..32 {
        let destination = dir.path().join(format!("copy-{index}.pop"));
        let reply = submit(&owner, Request::Copy { destination: destination.clone(), preview: None, icon: None });
        match reply.try_recv() {
            Err(mpsc::TryRecvError::Empty) => accepted.push(reply),
            Ok(Err(failure)) => {
                assert_eq!(failure.kind, FailureKind::Busy);
                refused = Some(destination);
                break;
            }
            _ => panic!("unexpected copy result while the owner is blocked"),
        }
    }
    assert!(refused.is_some(), "snapshot-producing requests must have a bounded outstanding budget");
    let flush = submit(&owner, Request::Flush);
    assert!(matches!(flush.try_recv(), Err(mpsc::TryRecvError::Empty)));
    lock.execute_batch("ROLLBACK").unwrap();
    release.send(()).unwrap();
    for reply in accepted {
        if let Err(failure) = reply.recv_timeout(Duration::from_secs(30)).unwrap() {
            assert_eq!(failure.kind, FailureKind::Busy);
        }
    }
    assert!(!refused.unwrap().exists(), "a refused copy must not publish a destination");
    assert!(matches!(flush.recv_timeout(Duration::from_secs(30)).unwrap().unwrap(), Reply::Unit));
    call(&owner, Request::Copy { destination: dir.path().join("recovered.pop"), preview: None, icon: None }).unwrap();
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    close(&owner);
}

#[test]
fn request_flood_refuses_before_mutation_and_preserves_flush_capacity() {
    struct Release(Option<mpsc::Sender<()>>);
    impl Drop for Release {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let (entered, blocked) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::channel();
    let release = Release(Some(release));
    owner.submit(
        Request::State,
        None,
        Box::new(move |_| {
            entered.send(()).unwrap();
            gate.recv().unwrap();
        }),
    );
    blocked.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut accepted = Vec::new();
    let mut refused = false;
    for index in 0..1024 {
        let reply = submit(&owner, set(&format!("request {index}")));
        match reply.try_recv() {
            Err(mpsc::TryRecvError::Empty) => accepted.push(reply),
            Ok(Err(failure)) => {
                assert_eq!(failure.kind, FailureKind::Busy);
                refused = true;
                break;
            }
            other => panic!("unexpected admission result while owner is blocked: {other:?}"),
        }
    }
    assert!(refused, "a blocked owner must bound retained requests");
    assert!(!accepted.is_empty());
    let flush = submit(&owner, Request::Flush);
    assert!(matches!(flush.try_recv(), Err(mpsc::TryRecvError::Empty)));
    drop(release);
    let expected = format!("request {}", accepted.len() - 1);
    for reply in accepted {
        reply.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    }
    assert!(matches!(flush.recv_timeout(Duration::from_secs(30)).unwrap().unwrap(), Reply::Unit));
    assert_eq!(state(&owner)["value"]["title"], expected);
    call(&owner, set("Capacity recovered")).unwrap();
    call(&owner, Request::Flush).unwrap();
    close(&owner);
}

#[test]
fn retained_batch_capacity_is_refused_before_mutation_and_preserves_flush_capacity() {
    struct Release(Option<mpsc::Sender<()>>);
    impl Drop for Release {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let (entered, blocked) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::channel();
    let release = Release(Some(release));
    owner.submit(
        Request::State,
        None,
        Box::new(move |_| {
            entered.send(()).unwrap();
            gate.recv().unwrap();
        }),
    );
    blocked.recv_timeout(Duration::from_secs(5)).unwrap();

    // Reserve a large backing allocation without touching it; the visible valid edit
    // stays tiny, so serialized JSON size cannot represent the queued memory.
    let mut value = String::with_capacity(65 * 1024 * 1024);
    value.push_str("Oversized backing allocation");
    let request = Request::Apply {
        batch: pop_runtime::Batch {
            if_version: None,
            intents: vec![pop_runtime::Intent::Set {
                path: vec![pop_runtime::Segment::Key("title".into())],
                value: Value::String(value),
                from: None,
                selection: None,
            }],
        },
        origin: Origin::Page,
    };
    let reply = submit(&owner, request);
    match reply.try_recv() {
        Ok(Err(failure)) => assert_eq!(failure.kind, FailureKind::Busy),
        other => panic!("oversized retained batch must be refused before enqueueing: {other:?}"),
    }
    let flush = submit(&owner, Request::Flush);
    assert!(matches!(flush.try_recv(), Err(mpsc::TryRecvError::Empty)));
    drop(release);
    assert!(matches!(flush.recv_timeout(Duration::from_secs(30)).unwrap().unwrap(), Reply::Unit));
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    call(&owner, set("Capacity recovered")).unwrap();
    call(&owner, Request::Flush).unwrap();
    close(&owner);
}

#[test]
fn sync_callback_panics_preserve_editing_and_saving() {
    use pop_runtime::owner::SyncEvent;

    for reconnect in [false, true] {
        let (_dir, path) = fixture();
        let (owner, _) = open(&path, Mode::Document);
        let (entered, received) = mpsc::channel();
        let base_entered = entered.clone();
        let connection = owner.sync_connection(Arc::new(move |event| {
            if reconnect && matches!(event, SyncEvent::Reconnect) {
                entered.send(()).unwrap();
                panic!("connection callback failed");
            }
        }));
        if reconnect {
            call(&owner, Request::State).unwrap();
            assert!(!connection.frame(vec![0; pop_runtime::sync::MAX_FRAME + 1]));
        } else {
            connection.base(move |_| {
                base_entered.send(()).unwrap();
                panic!("base callback failed");
            });
        }
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        call(&owner, set("Saved after callback panic")).unwrap();
        call(&owner, Request::Flush).unwrap();
        close(&owner);
        let (reopened, _) = open(&path, Mode::Document);
        assert_eq!(state(&reopened)["value"]["title"], "Saved after callback panic");
        close(&reopened);
    }
}

#[test]
fn sync_base_queued_during_owner_shutdown_is_settled() {
    let (_dir, path) = fixture();
    let (entered, blocked) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let gate = std::sync::Mutex::new(Some(gate));
    let owner = Owner::open(
        &path,
        Mode::Document,
        Arc::new(move |_| {
            if let Some(gate) = gate.lock().unwrap().take() {
                entered.send(()).unwrap();
                // Dropping the release sender also unblocks teardown if an assertion fails.
                let _ = gate.recv();
            }
        }),
    )
    .unwrap();
    blocked.recv_timeout(Duration::from_secs(5)).unwrap();
    let (sink, _) = sync_sink();
    let connection = owner.sync_connection(sink);
    drop(owner);
    let (sent, received) = mpsc::channel();
    connection.base(move |result| sent.send(result).unwrap());
    release.send(()).unwrap();
    assert!(matches!(received.recv_timeout(Duration::from_secs(5)).unwrap(), Ok(None)));
}

#[test]
fn sync_base_settles_after_the_owner_is_dropped() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    let (sink, _) = sync_sink();
    let connection = owner.sync_connection(sink);
    close(&owner);
    drop(owner);
    loop {
        match events.recv_timeout(Duration::from_secs(5)) {
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(error) => panic!("owner listener remained active after drop: {error}"),
        }
    }
    let (sent, received) = mpsc::channel();
    connection.base(move |result| sent.send(result).unwrap());
    assert!(matches!(received.recv_timeout(Duration::from_secs(5)).unwrap(), Ok(None)));
}

#[test]
fn sync_input_budget_recovers_and_old_connections_cannot_detach_the_replacement() {
    use pop_runtime::{
        owner::SyncEvent,
        store::Share,
        sync::{Frame, Permission, Status},
    };
    for frame in [vec![0; pop_runtime::sync::MAX_FRAME], Vec::new()] {
        let (_dir, path) = fixture();
        let (owner, _) = open(&path, Mode::Document);
        call(&owner, Request::Share { share: Share { endpoint: "https://relay.example".into() } }).unwrap();
        let (sink, events) = sync_sink();
        let old = owner.sync_connection(sink);
        call(&owner, Request::State).unwrap();
        while events.try_recv().is_ok() {}
        let (blocked, wait) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        owner.submit(
            Request::State,
            None,
            Box::new(move |_| {
                blocked.send(()).unwrap();
                gate.recv().unwrap();
            }),
        );
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        let count = if frame.is_empty() { pop_contracts::relay::limits::QUEUED_FRAMES } else { 2 };
        for _ in 0..count {
            assert!(old.frame(frame.clone()));
        }
        let refused = !old.frame(vec![0]);
        release.send(()).unwrap();
        assert!(refused, "Owner admission must bound both queued bytes and frame count");
        loop {
            if matches!(events.recv_timeout(Duration::from_secs(5)).unwrap(), SyncEvent::Reconnect) {
                break;
            }
        }
        call(&owner, set("Local editing survives overflow")).unwrap();
        let (sink, next) = sync_sink();
        let replacement = owner.sync_connection(sink);
        let version = loop {
            if let SyncEvent::Send { bytes } = next.recv_timeout(Duration::from_secs(5)).unwrap()
                && let Frame::Hello { version, .. } = Frame::decode(&bytes).unwrap()
            {
                break version;
            }
        };
        old.detach();
        assert!(!old.frame(Frame::CaughtUp.encode()));
        assert!(replacement.frame(Frame::Welcome { permission: Permission::Write, version }.encode()));
        assert!(replacement.frame(Frame::CaughtUp.encode()));
        loop {
            if matches!(
                next.recv_timeout(Duration::from_secs(5)).unwrap(),
                SyncEvent::Status { status: Status::Synced }
            ) {
                break;
            }
        }
        let current = state(&owner);
        assert_eq!(current["value"]["title"], "Local editing survives overflow");
        let (sent, old_receipt) = mpsc::channel();
        old.confirmed_version(move |receipt| sent.send(receipt).unwrap());
        assert!(
            old_receipt.recv_timeout(Duration::from_secs(5)).unwrap().is_none(),
            "an old connection cannot confirm a replacement's state under its former connection"
        );
        let (sent, confirmed) = mpsc::channel();
        replacement.confirmed_version(move |receipt| sent.send(receipt).unwrap());
        let receipt = confirmed.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(receipt.version, current["version"]);
        call(&owner, Request::Flush).unwrap();
        assert_eq!(
            Store::open(&path, Mode::Snapshot).unwrap().document().unwrap().version(),
            receipt.version,
            "the confirmed version is the saved one after a flush"
        );
        close(&owner);
    }
}

/// Drains both connections: each frame an owner sends goes to the relay, whose replies go
/// back to it and whose stored records go to the other joined owner. Returns once `done`
/// holds of the owners' latest sync statuses (and whatever else it reads).
fn pump(
    relay: &mut support::relay::Relay,
    owners: [(&SyncConnection, &mpsc::Receiver<pop_runtime::owner::SyncEvent>); 2],
    joined: &mut [bool; 2],
    status: &mut [Option<pop_runtime::sync::Status>; 2],
    done: impl Fn(&[Option<pop_runtime::sync::Status>; 2]) -> bool,
) {
    use pop_runtime::owner::SyncEvent;
    use pop_runtime::sync::Frame;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        for i in 0..2 {
            while let Ok(event) = owners[i].1.try_recv() {
                match event {
                    SyncEvent::Send { bytes } => {
                        let (replies, stored) = relay.reply(&bytes);
                        for reply in replies {
                            joined[i] |= matches!(reply, Frame::Welcome { .. });
                            owners[i].0.frame(reply.encode());
                        }
                        if !stored.is_empty() && joined[1 - i] {
                            owners[1 - i].0.frame(Frame::Records(stored).encode());
                        }
                    }
                    SyncEvent::Status { status: s } => status[i] = Some(s),
                    SyncEvent::Reconnect => panic!("Unexpected overflow in the bounded relay fixture"),
                }
            }
        }
        if done(status) {
            return;
        }
        assert!(Instant::now() < deadline, "sync did not settle: {status:?}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Two owners of one shared document, each opened from its own file, sync through their
/// hooks alone: a host only moves frames between a connection's sink and the relay. An
/// unshared document refuses to attach, and a shared one refuses to discard (other copies
/// may already hold its edits).
#[test]
fn owners_of_a_shared_document_sync_through_their_hooks() {
    use pop_runtime::owner::SyncEvent;
    use pop_runtime::store::Share;
    use pop_runtime::sync::{Frame, Status};
    let (dir, path) = fixture();
    let (a, _a_events) = open(&path, Mode::Document);
    let (sink, unshared) = sync_sink();
    a.sync_connection(sink);
    let SyncEvent::Status { status: paused } = unshared.recv_timeout(Duration::from_secs(10)).unwrap() else {
        panic!("an unshared document sends nothing")
    };
    assert!(matches!(paused, Status::Paused { .. }), "{paused:?}");
    let share = Share { endpoint: "wss://relay.example".into() };
    call(&a, Request::Share { share: share.clone() }).unwrap();
    assert_eq!(a.share().unwrap(), Some(share.clone()));
    let refused = call(&a, Request::Discard).unwrap_err();
    assert_eq!(refused.reason, Some(pop_runtime::Code::InvalidRequest), "{refused:?}");
    // The seed every replica starts from: a backup, which keeps the identity and share.
    let seed = dir.path().join("seed.pop");
    call(&a, Request::Backup { destination: seed.clone() }).unwrap();
    let (b, _b_events) = open(&seed, Mode::Document);
    assert_eq!(b.share().unwrap(), Some(share));

    let (a_sink, a_sync) = sync_sink();
    let a_connection = a.sync_connection(a_sink);
    let SyncEvent::Send { bytes: hello } = a_sync.recv_timeout(Duration::from_secs(10)).unwrap() else {
        panic!("a Hello first")
    };
    // The relay's room admits the app A presents; B, a copy of A, presents the same.
    let Frame::Hello { app_digest, version, .. } = Frame::decode(&hello).unwrap() else { panic!("a Hello first") };
    let mut relay = support::relay::Relay { app: app_digest, records: vec![], heads: version };
    let mut joined = [false; 2];
    let mut status = [None, None];
    // The Hello already taken from A's sink still needs its answer.
    let (replies, _) = relay.reply(&hello);
    for reply in replies {
        joined[0] |= matches!(reply, Frame::Welcome { .. });
        a_connection.frame(reply.encode());
    }
    let (b_sink, b_sync) = sync_sink();
    let b_connection = b.sync_connection(b_sink);
    let synced = |status: &[Option<Status>; 2]| status.iter().all(|s| s == &Some(Status::Synced));
    let owners = [(&a_connection, &a_sync), (&b_connection, &b_sync)];
    pump(&mut relay, owners, &mut joined, &mut status, synced);
    let title = |owner: &Owner| state(owner)["value"]["title"].clone();
    call(&a, set("From A")).unwrap();
    pump(&mut relay, owners, &mut joined, &mut status, |_| title(&b) == "From A");
    call(&b, set("From B")).unwrap();
    pump(&mut relay, owners, &mut joined, &mut status, |s| synced(s) && title(&a) == "From B");
    // What arrived is saved like an edit.
    call(&a, Request::Close { preview: None, icon: None }).unwrap();
    let (reopened, _) = open(&path, Mode::Document);
    assert_eq!(title(&reopened), "From B");
}

// An accepted immutable blob write can finish before Discard's reload. Its reply
// must describe that committed write, even though the page generation changed.
#[test]
fn discard_does_not_relabel_an_applied_attachment_as_not_applied() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let attachment = submit(&owner, Request::PutAttachment { bytes: b"kept immutable blob".to_vec() });
    let discarded = submit(&owner, Request::Discard);
    // The actor has admitted both requests; storage still cannot commit.
    call(&owner, Request::State).unwrap();
    lock.execute_batch("ROLLBACK").unwrap();
    let reply = attachment.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    let Reply::Attachment { item } = reply else { panic!("the applied attachment must return its identity") };
    discarded.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    let Reply::Bytes { bytes: Some(bytes) } = call(&owner, Request::ReadAttachment { id: item.id }).unwrap() else {
        panic!("the immutable attachment must remain readable");
    };
    assert_eq!(bytes, b"kept immutable blob");
    close(&owner);
}

#[test]
fn discard_waits_for_membership_outcome_without_retracting_shared_edits() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let sharing = submit(
        &owner,
        Request::Share { share: pop_runtime::store::Share { endpoint: "https://relay.example".into() } },
    );
    let discarded = submit(&owner, Request::Discard);
    call(&owner, Request::State).unwrap();
    lock.execute_batch("ROLLBACK").unwrap();
    sharing.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    assert_eq!(discarded.recv_timeout(Duration::from_secs(30)).unwrap().unwrap_err().kind, FailureKind::Busy);
    let refusal = call(&owner, Request::Discard).unwrap_err();
    assert_eq!(refusal.reason, Some(pop_runtime::Code::InvalidRequest));
    close(&owner);
}

// Folding cannot make the room fit, so the transport pauses sharing as outgrown. The
// owner keeps accepting and saving edits, and pushes none of them.
#[test]
fn an_outgrown_shared_document_keeps_editing_and_saving_without_pushing() {
    use pop_runtime::{
        owner::SyncEvent,
        sync::{Frame, PauseKind, Permission, Status},
    };
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    call(&owner, Request::Share { share: pop_runtime::store::Share { endpoint: "https://relay.example".into() } })
        .unwrap();
    let (sink, events) = sync_sink();
    let connection = owner.sync_connection(sink);
    let version = loop {
        if let SyncEvent::Send { bytes } = events.recv_timeout(Duration::from_secs(5)).unwrap()
            && let Frame::Hello { version, .. } = Frame::decode(&bytes).unwrap()
        {
            break version;
        }
    };
    assert!(connection.frame(Frame::Welcome { permission: Permission::Write, version }.encode()));
    assert!(connection.frame(Frame::CaughtUp.encode()));
    connection.pause(PauseKind::Outgrown, "This document has outgrown sharing".into());
    loop {
        if matches!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            SyncEvent::Status { status: Status::Paused { kind: PauseKind::Outgrown, .. } }
        ) {
            break;
        }
    }
    call(&owner, set("Kept locally")).unwrap();
    call(&owner, Request::Flush).unwrap();
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, SyncEvent::Send { .. }), "an outgrown document pushes nothing: {event:?}");
    }
    close(&owner);
    let (reopened, _) = open(&path, Mode::Document);
    assert_eq!(state(&reopened)["value"]["title"], "Kept locally");
    close(&reopened);
}

#[test]
fn durable_sync_confirmation_requires_ack_and_save_in_either_order() {
    use pop_runtime::{
        owner::{SyncConnection, SyncEvent},
        sync::{Ack, Frame, Permission, Status},
    };
    fn confirmed(connection: &SyncConnection) -> Option<pop_runtime::sync::Confirmation> {
        let (send, result) = mpsc::channel();
        connection.durable_confirmed_version(move |version| {
            let _ = send.send(version);
        });
        result.recv_timeout(Duration::from_secs(5)).unwrap()
    }
    fn frame(events: &mpsc::Receiver<SyncEvent>) -> Frame {
        loop {
            if let SyncEvent::Send { bytes } = events.recv_timeout(Duration::from_secs(5)).unwrap() {
                return Frame::decode(&bytes).unwrap();
            }
        }
    }
    fn synced(events: &mpsc::Receiver<SyncEvent>) {
        loop {
            if matches!(
                events.recv_timeout(Duration::from_secs(5)).unwrap(),
                SyncEvent::Status { status: Status::Synced }
            ) {
                return;
            }
        }
    }
    for save_first in [false, true] {
        let (_dir, path) = fixture();
        let (owner, _) = open(&path, Mode::Document);
        call(&owner, Request::Share { share: pop_runtime::store::Share { endpoint: "https://relay.example".into() } })
            .unwrap();
        let (send, events) = mpsc::channel();
        let connection = owner.sync_connection(Arc::new(move |event| {
            let _ = send.send(event);
        }));
        let Frame::Hello { version, .. } = frame(&events) else { panic!("expected Hello") };
        connection.frame(Frame::Welcome { permission: Permission::Write, version }.encode());
        connection.frame(Frame::CaughtUp.encode());
        synced(&events);
        assert!(confirmed(&connection).is_some());
        let lock = rusqlite::Connection::open(&path).unwrap();
        if !save_first {
            lock.execute_batch("BEGIN IMMEDIATE").unwrap();
        }
        call(&owner, set("Acknowledged and saved")).unwrap();
        let expected = state(&owner)["version"].as_str().unwrap().to_owned();
        let Frame::Push { id, .. } = frame(&events) else { panic!("expected Push") };
        if save_first {
            call(&owner, Request::Flush).unwrap();
            assert!(confirmed(&connection).is_none(), "a save without relay acknowledgement is insufficient");
        }
        connection.frame(Frame::Ack { id, status: Ack::Stored }.encode());
        synced(&events);
        if !save_first {
            assert!(confirmed(&connection).is_none(), "a relay acknowledgement must not confirm unsaved state");
            lock.execute_batch("ROLLBACK").unwrap();
            // Ordinary save completion must notify sync even though it was already Synced.
            synced(&events);
        }
        assert_eq!(confirmed(&connection).unwrap().version, expected);
        let snapshot = Store::open(&path, Mode::Snapshot).unwrap();
        assert_eq!(snapshot.document().unwrap().version(), expected);
        snapshot.close().unwrap();
        // A clean flush must not repeat the durable-version notification.
        call(&owner, Request::Flush).unwrap();
        call(&owner, Request::State).unwrap();
        assert!(!events.try_iter().any(|event| matches!(event, SyncEvent::Status { status: Status::Synced })));
        connection.detach();
        assert!(confirmed(&connection).is_none(), "a detached connection cannot confirm");
        close(&owner);
        assert!(confirmed(&connection).is_none(), "a closed owner cannot confirm");
    }
}

// A relay acknowledgement can arrive while the owner is still missing the history
// advertised by Welcome. It must not certify that missing history as saved locally.
#[test]
fn welcome_push_ack_does_not_confirm_withheld_backfill() {
    use pop_runtime::{
        owner::SyncEvent,
        sync::{Frame, Permission},
    };
    fn frame(events: &mpsc::Receiver<SyncEvent>) -> Frame {
        loop {
            if let SyncEvent::Send { bytes } = events.recv_timeout(Duration::from_secs(10)).unwrap() {
                return Frame::decode(&bytes).unwrap();
            }
        }
    }

    let (dir, path) = fixture();
    let (a, _) = open(&path, Mode::Document);
    call(&a, Request::Share { share: pop_runtime::store::Share { endpoint: "https://relay.example".into() } }).unwrap();
    let seed = dir.path().join("offline.pop");
    call(&a, Request::Backup { destination: seed.clone() }).unwrap();
    let (a_sink, a_events) = sync_sink();
    let a_connection = a.sync_connection(a_sink);
    let hello = frame(&a_events);
    let Frame::Hello { app_digest, ref version, .. } = hello else { panic!("expected Hello") };
    let mut relay = support::relay::Relay { app: app_digest, records: vec![], heads: version.clone() };
    for reply in relay.reply(&hello.encode()).0 {
        a_connection.frame(reply.encode());
    }
    call(&a, theme("#123456", Origin::Agent)).unwrap();
    let remote = frame(&a_events);
    assert!(matches!(remote, Frame::Push { .. }));
    let (replies, stored) = relay.reply(&remote.encode());
    assert!(!stored.is_empty());
    for reply in replies {
        a_connection.frame(reply.encode());
    }
    close(&a);

    let (b, _) = open(&seed, Mode::Document);
    call(&b, set("Own offline title")).unwrap();
    let (b_sink, b_events) = sync_sink();
    let b_connection = b.sync_connection(b_sink);
    let hello = frame(&b_events);
    let mut backfill = relay.reply(&hello.encode()).0.into_iter();
    let welcome = backfill.next().unwrap();
    assert!(matches!(welcome, Frame::Welcome { permission: Permission::Write, .. }));
    b_connection.frame(welcome.encode());

    // Deliberately deliver only Welcome: the real owner pushes its offline edit
    // before it has received the earlier remote record or CaughtUp.
    let own = frame(&b_events);
    assert!(matches!(own, Frame::Push { .. }));
    for reply in relay.reply(&own.encode()).0 {
        b_connection.frame(reply.encode());
    }
    call(&b, Request::Flush).unwrap();
    assert_eq!(state(&b)["theme"]["accent"], "#335577");
    let (send, confirmation) = mpsc::channel();
    b_connection.durable_confirmed_version(move |version| {
        let _ = send.send(version);
    });
    assert!(confirmation.recv_timeout(Duration::from_secs(10)).unwrap().is_none());

    let mut received_records = false;
    for reply in backfill {
        received_records |= matches!(reply, Frame::Records(_));
        b_connection.frame(reply.encode());
    }
    assert!(received_records);
    call(&b, Request::Flush).unwrap();
    assert_eq!(state(&b)["value"]["title"], "Own offline title");
    assert_eq!(state(&b)["theme"]["accent"], "#123456");
    let (send, confirmation) = mpsc::channel();
    b_connection.durable_confirmed_version(move |version| {
        let _ = send.send(version);
    });
    let confirmed = confirmation.recv_timeout(Duration::from_secs(10)).unwrap().expect("saved backfill is confirmed");
    assert_eq!(state(&b)["version"], confirmed.version);
    close(&b);
    let (reopened, _) = open(&seed, Mode::Document);
    assert_eq!(state(&reopened)["value"]["title"], "Own offline title");
    assert_eq!(state(&reopened)["theme"]["accent"], "#123456");
    close(&reopened);
}
