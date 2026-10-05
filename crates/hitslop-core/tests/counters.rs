// Failure: counter values differ between live use, replay or reopen, or leave the safe
// integer range. Oracles: literal values, the patch consumer and fresh snapshots.
mod support;
use hitslop_core::Document;
use support::{app, Edit, snapshot, updates_since};
use serde_json::{json, Value};

const MAX_SAFE: i64 = 9_007_199_254_740_991;
fn schema() -> String {
    json!({"kind":"object","properties":{
        "hits":{"kind":"counter"},
        "rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"},"votes":{"kind":"counter"}}}}
    }})
    .to_string()
}
fn initial() -> String {
    json!({"hits":5,"rows":[{"$id":"r1","text":"a","votes":0}]}).to_string()
}
fn increment(path: Value, by: i64) -> String {
    json!({"intents":[{"type":"increment","path":path,"by":by}]}).to_string()
}

#[test]
fn counters_stay_exact_past_float_precision() {
    // Exact integers, bounded to the safe range, where a float sum would drift.
    let mut d = Document::create(&app(schema()), &json!({"hits":0,"rows":[]}).to_string()).unwrap();
    let seed = d.checkpoint().unwrap();
    d.apply(&increment(json!(["hits"]), 9_000_000_000_000_000)).unwrap();
    let e = d.apply(&increment(json!(["hits"]), 9_000_000_000_000_000)).unwrap_err();
    assert_eq!(e.code.as_str(), "out_of_range");
    assert_eq!(snapshot(&d)["value"]["hits"], 9_000_000_000_000_000i64);
    for by in [1, -9_000_000_000_000_000, MAX_SAFE - 1, -MAX_SAFE + 1] {
        d.apply(&increment(json!(["hits"]), by)).unwrap();
    }
    assert_eq!(snapshot(&d)["value"]["hits"], 1);
    let replayed = Document::open(&app(schema()), &seed, &[updates_since(&seed, &d)]).unwrap();
    assert_eq!(snapshot(&replayed)["value"]["hits"], 1);
    let reopened = Document::open(&app(schema()), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"]["hits"], 1);
}




#[test]
fn counters_reject_set_zero_and_non_counter_targets() {
    let mut d = Document::create(&app(schema()), &initial()).unwrap();
    let before = snapshot(&d);
    for (batch, code) in [
        (increment(json!(["hits"]), 0), "out_of_range"),
        (increment(json!(["rows", {"id":"r1"}, "text"]), 1), "type_mismatch"),
        (json!({"intents":[{"type":"set","path":["hits"],"value":3}]}).to_string(), "type_mismatch"),
    ] {
        assert_eq!(d.apply(&batch).unwrap_err().code.as_str(), code);
        assert_eq!(snapshot(&d), before);
    }
}

