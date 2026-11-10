//! Owner coordination across a deliberately delayed child. The actual restricted JS
//! runtime is tested in pop-runner; here the oracle is state and lifecycle fencing.
use crate::support;
use pop_contracts::wire::Code;
use pop_core::Origin;
use pop_runner::Evaluator;
use pop_runtime::owner::{Failure, FailureKind, Owner, Reply, Request};
use pop_storage::file;
use pop_storage::store::Mode;
use pop_testkit::owner::{call, close, submit};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

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
        support::add_asset(&stage, "commands.js", "text/javascript", b"globalThis.__popCommands = {};");
        let template = dir.path().join("Template.pop");
        support::pack(&stage, &template).unwrap();
        let path = dir.path().join("Document.pop");
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
            Evaluator::process(
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
fn receive(rx: mpsc::Receiver<Result<Reply, Failure>>) -> Result<Reply, Failure> {
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}
fn command(args: Value) -> Request {
    Request::Command { name: "rename".into(), args_json: args.to_string(), origin: Origin::Page }
}
fn edit(title: &str) -> Request {
    Request::Apply {
        batch: serde_json::from_value(json!({"intents":[{"type":"set","path":["title"],"value":title}]})).unwrap(),
        origin: Origin::Page,
    }
}
fn accepted(title: &str) -> Value {
    json!({"ok":true,"intents":[{"type":"set","path":["title"],"value":title}],"result":title})
}
fn title(owner: &Owner) -> Value {
    let Reply::State { reading, .. } = call(owner, Request::State).unwrap() else { panic!("state") };
    let json = serde_json::to_string(&reading).unwrap();
    serde_json::from_str::<Value>(&json).unwrap()["value"]["title"].clone()
}

#[test]
fn saved_state_fences_wait_for_an_earlier_command() {
    let fixture = Fixture::new();
    let owner = fixture.open(true);
    let command = submit(&owner, command(json!({"title":"Included"})), None);
    fixture.input(0);
    let flushed = submit(&owner, Request::Flush, None);
    let copy = fixture.dir.path().join("Copy.pop");
    let copied = submit(&owner, Request::Copy { destination: copy.clone(), preview: None, icon: None }, None);
    let closed = submit(&owner, Request::Close { preview: None, icon: None }, None);
    assert_eq!(title(&owner), "Saved"); // The fences have reached the owner; reads still work.
    let premature = flushed.try_recv();
    assert_eq!(call(&owner, edit("too late")).unwrap_err().kind, FailureKind::Closing);
    fixture.reply(0, accepted("Included")); // Always release the child before an assertion.
    assert!(premature.is_err(), "flush must wait for the command outcome");
    receive(command).unwrap();
    receive(flushed).unwrap();
    receive(copied).unwrap();
    let saved = pop_storage::store::Store::open(&copy, Mode::Snapshot).unwrap().document().unwrap();
    assert_eq!(serde_json::from_str::<Value>(&saved.value()).unwrap()["title"], "Included");
    receive(closed).unwrap();
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
    assert_eq!(receive(reply).unwrap_err().reason, Some(Code::StaleBase));
    assert_eq!(title(&owner), "Concurrent 1");
    for (attempt, output) in
        [(2, json!({"ok":true,"intents":{},"result":null})), (3, json!({"ok":false,"error":"Runtime failed"}))]
    {
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
fn command_envelopes_ignore_unknown_members_but_keep_required_fields_and_intents_strict() {
    let fixture = Fixture::new();
    let owner = fixture.open(true);
    let reply = submit(&owner, command(json!({"title":"Command"})), None);
    fixture.input(0);
    let mut output = accepted("Command");
    output["extra"] = json!(1);
    fixture.reply(0, output);
    assert!(matches!(receive(reply).unwrap(), Reply::Command { sequence: 1, .. }));
    assert_eq!(title(&owner), "Command");

    let reply = submit(&owner, command(json!({"title":"Ignored"})), None);
    fixture.input(1);
    fixture.reply(1, json!({"ok":false,"error":"Enter a title.","refused":true,"extra":1}));
    let failure = receive(reply).unwrap_err();
    assert_eq!((failure.kind, failure.reason), (FailureKind::Rejected, Some(Code::Refused)));
    assert_eq!(failure.message, "Enter a title.");

    for (attempt, output) in [
        json!({"ok":true,"intents":[{"type":"set","path":["title"],"value":"Ignored","extra":1}],"result":null,"extra":1}),
        json!({"ok":true,"intents":[]}),
        json!({"ok":true,"result":null}),
        json!({"ok":"true","intents":[],"result":null}),
        json!({"ok":true,"intents":{},"result":null}),
        json!({"ok":false,"refused":true,"extra":1}),
        json!({"ok":false,"error":1,"refused":true,"extra":1}),
        json!({"ok":false,"error":"Enter a title.","refused":1,"extra":1}),
    ].into_iter().enumerate() {
        let reply = submit(&owner, command(json!({"title":"Ignored"})), None);
        fixture.input(attempt + 2);
        fixture.reply(attempt + 2, output);
        let failure = receive(reply).unwrap_err();
        assert_eq!((failure.kind, failure.reason), (FailureKind::Rejected, Some(Code::EngineError)));
        assert!(failure.message.contains("it returned an invalid reply"));
        assert_eq!(title(&owner), "Command");
    }
    close(&owner);
}

// A command's reply stays raw JSON until the arm for its runtime ABI decodes the intents, so
// an intent this core cannot decode is the evaluator's fault: a definite refusal, applied to
// nothing, and the owner is left as it was.
#[test]
fn an_intent_that_does_not_decode_is_an_invalid_reply_and_applies_nothing() {
    let fixture = Fixture::new();
    let owner = fixture.open(true);
    let undecodable = [
        json!({"type":"teleport","path":["title"]}),
        json!({"type":"set","path":["title"],"value":"Command","unexpected":1}),
        json!({"type":"increment","path":["title"],"by":"1"}),
        json!({"type":"set","value":"Command"}),
        json!("set"),
    ];
    for (attempt, intent) in undecodable.into_iter().enumerate() {
        let reply = submit(&owner, command(json!({"title":"Command"})), None);
        fixture.input(attempt);
        fixture.reply(attempt, json!({"ok":true,"intents":[intent],"result":null}));
        let failure = receive(reply).unwrap_err();
        assert_eq!((failure.kind, failure.reason), (FailureKind::Rejected, Some(Code::EngineError)), "{attempt}");
        assert!(failure.message.contains("it returned an invalid reply"), "{attempt}: {}", failure.message);
        assert_eq!(title(&owner), "Saved");
    }
    // Nothing was applied, so the next accepted command is the document's first edit.
    let reply = submit(&owner, command(json!({"title":"Command"})), None);
    fixture.input(5);
    fixture.reply(5, accepted("Command"));
    assert!(matches!(receive(reply).unwrap(), Reply::Command { sequence: 1, .. }));
    assert_eq!(title(&owner), "Command");
    close(&owner);
}

#[test]
fn a_refusal_for_the_person_keeps_its_message_under_its_own_reason() {
    let fixture = Fixture::new();
    let owner = fixture.open(true);
    for (attempt, output, reason) in [
        (0, json!({"ok":false,"error":"Enter a title.","refused":true}), Code::Refused),
        (1, json!({"ok":false,"error":"Enter a title."}), Code::InvalidRequest),
    ] {
        let reply = submit(&owner, command(json!({"title":"Command"})), None);
        fixture.input(attempt);
        fixture.reply(attempt, output);
        let failure = receive(reply).unwrap_err();
        assert_eq!((failure.kind, failure.reason), (FailureKind::Rejected, Some(reason)));
        assert_eq!(failure.message, "Enter a title.");
    }
    assert_eq!(title(&owner), "Saved");
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
        (missing.message.as_str(), missing.reason),
        ("This pop has no command evaluator", Some(Code::EngineError))
    );
    assert!(!fixture.dir.path().join("started-0").exists());
    assert_eq!(title(&owner), "Saved");
    close(&owner);
}

#[test]
fn late_commands_cannot_cross_page_replacement_or_discard() {
    for action in ["replace", "discard"] {
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
            _ => unreachable!(),
        }
        fixture.reply(0, accepted("Late"));
        assert!(receive(reply).is_err(), "{action}");
        assert_eq!(title(&owner), "Saved");
        close(&owner);
        let saved = pop_storage::store::Store::open(&fixture.path, Mode::Snapshot).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&saved.document().unwrap().value()).unwrap()["title"], "Saved");
    }
}
