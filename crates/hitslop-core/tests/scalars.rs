// Scalars merge last-writer-wins per field (concurrently created objects merge field by
// field: tests/concurrent_creation.rs). Failure: replicas that disagree after exchanging
// updates, or a merged anomaly repaired on read. Oracle: equal snapshots on both replicas
// and literal issues.
mod support;
use support::{Edit, exchange, fixture, pair, snapshot};
use hitslop_core::Document;
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};

fn apply(d: &mut Document, intents: Value) {
    d.apply(&json!({ "intents": intents }).to_string()).unwrap();
}

#[test]
fn concurrent_scalar_sets_converge_on_one_value() {
    let (mut a, mut b, base) = pair(&fixture("scalars"));
    apply(&mut a, json!([{"type":"set","path":["currency"],"value":"USD"},{"type":"set","path":["rating"],"value":1}]));
    apply(&mut b, json!([{"type":"set","path":["currency"],"value":"EUR"},{"type":"set","path":["rating"],"value":5}]));
    exchange(&mut a, &mut b, &base);
    assert_eq!(snapshot(&a)["value"], snapshot(&b)["value"]);
    assert!(["USD", "EUR"].contains(&snapshot(&a)["value"]["currency"].as_str().unwrap()));
    assert_eq!(snapshot(&a)["issues"], json!([]));
}

#[test]
fn a_clear_and_a_concurrent_set_converge() {
    let (mut a, mut b, _) = pair(&fixture("scalars"));
    apply(&mut a, json!([{"type":"set","path":["memo"],"value":"first"}]));
    b.import(&a.export_since(&b.version()).unwrap()).unwrap();
    let base = a.version();
    apply(&mut a, json!([{"type":"clear","path":["memo"]}]));
    apply(&mut b, json!([{"type":"set","path":["memo"],"value":"second"}]));
    exchange(&mut a, &mut b, &base);
    assert_eq!(snapshot(&a)["value"], snapshot(&b)["value"]);
    assert_eq!(snapshot(&a)["issues"], json!([]));
}

#[test]
fn a_merged_out_of_range_value_is_flagged_not_repaired_and_can_be_overwritten() {
    let f = fixture("scalars");
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    peer.get_map("data").insert("rating", 9i64).unwrap();
    peer.get_map("data").insert("currency", "GBP").unwrap();
    peer.commit();
    d.import(&peer.export(ExportMode::updates(&from)).unwrap()).unwrap();
    let before = snapshot(&d);
    assert_eq!(before["value"]["rating"], 9);
    assert_eq!(before["value"]["currency"], "GBP");
    assert_eq!(
        before["issues"],
        json!([{"code":"type_mismatch","path":["currency"]},{"code":"out_of_range","path":["rating"]}])
    );
    // A value of the right type may be written over; a wrong-typed one is preserved.
    apply(&mut d, json!([{"type":"set","path":["rating"],"value":4}]));
    let refused = d.apply(&json!({"intents":[{"type":"set","path":["currency"],"value":"CAD"}]}).to_string());
    assert_eq!(refused.unwrap_err().code.as_str(), "type_mismatch");
    assert_eq!(snapshot(&d)["issues"], json!([{"code":"type_mismatch","path":["currency"]}]));
}
