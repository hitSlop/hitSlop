// Failure: nested lists and objects inside rows publish wrong paths or order, e.g. a
// nested edit and removal of its containing row in one import.
// Oracle: independent patch consumer + fresh snapshots. Gap: every other randomized
// test uses the flat checklist schema.
mod support;
use hitslop_core::Document;
use serde_json::{json, Value};
fn next(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}
fn random_op(rng: &mut u64, serial: &mut u64, d: &Document) -> Value {
    let view: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
    let rows = view["value"]["rows"].as_array().unwrap().clone();
    *serial += 1;
    let r = next(rng);
    if rows.is_empty() || r % 7 == 0 {
        return json!({"type":"insert","path":["rows"],"id":format!("row{serial}"),"value":{"text":"n","meta":{"pinned":false},"tags":[{"$id":format!("tag{serial}"),"label":"t","on":true}]}});
    }
    let row = &rows[(r as usize / 7) % rows.len()];
    let rid = row["$id"].clone();
    let tags = row["tags"].as_array().unwrap();
    match r % 9 {
        1 => json!({"type":"set","path":["rows",{"id":rid},"meta","pinned"],"value":!row["meta"]["pinned"].as_bool().unwrap()}),
        2 => json!({"type":"insert","path":["rows",{"id":rid},"tags"],"id":format!("tag{serial}"),"value":{"label":"l","on":false}}),
        3 if !tags.is_empty() => json!({"type":"set","path":["rows",{"id":rid},"tags",{"id":tags[0]["$id"]},"on"],"value":!tags[0]["on"].as_bool().unwrap()}),
        4 if tags.len() > 1 => json!({"type":"move","path":["rows",{"id":rid},"tags"],"id":tags[0]["$id"]}),
        5 if !tags.is_empty() => json!({"type":"remove","path":["rows",{"id":rid},"tags"],"id":tags[tags.len()-1]["$id"]}),
        6 if rows.len() > 1 => json!({"type":"remove","path":["rows"],"id":rid}),
        7 if rows.len() > 1 => json!({"type":"move","path":["rows"],"id":rid,"at":{"before":rows[0]["$id"]}}),
        8 if !tags.is_empty() => json!({"type":"set","path":["rows",{"id":rid},"tags",{"id":tags[0]["$id"]},"label"],"value":format!("q{r}")}),
        _ => json!({"type":"set","path":["rows",{"id":rid},"text"],"value":format!("s{r}")}),
    }
}
#[test]
fn seeded_nested_local_and_remote_steps() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/nested.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut rng = 0x7e57edu64;
    let mut serial = 0u64;
    for _round in 0..300 {
        let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
        let mut projected = f["initial"].clone();
        for step in 0..60 {
            let reply = if step % 4 == 0 {
                // Several remote edits in one import, possibly a nested edit then
                // removal of the row that contains it.
                let mut peer = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
                let base = d.version();
                for _ in 0..1 + next(&mut rng) % 4 {
                    let op = random_op(&mut rng, &mut serial, &peer);
                    peer.apply(&json!({"intents":[op]}).to_string()).unwrap();
                }
                d.import(&peer.export_since(&base).unwrap()).unwrap()
            } else {
                let op = random_op(&mut rng, &mut serial, &d);
                d.apply(&json!({"intents":[op]}).to_string()).unwrap()
            };
            let reply: Value = serde_json::from_str(&reply).unwrap();
            support::apply_patches(&mut projected, &reply["ops"]);
            let fresh: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
            assert_eq!(projected, fresh["value"]);
            assert_eq!(reply["issues"], fresh["issues"]);
            // Clean nested lists publish row operations, not a replacement of their row list.
            for op in reply["ops"].as_array().unwrap() {
                if op["type"] == "set" {
                    let last = op["path"].as_array().unwrap().last().cloned();
                    assert!(last != Some(json!("rows")) && last != Some(json!("tags")), "fallback in clean list: {op}");
                }
            }
        }
    }
}
#[test]
fn nested_edit_and_removal_of_its_row_in_one_import() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/nested.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let mut projected = f["initial"].clone();
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
    let reply: Value = serde_json::from_str(&d.import(&peer.export_since(&base).unwrap()).unwrap()).unwrap();
    support::apply_patches(&mut projected, &reply["ops"]);
    let fresh: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
    assert_eq!(projected, fresh["value"]);
    assert_eq!(reply["ops"], json!([{"type":"deleteRow","path":["rows"],"id":"row1"}]));
}
