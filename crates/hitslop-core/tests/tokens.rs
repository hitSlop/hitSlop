// Failure: a version token naming operations this owner never saw reached a panicking
// Loro API (`vv_to_frontiers`), poisoning the native owner and aborting WASM.
// Oracle: a typed `stale_base`/`invalid_version` error and an unchanged snapshot.
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
fn foreign_and_malformed_bases_are_refused_without_panicking() {
    let mut d = two_peers();
    let foreign = two_peers().version();
    let before = view(&d);
    for base in [foreign.as_str(), "zz", "", "00"] {
        let request = json!({"base":base,"path":["title"],"from":"abc","to":"abcx","selectionStart":4,"selectionEnd":4});
        let code = d.edit_text(&request.to_string()).unwrap_err().code;
        assert!(["stale_base", "invalid_version"].contains(&code.as_str()), "{base}: {code}");
        let code = d.export_since(base).unwrap_err().code;
        assert!(["stale_base", "invalid_version"].contains(&code.as_str()), "{base}: {code}");
    }
    assert_eq!(view(&d), before);
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
