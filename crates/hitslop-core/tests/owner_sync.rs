#![cfg(feature = "dev-sync")]
use hitslop_core::{
    Origin, file,
    owner::{
        Event, Failure, FailureKind, Owner, Reply, Request,
        session::{Forwarder, SyncCompletion},
    },
    store::{Mode, Store},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::Duration,
};
mod support;
fn wait<T>(receive: mpsc::Receiver<T>) -> T {
    receive.recv_timeout(Duration::from_secs(20)).expect("owner completion")
}
fn completion_pair() -> (hitslop_core::owner::Completion, mpsc::Receiver<Result<Reply, Failure>>) {
    let (send, receive) = mpsc::channel();
    (
        Box::new(move |r| {
            let _ = send.send(r);
        }),
        receive,
    )
}
fn call(owner: &Owner, request: Request) -> Result<Reply, Failure> {
    let (callback, receive) = completion_pair();
    owner.submit(request, None, callback);
    wait(receive)
}
fn edit(by: i64) -> Request {
    Request::Apply {
        origin: Origin::Page,
        batch: serde_json::from_value(json!({"intents":[{"type":"increment","path":["count"],"by":by}]})).unwrap(),
    }
}
fn value(owner: &Owner) -> Value {
    let Reply::State { reading, .. } = call(owner, Request::State).unwrap() else { panic!() };
    serde_json::to_value(reading).unwrap()["value"].clone()
}
fn open(path: &Path) -> Arc<Owner> {
    Arc::new(Owner::open(path, Mode::Document, Arc::new(|_| {})).unwrap())
}
fn fixture() -> (tempfile::TempDir, PathBuf) {
    support::isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/ui.js"), "export default {}").unwrap();
    support::write_app(
        &stage,
        support::App::new(r#"{"kind":"object","properties":{"count":{"kind":"counter"}}}"#, r#"{"count":0}"#),
    );
    support::edit_input(&stage, |input| {
        input["declaration"]["commands"] = json!([{
            "name":"increment", "description":"Increment", "args":{"kind":"object","properties":{}}
        }])
    });
    support::add_asset(&stage, "commands.js", "text/javascript", b"globalThis.__slopCommands = {};");
    let template = dir.path().join("template.slop");
    support::pack(&stage, &template).unwrap();
    let document = dir.path().join("authority.slop");
    file::create_document(&template, &document).unwrap();
    (dir, document)
}
fn exported(owner: &Owner) -> (hitslop_core::owner::session::Identity, Vec<u8>, Vec<u8>) {
    let (send, receive) = mpsc::channel();
    owner.sync_export(Box::new(move |r| {
        let _ = send.send(r);
    }));
    wait(receive).unwrap()
}
fn replica(owner: &Owner, path: &Path, forward: Forwarder) -> Arc<Owner> {
    let (callback, receive) = completion_pair();
    owner.submit(Request::Backup { destination: path.to_owned() }, None, callback);
    wait(receive).unwrap();
    let copy = open(path);
    let (identity, _, _) = exported(owner);
    let (callback, receive) = completion_pair();
    copy.sync_replica(identity, forward, callback);
    wait(receive).unwrap();
    copy
}
fn authority(owner: &Owner) {
    let (callback, receive) = completion_pair();
    owner.sync_authority(Arc::new(|_| {}), callback);
    wait(receive).unwrap();
}
#[test]
fn replica_routes_edits_through_authority_and_saves_before_reply() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let remote = owner.clone();
    let forward: Forwarder = Arc::new(move |request, base, callback| remote.sync_submit(request, base, callback));
    let copy_path = dir.path().join("replica.slop");
    let copy = replica(&owner, &copy_path, forward);
    call(&copy, edit(7)).unwrap();
    call(&copy, edit(4)).unwrap();
    assert_eq!(value(&owner), json!({"count":11}));
    assert_eq!(value(&copy), value(&owner));
    assert_eq!(exported(&owner).2, exported(&copy).2);
    for path in [&path, &copy_path] {
        let store = Store::open(path, Mode::Snapshot).unwrap();
        assert_eq!(store.document().unwrap().value(), r#"{"count":11}"#);
    }
    call(&copy, Request::Close { preview: None, icon: None }).unwrap();
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn flush_backup_and_close_wait_for_previously_forwarded_admissions() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let (send, receive) = mpsc::channel::<(Request, Vec<u8>, SyncCompletion)>();
    let copy = replica(
        &owner,
        &dir.path().join("replica.slop"),
        Arc::new(move |r, b, c| {
            send.send((r, b, c)).unwrap();
        }),
    );
    let (done, result) = completion_pair();
    copy.submit(edit(1), None, done);
    let (request, base, complete) = wait(receive);
    let (flush, flushed) = completion_pair();
    copy.submit(Request::Flush, None, flush);
    let backup_path = dir.path().join("backup.slop");
    let (backup, backed_up) = completion_pair();
    copy.submit(Request::Backup { destination: backup_path.clone() }, None, backup);
    let (close, closed) = completion_pair();
    copy.submit(Request::Close { preview: None, icon: None }, None, close);
    assert!(flushed.recv_timeout(Duration::from_millis(30)).is_err());
    assert!(closed.try_recv().is_err());
    assert!(backed_up.try_recv().is_err());
    assert_eq!(call(&copy, edit(10)).unwrap_err().kind, FailureKind::Closing);
    owner.sync_submit(request, base, complete);
    wait(result).unwrap();
    wait(flushed).unwrap();
    wait(backed_up).unwrap();
    wait(closed).unwrap();
    let backup = Store::open(&backup_path, Mode::Snapshot).unwrap();
    assert_eq!(backup.document().unwrap().value(), r#"{"count":1}"#);
    assert_eq!(backup.app_digest().unwrap(), exported(&owner).0.app_digest);
    assert_eq!(value(&owner), json!({"count":1}));
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn shared_restrictions_and_disconnect_are_enforced_inside_owner() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let remote = owner.clone();
    let copy = replica(&owner, &dir.path().join("replica.slop"), Arc::new(move |r, b, c| remote.sync_submit(r, b, c)));
    for request in [
        Request::Undo { redo: false },
        Request::Undo { redo: true },
        Request::Discard,
        Request::PutAttachment { bytes: vec![1] },
    ] {
        assert_eq!(call(&copy, request).unwrap_err().kind, FailureKind::Rejected);
    }
    let (callback, receive) = completion_pair();
    copy.sync_disconnect(callback);
    wait(receive).unwrap();
    assert_eq!(call(&copy, edit(1)).unwrap_err().kind, FailureKind::Busy);
    assert_eq!(value(&copy), json!({"count":0}));
    call(&copy, Request::Close { preview: None, icon: None }).unwrap();
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn unknown_outcome_is_not_replayed_and_fences_close() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let remote = owner.clone();
    let copy = replica(
        &owner,
        &dir.path().join("replica.slop"),
        Arc::new(move |r, b, c| {
            remote.sync_submit(
                r,
                b,
                Box::new(move |outcome| {
                    outcome.unwrap();
                    c(Err(Failure {
                        kind: FailureKind::Failed,
                        message: "Connection lost after acceptance".into(),
                        reason: None,
                        op_index: None,
                    }));
                }),
            );
        }),
    );
    assert_eq!(call(&copy, edit(1)).unwrap_err().kind, FailureKind::Failed);
    assert_eq!(value(&owner), json!({"count":1}));
    assert_eq!(value(&copy), json!({"count":0}));
    assert_eq!(call(&copy, edit(1)).unwrap_err().kind, FailureKind::Failed);
    assert_eq!(call(&copy, Request::Close { preview: None, icon: None }).unwrap_err().kind, FailureKind::Failed);
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn snapshot_reset_publishes_after_durable_replacement() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let copy_path = dir.path().join("replica.slop");
    call(&owner, Request::CaptureSource { destination: copy_path.clone() }).unwrap();
    let (events, receive) = mpsc::channel();
    let copy = Owner::open(
        &copy_path,
        Mode::Document,
        Arc::new(move |e| {
            events.send(e).unwrap();
        }),
    )
    .unwrap();
    let (identity, _, _) = exported(&owner);
    let remote = owner.clone();
    let (callback, ready) = completion_pair();
    copy.sync_replica(identity.clone(), Arc::new(move |r, b, c| remote.sync_submit(r, b, c)), callback);
    wait(ready).unwrap();
    call(&owner, edit(12)).unwrap();
    let (_, bytes, _) = exported(&owner);
    let (callback, ready) = completion_pair();
    copy.sync_snapshot(identity, bytes, callback);
    wait(ready).unwrap();
    assert_eq!(value(&copy), json!({"count":12}));
    let publications: Vec<_> = receive
        .try_iter()
        .filter_map(|e| if let Event::Publication { json } = e { Some(json) } else { None })
        .collect();
    assert_eq!(publications.len(), 1);
    let publication: Value = serde_json::from_str(&publications[0]).unwrap();
    assert_eq!(publication["sequence"], 1);
    assert_eq!(publication["ops"][0]["value"]["count"], 12);
    let saved = Store::open(&copy_path, Mode::Snapshot).unwrap();
    assert_eq!(saved.document().unwrap().value(), r#"{"count":12}"#);
    call(&copy, Request::Close { preview: None, icon: None }).unwrap();
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn history_only_updates_persist_and_duplicates_do_not_publish() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let remote = owner.clone();
    let copy_path = dir.path().join("replica.slop");
    let copy = replica(&owner, &copy_path, Arc::new(move |r, b, c| remote.sync_submit(r, b, c)));
    let (identity, snapshot, _) = exported(&owner);
    let raw = loro::LoroDoc::new();
    raw.import(&snapshot).unwrap();
    let before = raw.oplog_vv();
    raw.get_map("meta").insert("proof-only", "metadata").unwrap();
    raw.commit();
    let updates = raw.export(loro::ExportMode::updates(&before)).unwrap();
    for _ in 0..2 {
        let (callback, ready) = completion_pair();
        copy.sync_install(identity.clone(), updates.clone(), callback);
        wait(ready).unwrap();
    }
    let Reply::State { sequence, .. } = call(&copy, Request::State).unwrap() else { panic!() };
    assert_eq!(sequence, 1);
    assert_eq!(value(&copy), json!({"count":0}));
    let saved = Store::open(&copy_path, Mode::Snapshot).unwrap();
    let stored = saved.document().unwrap();
    assert_eq!(loro::VersionVector::decode(&stored.version_vector()).unwrap(), raw.oplog_vv());
    call(&copy, Request::Close { preview: None, icon: None }).unwrap();
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn failed_authority_save_withholds_broadcast_until_successful_retry() {
    let (_dir, path) = fixture();
    let owner = open(&path);
    let (updates, delivered) = mpsc::channel();
    let (callback, ready) = completion_pair();
    owner.sync_authority(
        Arc::new(move |update| {
            updates.send(update).unwrap();
        }),
        callback,
    );
    wait(ready).unwrap();
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert_eq!(call(&owner, edit(3)).unwrap_err().kind, FailureKind::Busy);
    assert!(delivered.try_recv().is_err());
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    call(&owner, Request::Flush).unwrap();
    let update = wait(delivered);
    let raw = loro::LoroDoc::new();
    let saved = Store::open(&path, Mode::Snapshot).unwrap();
    raw.import(&saved.document().unwrap().checkpoint().unwrap()).unwrap();
    assert_eq!(loro::VersionVector::decode(&update.after).unwrap(), raw.oplog_vv());
    assert_eq!(value(&owner), json!({"count":3}));
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn failed_snapshot_save_keeps_old_core_and_flush_retries_candidate() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let remote = owner.clone();
    let copy_path = dir.path().join("replica.slop");
    let copy = replica(&owner, &copy_path, Arc::new(move |r, b, c| remote.sync_submit(r, b, c)));
    call(&owner, edit(8)).unwrap();
    let (identity, snapshot, _) = exported(&owner);
    let lock = rusqlite::Connection::open(&copy_path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let (callback, ready) = completion_pair();
    copy.sync_snapshot(identity, snapshot, callback);
    assert_eq!(wait(ready).unwrap_err().kind, FailureKind::Busy);
    assert_eq!(value(&copy), json!({"count":0}));
    assert_eq!(call(&copy, edit(1)).unwrap_err().kind, FailureKind::Busy);
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    call(&copy, Request::Flush).unwrap();
    assert_eq!(value(&copy), json!({"count":8}));
    let saved = Store::open(&copy_path, Mode::Snapshot).unwrap();
    assert_eq!(saved.document().unwrap().value(), r#"{"count":8}"#);
    call(&copy, Request::Close { preview: None, icon: None }).unwrap();
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn forwarded_command_runs_only_on_authority_and_export_close_wait_for_it() {
    let (dir, path) = fixture();
    let script = dir.path().join("evaluate.sh");
    std::fs::write(
        &script,
        r#"#!/bin/sh
set -eu
/bin/cat > "$1/started"
while ! test -f "$1/release"; do /bin/sleep 0.01; done
/bin/echo '{"ok":true,"intents":[{"type":"increment","path":["count"],"by":5}],"result":5}'
"#,
    )
    .unwrap();
    let evaluator = hitslop_core::owner::Evaluator::new(
        PathBuf::from("/bin/sh"),
        vec![script.to_string_lossy().into(), dir.path().to_string_lossy().into()],
    )
    .unwrap();
    let owner = Arc::new(Owner::open_with_evaluator(&path, Mode::Document, Arc::new(|_| {}), Some(evaluator)).unwrap());
    authority(&owner);
    let remote = owner.clone();
    let copy = replica(&owner, &dir.path().join("replica.slop"), Arc::new(move |r, b, c| remote.sync_submit(r, b, c)));
    let (callback, result) = completion_pair();
    copy.submit(
        Request::Command { name: "increment".into(), args_json: "{}".into(), origin: Origin::Page },
        None,
        callback,
    );
    let timeout = std::time::Instant::now() + Duration::from_secs(3);
    while !dir.path().join("started").exists() {
        assert!(std::time::Instant::now() < timeout);
        std::thread::sleep(Duration::from_millis(5));
    }
    let (send, export) = mpsc::channel();
    owner.sync_export(Box::new(move |r| {
        send.send(r).unwrap();
    }));
    let (callback, closed) = completion_pair();
    copy.submit(Request::Close { preview: None, icon: None }, None, callback);
    assert!(export.recv_timeout(Duration::from_millis(30)).is_err());
    assert!(closed.try_recv().is_err());
    std::fs::write(dir.path().join("release"), "go").unwrap();
    assert!(matches!(wait(result).unwrap(), Reply::Command { result_json, .. } if result_json == "5"));
    wait(closed).unwrap();
    let (_, snapshot, _) = wait(export).unwrap();
    let raw = loro::LoroDoc::new();
    raw.import(&snapshot).unwrap();
    assert!(matches!(raw.get_map("data").get("count"), Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(5)))));
    assert_eq!(value(&owner), json!({"count":5}));
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn an_uninstallable_accepted_outcome_is_unknown_not_a_definite_refusal() {
    let (dir, path) = fixture();
    let owner = open(&path);
    authority(&owner);
    let remote = owner.clone();
    let copy = replica(
        &owner,
        &dir.path().join("replica.slop"),
        Arc::new(move |request, base, callback| {
            remote.sync_submit(
                request,
                base,
                Box::new(move |result| {
                    let mut accepted = result.unwrap();
                    accepted.updates = vec![0, 1, 2];
                    callback(Ok(accepted));
                }),
            );
        }),
    );
    let error = call(&copy, edit(1)).unwrap_err();
    assert_eq!(error.kind, FailureKind::Failed, "the authority already accepted and saved this edit");
    assert_eq!(value(&owner), json!({"count":1}));
    assert_eq!(value(&copy), json!({"count":0}));
    assert_eq!(call(&copy, edit(1)).unwrap_err().kind, FailureKind::Failed);
    assert_eq!(value(&owner), json!({"count":1}), "uncertain edits are never replayed");
    call(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
