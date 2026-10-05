// Version tokens: stable across reopen and replay, and stale before a trimmed document's
// retained history. Bad bases in text edits are refused in `text.rs`.
mod support;
use support::{app, Edit, fixture, knows, snapshot, type_text, updates_since};
use hitslop_core::Document;
use serde_json::json;
fn create() -> Document {
    let f = fixture("checklist");
    Document::create(&app(f["schema"].to_string()), &f["initial"].to_string()).unwrap()
}

#[test]
fn tokens_are_stable_across_reopen_and_replay() {
    let schema = fixture("checklist")["schema"].to_string();
    let mut a = create();
    let seed = a.checkpoint().unwrap();
    a.apply(&json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}).to_string()).unwrap();
    let reopened = Document::open(&app(&schema), &a.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(reopened.version(), a.version());
    let replayed = Document::open(&app(&schema), &seed, &[updates_since(&seed, &a)]).unwrap();
    assert_eq!(snapshot(&replayed)["value"], snapshot(&a)["value"]);
    assert_eq!(replayed.version(), a.version());
}

// Failure: a version from before a trimmed document's history passed validation, which
// only asked whether the version vector covers it; Loro cannot branch or diff from it.
#[test]
fn versions_before_retained_history_are_stale() {
    let mut d = create();
    let mut tokens = vec![d.version()];
    let mut retained = None;
    // One operation per change: the version just before the retained start is one Loro
    // still resolves from the trimmed change it shares with that start.
    for i in 0..4 {
        d.apply(&json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}).to_string()).unwrap();
        tokens.push(d.version());
        if i == 1 {
            // A maintenance checkpoint keeps the history after the previous checkpoint.
            let at = loro::LoroDoc::new();
            at.import(&d.checkpoint().unwrap()).unwrap();
            retained = Some(at.oplog_frontiers());
        }
    }
    let loro = loro::LoroDoc::new();
    loro.import(&d.checkpoint().unwrap()).unwrap();
    let shallow = loro.export(loro::ExportMode::shallow_snapshot(&retained.unwrap())).unwrap();
    let mut trimmed = Document::open(&app(fixture("checklist")["schema"].to_string()), &shallow, &[]).unwrap();
    let before = snapshot(&trimmed);
    for (i, token) in tokens.iter().enumerate() {
        // The fast path (the text is still `from`) and the slow path (it changed since).
        for from in ["abc", "changed"] {
            let result = type_text(&mut trimmed, token, json!(["title"]), from, "abc!", 0);
            if i < 2 {
                assert_eq!(result.unwrap_err().code.as_str(), "stale_base", "token {i}, from {from}");
            } else if from == "changed" {
                assert_eq!(result.unwrap_err().code.as_str(), "stale_base", "token {i}: the text was not `from`");
            }
            trimmed = Document::open(&app(fixture("checklist")["schema"].to_string()), &shallow, &[]).unwrap();
        }
        assert_eq!(knows(&mut trimmed, token, "abc"), i >= 2, "token {i}");
        trimmed = Document::open(&app(fixture("checklist")["schema"].to_string()), &shallow, &[]).unwrap();
    }
    assert_eq!(snapshot(&trimmed), before);
}
