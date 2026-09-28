// Failure: queued drafts branch from merged state and duplicate or lose input.
// Oracle: literal strings/caret offsets and unchanged snapshot after rejection.
// Gap: milestone-one tests only refused stale splices, never rebased drafts.
use hitslop_core::Document;
use serde_json::{json, Value};
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap()
}
fn view(d: &Document) -> Value {
    serde_json::from_str(&d.snapshot().unwrap()).unwrap()
}
fn setup() -> Document {
    let f = fixture();
    Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap()
}
fn request(d: &Document, base: &str, seq: usize, index: usize, text: &str) -> Value {
    let mut r = json!({"session":view(d)["session"],"draft":"typing","sequence":seq,"base":base,"path":["title"],"index":index,"delete":0,"insert":text,"selectionStart":index+text.encode_utf16().count(),"selectionEnd":index+text.encode_utf16().count()});
    if seq > 1 {
        r["parent"] = json!(seq - 1);
    }
    r
}
#[test]
fn queued_draft_uses_authored_parent_not_merged_view() {
    let mut d = setup();
    let base = d.version();
    let r = request(&d, &base, 1, 3, "X").to_string();
    let reply = d.text(&r).unwrap();
    let accepted = view(&d);
    assert_eq!(d.text(&r).unwrap(), reply);
    assert_eq!(view(&d), accepted);
    let mut conflict: Value = serde_json::from_str(&r).unwrap();
    conflict["insert"] = json!("oops");
    assert_eq!(
        d.text(&conflict.to_string()).unwrap_err().code,
        "request_conflict"
    );
    // A command enters through the same owner between draft acknowledgements.
    d.command_current(
        &json!({"intents":[{"type":"splice","path":["title"],"index":0,"delete":0,"insert":"R"}]})
            .to_string(),
    )
    .unwrap();
    let r = request(&d, &base, 2, 4, "Y");
    d.text(&r.to_string()).unwrap();
    let r = request(&d, &base, 3, 5, "Z");
    let reply: Value = serde_json::from_str(&d.text(&r.to_string()).unwrap()).unwrap();
    assert_eq!(view(&d)["value"]["title"], "RabcXYZ");
    assert_eq!(reply["text"]["selectionStart"], 7);
    let reopened = Document::open(
        &fixture()["schema"].to_string(),
        &d.checkpoint().unwrap(),
        &[],
    )
    .unwrap();
    assert_eq!(view(&reopened)["value"]["title"], "RabcXYZ");
    d.release_draft("typing").unwrap();
    assert_eq!(d.text(&r.to_string()).unwrap_err().code, "unknown_outcome");
}
#[test]
fn emoji_selection_and_bad_parent_do_not_mutate_owner() {
    let mut d = setup();
    let base = d.version();
    let r = request(&d, &base, 1, 3, "😀é");
    let response: Value = serde_json::from_str(&d.text(&r.to_string()).unwrap()).unwrap();
    assert_eq!(response["text"]["selectionStart"], 7);
    assert_eq!(view(&d)["value"]["title"], "abc😀é");
    let before = view(&d);
    let mut bad = request(&d, &base, 3, 7, "bad");
    assert_eq!(d.text(&bad.to_string()).unwrap_err().code, "draft_parent");
    assert_eq!(view(&d), before);
    bad = request(&d, &base, 2, 7, "!");
    bad["selectionStart"] = json!(4);
    assert_eq!(d.text(&bad.to_string()).unwrap_err().code, "out_of_range");
    assert_eq!(view(&d), before);
}
#[test]
fn removed_focused_row_cannot_be_resurrected() {
    let mut d = setup();
    let base = d.version();
    let id = fixture()["initial"]["rows"][0]["$id"].clone();
    let mut r = request(&d, &base, 1, 1, "X");
    r["path"] = json!(["rows",{"id":id},"text"]);
    d.text(&r.to_string()).unwrap();
    d.apply(&json!({"intents":[{"type":"remove","path":["rows"],"id":id}]}).to_string())
        .unwrap();
    let before = view(&d);
    r["sequence"] = json!(2);
    r["parent"] = json!(1);
    r["index"] = json!(2);
    assert_eq!(d.text(&r.to_string()).unwrap_err().code, "path_not_found");
    assert_eq!(view(&d), before);
}
#[test]
fn actual_remote_update_and_draft_merge_preserve_both() {
    let mut d = setup();
    let base = d.version();
    let mut peer = Document::open(
        &fixture()["schema"].to_string(),
        &d.checkpoint().unwrap(),
        &[],
    )
    .unwrap();
    let r = request(&d, &base, 1, 3, "X");
    d.text(&r.to_string()).unwrap();
    peer.apply(&json!({"intents":[{"type":"splice","path":["title"],"base":base,"index":0,"delete":0,"insert":"遠"}]}).to_string()).unwrap();
    d.import(&peer.export_since(&base).unwrap()).unwrap();
    let r = request(&d, &base, 2, 4, "YZ");
    d.text(&r.to_string()).unwrap();
    assert_eq!(view(&d)["value"]["title"], "遠abcXYZ");
}

#[test]
fn current_command_does_not_reinterpret_an_authored_base() {
    let mut d = setup();
    let before = view(&d);
    let request=json!({"intents":[{"type":"splice","path":["title"],"base":d.version(),"index":0,"delete":0,"insert":"oops"}]}).to_string();
    assert_eq!(
        d.command_current(&request).unwrap_err().code,
        "invalid_request"
    );
    assert_eq!(view(&d), before);
}
