#![cfg(feature = "storage")]
use hitslop_core::{
    file,
    owner::{Event, Failure, FailureKind, Owner, Reply, Request, ThemeChange},
    store::{Mode, Store},
    Origin,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
mod support;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    support::isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/app.js"), "export default {}").unwrap();
    support::write_app(
        &stage,
        support::App::new(
            r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#,
            r#"{"title":"Saved"}"#,
        ),
    );
    let template = dir.path().join("template.slop");
    file::pack(&stage, &template).unwrap();
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
        None,
        Box::new(move |result| {
            let _ = tx.send(result);
        }),
    );
    rx
}
fn call(owner: &Owner, request: Request) -> Result<Reply, Failure> {
    submit(owner, request)
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
}
fn set(title: &str) -> Request {
    Request::Apply {
        batch_json: json!({"intents":[{"type":"set","path":["title"],"value":title}]}).to_string(),
        origin: Origin::Page,
    }
}
fn state(owner: &Owner) -> Value {
    let Reply::State { json } = call(owner, Request::State).unwrap() else {
        panic!("state")
    };
    serde_json::from_str(&json).unwrap()
}
fn close(owner: &Owner) {
    call(
        owner,
        Request::Close {
            preview: None,
            icon: None,
        },
    )
    .unwrap();
}
fn theme(color: &str) -> Request {
    Request::Theme {
        change: ThemeChange::Set(json!({"accent":color}).to_string()),
        gesture: false,
    }
}

