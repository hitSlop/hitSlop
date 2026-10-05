// Optional text in a collection. Failure: typing into an unset optional text does not
// create it, or an edit from cleared text resurrects it. Random workloads run in
// `model.rs`. Oracle: literal values.
mod support;
use hitslop_core::Document;
use support::{app, Edit, fixture, snapshot};
use serde_json::json;



#[test]
fn page_typing_into_an_unset_optional_text_creates_it() {
    let f = fixture("collections");
    let mut d = Document::create(&app(f["schema"].to_string()), &f["initial"].to_string()).unwrap();
    let base = d.version();
    let request = json!({"base":base,"path":["notes"],"from":"","to":"Hi","selectionStart":2,"selectionEnd":2});
    let edit = d.edit_text(&request.to_string()).unwrap();
    assert_eq!(snapshot(&d)["value"]["notes"], "Hi");
    let next = json!({"base":edit.authored,"path":["notes"],"from":"Hi","to":"Hi there","selectionStart":8,"selectionEnd":8});
    d.edit_text(&next.to_string()).unwrap();
    assert_eq!(snapshot(&d)["value"]["notes"], "Hi there");
    // After a clear, an edit from the old text is refused rather than resurrecting it.
    d.apply(&json!({"intents":[{"type":"clear","path":["notes"]}]}).to_string()).unwrap();
    let stale = json!({"base":d.version(),"path":["notes"],"from":"Hi there","to":"Hi there!","selectionStart":9,"selectionEnd":9});
    assert_eq!(d.edit_text(&stale.to_string()).unwrap_err().code.as_str(), "path_not_found");
}

