// Optional text in a collection. Failure: typing into an unset optional text does not
// create it, or an edit from cleared text resurrects it. Random workloads run in
// `model.rs`. Oracle: literal values.
mod support;
use hitslop_core::Document;
use serde_json::json;
use support::{Edit, app, fixture, snapshot, type_text};

#[test]
fn page_typing_into_an_unset_optional_text_creates_it() {
    let f = fixture("collections");
    let mut d = Document::create(&app(f["schema"].to_string()), &f["initial"].to_string()).unwrap();
    let base = d.version();
    let edit = type_text(&mut d, &base, json!(["notes"]), "", "Hi", 2).unwrap();
    assert_eq!(snapshot(&d)["value"]["notes"], "Hi");
    type_text(&mut d, &edit.text.unwrap().authored, json!(["notes"]), "Hi", "Hi there", 8).unwrap();
    assert_eq!(snapshot(&d)["value"]["notes"], "Hi there");
    // After a clear, an edit from the old text is refused rather than resurrecting it.
    d.apply(&json!({"intents":[{"type":"clear","path":["notes"]}]}).to_string()).unwrap();
    let version = d.version();
    let stale = type_text(&mut d, &version, json!(["notes"]), "Hi there", "Hi there!", 9);
    assert_eq!(stale.unwrap_err().code.as_str(), "path_not_found");
}