#[test]
fn ordered_publications_flush_and_reopen_include_theme() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    let a = submit(&owner, set("First"));
    let b = submit(&owner, theme("#123456"));
    let c = submit(&owner, set("Last"));
    let sequences: Vec<_> = [a, b, c]
        .into_iter()
        .map(|rx| match rx.recv().unwrap().unwrap() {
            Reply::Applied { sequence, .. } | Reply::Theme { sequence, .. } => sequence,
            _ => panic!("edit"),
        })
        .collect();
    assert_eq!(sequences, vec![1, 2, 3]);
    call(&owner, Request::Flush).unwrap();
    let published: Vec<Value> = events
        .try_iter()
        .filter_map(|e| {
            if let Event::Publication { json } = e {
                Some(serde_json::from_str(&json).unwrap())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        published
            .iter()
            .map(|p| p["sequence"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        sequences
    );
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
fn theme_and_content_share_undo_and_cli_theme_ends_panel_gesture() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    call(&owner, Request::BeginThemeGesture).unwrap();
    for color in ["#111111", "#222222"] {
        call(
            &owner,
            Request::Theme {
                change: ThemeChange::Set(json!({"accent":color}).to_string()),
                gesture: true,
            },
        )
        .unwrap();
    }
    call(&owner, theme("#333333")).unwrap();
    call(&owner, set("Content after theme")).unwrap();
    call(&owner, Request::Undo { redo: false }).unwrap();
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    assert_eq!(state(&owner)["theme"]["accent"], "#333333");
    call(&owner, Request::Undo { redo: false }).unwrap();
    assert_eq!(state(&owner)["theme"]["accent"], "#222222");
    call(&owner, Request::Undo { redo: false }).unwrap();
    assert_eq!(state(&owner)["theme"]["accent"], "#335577");
    assert!(
        events
            .try_iter()
            .filter(|e| matches!(e, Event::ThemeChanged))
            .count()
            >= 5
    );
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
    assert!(second
        .recv_timeout(Duration::from_millis(250))
        .unwrap()
        .is_ok());
    assert!(first.try_recv().is_err());
    let latest = submit(&owner, Request::Flush);
    lock.execute_batch("ROLLBACK").unwrap();
    first.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    latest
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
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
    assert_eq!(
        call(
            &owner,
            Request::Close {
                preview: None,
                icon: None
            }
        )
        .unwrap_err()
        .kind,
        FailureKind::Moved
    );
    assert!(matches!(
        Store::open(&moved, Mode::Document),
        Err(hitslop_core::store::Error::Locked)
    ));
    assert_eq!(state(&owner)["value"]["title"], "Keep me");
    std::fs::rename(&moved, &path).unwrap();
    call(&owner, set("Retried")).unwrap();
    close(&owner);
    let (saved, _) = open(&path, Mode::Snapshot);
    assert_eq!(state(&saved)["value"]["title"], "Retried");
    close(&saved);
}
#[test]
fn discard_restores_data_and_theme_and_fences_old_epoch_and_view() {
    let (_dir, path) = fixture();
    let (owner, _) = open(&path, Mode::Document);
    owner.attach("page".into());
    let epoch = owner.epoch();
    call(&owner, set("Unsaved")).unwrap();
    call(&owner, theme("#123456")).unwrap();
    call(&owner, Request::Discard).unwrap();
    assert_ne!(owner.epoch(), epoch);
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    assert_eq!(state(&owner)["theme"]["accent"], "#335577");
    for (epoch, view) in [(Some(epoch), None), (None, Some("page".into()))] {
        let (tx, rx) = mpsc::channel();
        owner.submit(
            set("stale"),
            epoch,
            view,
            Box::new(move |r| tx.send(r).unwrap()),
        );
        assert_eq!(rx.recv().unwrap().unwrap_err().kind, FailureKind::Replaced);
    }
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
    assert_eq!(
        flush
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err()
            .kind,
        FailureKind::Replaced
    );
    lock.execute_batch("ROLLBACK").unwrap();
    discard
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
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
        theme("#123456"),
        Request::Undo { redo: false },
        Request::PutAttachment { bytes: vec![1] },
    ] {
        assert_eq!(
            call(&owner, request).unwrap_err().kind,
            FailureKind::ReadOnly
        );
    }
    call(&owner, Request::Flush).unwrap();
    close(&owner);
    assert_eq!(
        call(&owner, Request::State).unwrap_err().kind,
        FailureKind::Closed
    );
}
#[test]
fn autosave_is_bounded_while_edits_continue() {
    let (_dir, path) = fixture();
    let (owner, events) = open(&path, Mode::Document);
    let start = Instant::now();
    let mut saved = false;
    let mut n = 0;
    while start.elapsed() < Duration::from_millis(1600) {
        call(&owner, set(&format!("{n}"))).unwrap();
        n += 1;
        if events.try_iter().any(|e| {
            matches!(
                e,
                Event::SaveStatus {
                    status: hitslop_core::owner::SaveStatus::Saved,
                    ..
                }
            )
        }) {
            saved = true;
            break;
        }
        // Keep the stream below the idle deadline while allowing realistic processing.
        std::thread::sleep(Duration::from_millis(10));
    }
    // A saving event isn't evidence of persistence: inspect the saved document itself.
    let snapshot = Store::open(&path, Mode::Snapshot).unwrap();
    let durable: Value =
        serde_json::from_str(&snapshot.document().unwrap().value().unwrap()).unwrap();
    assert_ne!(
        durable["title"], "Saved",
        "continuous edits never reached disk (saved event: {saved})"
    );
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
    let copied = submit(
        &owner,
        Request::Copy {
            destination: destination.clone(),
        },
    );
    let closed = submit(
        &owner,
        Request::Close {
            preview: None,
            icon: None,
        },
    );
    assert_eq!(
        call(&owner, set("Too late")).unwrap_err().kind,
        FailureKind::Closing
    );
    lock.execute_batch("ROLLBACK").unwrap();
    copied
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    closed
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
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
        None,
        Instant::now() - Duration::from_millis(1),
        Box::new(move |result| tx.send(result).unwrap()),
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err()
            .kind,
        FailureKind::Closing
    );
    assert_eq!(state(&owner)["value"]["title"], "Saved");
    assert!(!events
        .try_iter()
        .any(|e| matches!(e, Event::Publication { .. })));
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
