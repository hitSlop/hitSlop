// Version tokens: opaque, compared for equality by a command's `ifVersion`, and stable
// across reopen and replay.
mod support;
use hitslop_core::Document;
use serde_json::json;
use support::{Edit, app, fixture, snapshot, updates_since};
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
