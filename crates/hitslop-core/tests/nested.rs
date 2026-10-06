// Failure: nested lists and objects inside rows publish wrong paths or order, e.g. a
// nested edit and removal of its containing row in one import. Oracle: independent patch
// consumer + fresh snapshots. Random nested workloads run in `model.rs`.
mod support;
use hitslop_core::Document;
use support::{Edit, View};
use serde_json::{json, Value};
#[test]
fn nested_edit_and_removal_of_its_row_in_one_import() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/nested.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let mut projected = View::of(&d);
    let mut peer = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
    let base = d.version();
    for op in [
        json!({"type":"insert","path":["rows",{"id":"row1"},"tags"],"id":"tag9","value":{"label":"l","on":false}}),
        json!({"type":"set","path":["rows",{"id":"row1"},"tags",{"id":"tag1"},"on"],"value":true}),
        json!({"type":"set","path":["rows",{"id":"row1"},"meta","pinned"],"value":true}),
        json!({"type":"remove","path":["rows"],"id":"row1"}),
    ] {
        peer.apply(&json!({"intents":[op]}).to_string()).unwrap();
    }
    let reply = d.merge(&peer.export_since(&base).unwrap()).unwrap();
    projected.publish(&reply);
    projected.check(&d, "import");
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["ops"], json!([{"type":"deleteRow","path":["rows"],"id":"row1"}]));
}
