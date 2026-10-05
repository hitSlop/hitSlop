// Records, scalar lists and optional text. Failure: replicas that disagree after
// exchanging updates, or a merged anomaly repaired on read. Concurrent creation is covered
// in `concurrent_creation.rs`, random workloads in `model.rs`. Oracle: equal snapshots on
// both replicas and literal issues.
mod support;
use hitslop_core::Document;
use support::{Edit, exchange, fixture, pair, snapshot};
use loro::{ExportMode, LoroDoc, LoroMap};
use serde_json::{json, Value};

fn apply(d: &mut Document, intents: Value) {
    d.apply(&json!({ "intents": intents }).to_string()).unwrap();
}

#[test]
fn concurrent_edits_to_fields_of_one_entry_both_survive() {
    let (mut a, mut b, _) = pair(&fixture("collections"));
    apply(&mut a, json!([{"type":"set","path":["cells","A1"],"value":{"input":"start"}}]));
    b.import(&a.export_since(&b.version()).unwrap()).unwrap();
    let base = a.version();
    apply(&mut a, json!([{"type":"set","path":["cells","A1","input"],"value":"typed"}]));
    apply(&mut b, json!([{"type":"set","path":["cells","A1","tint"],"value":"blue"}]));
    exchange(&mut a, &mut b, &base);
    assert_eq!(snapshot(&a)["value"]["cells"]["A1"], json!({"input":"typed","tint":"blue"}));
    assert_eq!(snapshot(&a)["value"], snapshot(&b)["value"]);
}

#[test]
fn concurrent_scalar_list_inserts_both_survive_and_sets_converge() {
    let (mut a, mut b, base) = pair(&fixture("collections"));
    apply(&mut a, json!([{"type":"insert","path":["presets"],"value":100,"index":1},{"type":"set","path":["pixels",{"index":0}],"value":"#a00"}]));
    apply(&mut b, json!([{"type":"insert","path":["presets"],"value":200},{"type":"set","path":["pixels",{"index":0}],"value":"#0b0"}]));
    exchange(&mut a, &mut b, &base);
    let presets = snapshot(&a)["value"]["presets"].clone();
    assert_eq!(presets, json!([60, 100, 90, 200]));
    assert_eq!(snapshot(&a)["value"], snapshot(&b)["value"]);
    assert!(["#a00", "#0b0"].contains(&snapshot(&a)["value"]["pixels"][0].as_str().unwrap()));
}

#[test]
fn page_typing_into_an_unset_optional_text_creates_it() {
    let f = fixture("collections");
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
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

#[test]
fn merged_invalid_entries_and_elements_are_flagged_not_repaired() {
    let f = fixture("collections");
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    let data = peer.get_map("data");
    let widths = data.get("widths").unwrap().into_container().unwrap().into_map().unwrap();
    widths.insert("B", "wide").unwrap();
    let presets = data.get("presets").unwrap().into_container().unwrap().into_movable_list().unwrap();
    presets.insert(0, 999i64).unwrap();
    let done = data.get("done").unwrap().into_container().unwrap().into_map().unwrap();
    let _: LoroMap = done.insert_container("__proto__", LoroMap::new()).unwrap();
    peer.commit();
    d.import(&peer.export(ExportMode::updates(&from)).unwrap()).unwrap();
    let snapshot = snapshot(&d);
    assert_eq!(snapshot["value"]["widths"]["B"], "wide");
    assert_eq!(snapshot["value"]["presets"], json!([999, 60, 90]));
    assert_eq!(
        snapshot["issues"],
        json!([
            {"code":"invalid_key","path":["done","__proto__"]},
            {"code":"out_of_range","path":["presets",{"index":0}]},
            {"code":"type_mismatch","path":["widths","B"]}
        ])
    );
}
