// Failure: nested lists and objects inside rows publish wrong paths or order, e.g. a
// nested edit and removal of its containing row in one batch. Oracle: independent patch
// consumer + fresh snapshots. Random nested workloads run in `model.rs`.
mod support;
use hitslop_core::Document;
use support::{app, Edit, View};
use serde_json::{json, Value};
#[test]
fn nested_edit_and_removal_of_its_row_in_one_batch() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/nested.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut d = Document::create(&app(&schema), &f["initial"].to_string()).unwrap();
    let mut projected = View::of(&d);
    let reply = d.apply(&json!({"intents":[
        {"type":"insert","path":["rows",{"id":"row1"},"tags"],"id":"tag9","value":{"label":"l","on":false}},
        {"type":"set","path":["rows",{"id":"row1"},"tags",{"id":"tag1"},"on"],"value":true},
        {"type":"set","path":["rows",{"id":"row1"},"meta","pinned"],"value":true},
        {"type":"remove","path":["rows"],"id":"row1"},
    ]}).to_string()).unwrap();
    projected.publish(&reply);
    projected.check(&d, "batch");
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["ops"], json!([{"type":"deleteRow","path":["rows"],"id":"row1"}]));
}
