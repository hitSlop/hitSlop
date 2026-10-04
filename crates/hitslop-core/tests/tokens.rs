// Version tokens: stable across reopen and merge order, and stale before a trimmed
// document's retained history. Bad bases in text edits are refused in `text.rs`.
mod support;
use support::Edit;
use hitslop_core::Document;
use serde_json::{json, Value};
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap()
}
fn view(d: &Document) -> Value {
    serde_json::from_str(&d.snapshot().unwrap()).unwrap()
}
fn create() -> Document {
    let f = fixture();
    Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap()
}
/// A document whose history spans two peers.
fn two_peers() -> Document {
    let mut d = create();
    let base = d.version();
    let mut peer = Document::open(&fixture()["schema"].to_string(), &d.checkpoint().unwrap(), &[]).unwrap();
    peer.apply(&json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}).to_string())
        .unwrap();
    d.import(&peer.export_since(&base).unwrap()).unwrap();
    d
}

#[test]
fn tokens_are_stable_across_reopen_and_merge_order() {
    let d = two_peers();
    let schema = fixture()["schema"].to_string();
    let reopened = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(reopened.version(), d.version());
    // A token from an ancestor still exports exactly the later operations.
    let mut a = create();
    let v0 = a.version();
    let seed = a.checkpoint().unwrap();
    a.apply(&json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}).to_string())
        .unwrap();
    let replayed = Document::open(&schema, &seed, &[a.export_since(&v0).unwrap()]).unwrap();
    assert_eq!(view(&replayed)["value"], view(&a)["value"]);
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
    let mut trimmed = Document::open(&fixture()["schema"].to_string(), &shallow, &[]).unwrap();
    let before = view(&trimmed);
    for (i, token) in tokens.iter().enumerate() {
        // The fast path (the text is still `from`) and the slow path (it changed since).
        for from in ["abc", "changed"] {
            let request = json!({"base":token,"path":["title"],"from":from,"to":"abc!","selectionStart":0,"selectionEnd":0});
            let result = trimmed.edit_text(&request.to_string());
            if i < 2 {
                assert_eq!(result.unwrap_err().code.as_str(), "stale_base", "token {i}, from {from}");
            } else if from == "changed" {
                assert_eq!(result.unwrap_err().code.as_str(), "stale_base", "token {i}: the text was not `from`");
            }
            trimmed = Document::open(&fixture()["schema"].to_string(), &shallow, &[]).unwrap();
        }
        assert_eq!(trimmed.export_since(token).is_ok(), i >= 2, "token {i}");
    }
    assert_eq!(view(&trimmed), before);
}
