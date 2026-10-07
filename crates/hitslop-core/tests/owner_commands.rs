//! Owner coordination across a deliberately delayed child. The actual restricted JS
//! runtime is tested in hitslop-runner; here the oracle is state and lifecycle fencing.
#![cfg(feature = "storage")]
mod support;
use hitslop_core::{
    Origin, file,
    owner::{Failure, FailureKind, Owner, Reply, Request},
    store::Mode,
};
use hitslop_runner::Evaluator;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

struct Fixture {
    dir: tempfile::TempDir,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        support::isolate_registry();
        let dir = tempfile::tempdir().unwrap();
        let stage = dir.path().join("stage");
        fs::create_dir_all(stage.join("assets")).unwrap();
        support::write_app(
            &stage,
            support::App::new(r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#, r#"{"title":"Saved"}"#),
        );
        fs::write(stage.join("assets/ui.js"), "export default {}").unwrap();
        support::edit_input(
            &stage,
            |input| input["declaration"]["commands"] = json!([{"name":"rename","description":"Rename","args":{"kind":"object","properties":{"title":{"kind":"string","minLength":1}}}}]),
        );
        support::add_asset(&stage, "commands.js", "text/javascript", b"globalThis.__slopCommands = {};");
        let template = dir.path().join("Template.slop");
        support::pack(&stage, &template).unwrap();
        let path = dir.path().join("Document.slop");
        file::create_document(&template, &path).unwrap();
        fs::write(
            dir.path().join("child.sh"),
            r#"#!/bin/sh
set -eu
folder="$1"
i=0
while test -f "$folder/started-$i"; do i=$((i + 1)); done
/bin/cat > "$folder/input-$i"
/bin/mv "$folder/input-$i" "$folder/started-$i"
while ! test -f "$folder/reply-$i"; do /bin/sleep 0.01; done
/bin/cat "$folder/reply-$i"
"#,
        )
        .unwrap();
        Self { dir, path }
    }
    fn open(&self, evaluator: bool) -> Owner {
        let evaluator = evaluator.then(|| {
            Evaluator::new(
                PathBuf::from("/bin/sh"),
                vec![
                    self.dir.path().join("child.sh").to_string_lossy().into(),
                    self.dir.path().to_string_lossy().into(),
                ],
            )
            .unwrap()
        });
        Owner::open_with_evaluator(&self.path, Mode::Document, Arc::new(|_| {}), evaluator).unwrap()
    }
    fn input(&self, attempt: usize) -> Value {
        let path = self.dir.path().join(format!("started-{attempt}"));
        let limit = Instant::now() + Duration::from_secs(2);
        while !path.exists() {
            assert!(Instant::now() < limit, "child did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
        let outer: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        serde_json::from_str(outer["request"].as_str().unwrap()).unwrap()
    }
    fn reply(&self, attempt: usize, value: Value) {
        let pending = self.dir.path().join("pending");
        fs::write(&pending, value.to_string()).unwrap();
        fs::rename(pending, self.dir.path().join(format!("reply-{attempt}"))).unwrap();
    }
}
fn submit(owner: &Owner, request: Request, view: Option<&str>) -> mpsc::Receiver<Result<Reply, Failure>> {
    let (tx, rx) = mpsc::channel();
    owner.submit(
        request,
        view.map(str::to_owned),
        Box::new(move |result| {
            let _ = tx.send(result);
        }),
    );
    rx
}
fn receive(rx: mpsc::Receiver<Result<Reply, Failure>>) -> Result<Reply, Failure> {
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}
fn call(owner: &Owner, request: Request) -> Result<Reply, Failure> {
    receive(submit(owner, request, None))
}
fn command(args: Value) -> Request {
    Request::Command { name: "rename".into(), args_json: args.to_string(), origin: Origin::Page }
}
fn edit(title: &str) -> Request {
    Request::Apply {
        batch_json: json!({"intents":[{"type":"set","path":["title"],"value":title}]}).to_string(),
        origin: Origin::Page,
    }
}
fn accepted(title: &str) -> Value {
    json!({"ok":true,"intents":[{"type":"set","path":["title"],"value":title}],"result":title})
}
fn title(owner: &Owner) -> Value {
    let Reply::State { json, .. } = call(owner, Request::State).unwrap() else { panic!("state") };
    serde_json::from_str::<Value>(&json).unwrap()["value"]["title"].clone()
}
fn close(owner: &Owner) {
    call(owner, Request::Close { preview: None, icon: None }).unwrap();
}

#[test]
fn a_command_retries_one_stale_base_with_a_fresh_snapshot_and_stable_clock_and_seed() {
    let fixture = Fixture::new();
    let owner = fixture.open(true);
    let reply = submit(&owner, command(json!({"title":"Command"})), None);
    let first = fixture.input(0);
    // These calls complete while the child is still waiting: it cannot block the owner.
    assert_eq!(title(&owner), "Saved");
    call(&owner, edit("Concurrent")).unwrap();
    fixture.reply(0, accepted("Stale result"));
    let second = fixture.input(1);
    assert_eq!(first["value"]["title"], "Saved");
    assert_eq!(second["value"]["title"], "Concurrent");
    assert_eq!(first["now"], second["now"]);
    assert_eq!(first["seed"], second["seed"]);
    fixture.reply(1, accepted("Command"));
    assert!(matches!(receive(reply).unwrap(), Reply::Command { sequence: 2, .. }));
    assert_eq!(title(&owner), "Command");
    call(&owner, Request::Undo { redo: false }).unwrap();
    assert_eq!(title(&owner), "Concurrent");
    close(&owner);
}

#[test]
fn a_second_conflict_and_invalid_results_are_not_replayed() {
    let fixture = Fixture::new();
    let owner = fixture.open(true);
    let reply = submit(&owner, command(json!({"title":"Command"})), None);
    for attempt in 0..2 {
        fixture.input(attempt);
        call(&owner, edit(&format!("Concurrent {attempt}"))).unwrap();
        fixture.reply(attempt, accepted("Command"));
    }
    assert_eq!(receive(reply).unwrap_err().reason.as_deref(), Some("stale_base"));
    assert_eq!(title(&owner), "Concurrent 1");
    for (attempt, output) in [
        (2, json!({"ok":true,"intents":[],"result":null,"extra":true})),
        (3, json!({"ok":false,"error":"Runtime failed"})),
    ] {
        let reply = submit(&owner, command(json!({"title":"Command"})), None);
        fixture.input(attempt);
        fixture.reply(attempt, output);
        assert_eq!(receive(reply).unwrap_err().kind, FailureKind::Rejected);
        assert_eq!(title(&owner), "Concurrent 1");
    }
    assert!(!fixture.dir.path().join("started-4").exists());
    close(&owner);
}

#[test]
fn arguments_are_checked_before_launch_and_missing_evaluators_never_fall_back() {
    let fixture = Fixture::new();
    let owner = fixture.open(false);
    for args in [json!({}), json!({"title":1}), json!({"title":""}), json!({"title":"Valid","extra":1})] {
        let failure = call(&owner, command(args)).unwrap_err();
        assert!(failure.message.contains("Invalid arguments"));
    }
    let missing = call(&owner, command(json!({"title":"Valid"}))).unwrap_err();
    assert_eq!(
        (missing.message.as_str(), missing.reason.as_deref()),
        ("This hitSlop has no command evaluator", Some("engine_error"))
    );
    assert!(!fixture.dir.path().join("started-0").exists());
    assert_eq!(title(&owner), "Saved");
    close(&owner);
}

#[test]
fn late_commands_cannot_cross_page_replacement_discard_or_close() {
    for action in ["replace", "discard", "close"] {
        let fixture = Fixture::new();
        let owner = fixture.open(true);
        owner.attach("old".into());
        let reply = submit(&owner, command(json!({"title":"Late"})), Some("old"));
        fixture.input(0);
        match action {
            "replace" => owner.attach("new".into()),
            "discard" => {
                call(&owner, Request::Discard).unwrap();
            }
            _ => close(&owner),
        }
        fixture.reply(0, accepted("Late"));
        assert!(receive(reply).is_err(), "{action}");
        if action != "close" {
            assert_eq!(title(&owner), "Saved");
            close(&owner);
        }
        let saved = hitslop_core::store::Store::open(&fixture.path, Mode::Snapshot).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&saved.document().unwrap().value()).unwrap()["title"], "Saved");
    }
}
