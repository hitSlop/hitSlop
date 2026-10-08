// Failure: row indexes or patches drift after structural changes/imports.
// Oracle: independent patch consumer + fresh snapshots, with literal outcomes
// covered by conformance.rs. Gap: no randomized native publication sequences;
// event-driven publication must also combine several list changes per import.
mod support;
use hitslop_core_spike::Document;
use serde_json::{json, Value};
fn next(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}
fn random_op(rng: &mut u64, id: &mut u64, d: &Document) -> Value {
    let view: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
    let rows = view["value"]["rows"].as_array().unwrap();
    let r = next(rng);
    let index = (r as usize) % rows.len().max(1);
    let target = (r as usize / 7) % rows.len().max(1);
    let anchor = match (r / 3) % 3 {
        0 => json!({"before":rows.get(target).map(|v| v["$id"].clone())}),
        1 => json!({"after":rows.get(target).map(|v| v["$id"].clone())}),
        _ => Value::Null,
    };
    match r % 5 {
        0 if !rows.is_empty() => {
            json!({"type":"set","path":["rows",{"id":rows[index]["$id"]},"done"],"value":!rows[index]["done"].as_bool().unwrap()})
        }
        1 if rows.len() < 20 => {
            *id += 1;
            let mut op = json!({"type":"insert","path":["rows"],"id":format!("{id:032x}"),"value":{"text":"new","done":false}});
            if !rows.is_empty() && !anchor.is_null() {
                op["at"] = anchor;
            }
            op
        }
        2 if rows.len() > 1 && rows[target]["$id"] != rows[index]["$id"] && !anchor.is_null() => {
            json!({"type":"move","path":["rows"],"id":rows[index]["$id"],"at":anchor})
        }
        2 if !rows.is_empty() => json!({"type":"move","path":["rows"],"id":rows[index]["$id"]}),
        3 if rows.len() > 1 => json!({"type":"remove","path":["rows"],"id":rows[index]["$id"]}),
        _ => json!({"type":"splice","path":["title"],"base":d.version(),"index":0,"delete":0,"insert":"x"}),
    }
}
#[test]
fn seeded_100000_local_and_remote_steps() {
    let f: Value = serde_json::from_str(include_str!("../../fixtures/checklist.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut rng = 0x5eeda11u64;
    let mut id = 100u64;
    for _round in 0..1000 {
        let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
        let mut projected = f["initial"].clone();
        for step in 0..100 {
            let reply = if step % 7 == 0 {
                // A remote peer makes one to four edits; one import publishes them together.
                let mut peer = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
                let base = d.version();
                for _ in 0..1 + next(&mut rng) % 4 {
                    let op = random_op(&mut rng, &mut id, &peer);
                    peer.apply(&json!({"intents":[op]}).to_string()).unwrap();
                }
                d.import(&peer.export_since(&base).unwrap()).unwrap()
            } else {
                let op = random_op(&mut rng, &mut id, &d);
                d.apply(&json!({"intents":[op]}).to_string()).unwrap()
            };
            let reply: Value = serde_json::from_str(&reply).unwrap();
            support::apply_patches(&mut projected, &reply["patch"]["ops"]);
            let fresh: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
            assert_eq!(projected, fresh["value"]);
            assert_eq!(reply["patch"]["issues"], fresh["issues"]);
            // Clean lists publish row operations, never a whole-list replacement.
            assert!(!reply["patch"]["ops"].as_array().unwrap().iter().any(|op| op["path"] == json!(["rows"]) && op["type"] == "set"));
        }
    }
}
#[test]
fn merged_duplicate_ids_publish_exactly_and_stay_flagged() {
    let f: Value = serde_json::from_str(include_str!("../../fixtures/checklist.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut a = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let mut b = Document::open(&schema, &a.checkpoint().unwrap(), &[]).unwrap();
    let base = a.version();
    let insert = json!({"intents":[{"type":"insert","path":["rows"],"id":"same","value":{"text":"x","done":false}}]}).to_string();
    a.apply(&insert).unwrap();
    b.apply(&insert).unwrap();
    let mut projected: Value = serde_json::from_str::<Value>(&a.snapshot().unwrap()).unwrap()["value"].clone();
    let remote = b.export_since(&base).unwrap();
    for step in 0..3 {
        let reply = match step {
            0 => a.import(&remote).unwrap(),
            1 => a.apply(r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000001"},"done"],"value":true}]}"#).unwrap(),
            _ => a.apply(r#"{"intents":[{"type":"move","path":["rows"],"id":"00000000000000000000000000000002","at":{"before":"00000000000000000000000000000001"}}]}"#).unwrap(),
        };
        let reply: Value = serde_json::from_str(&reply).unwrap();
        support::apply_patches(&mut projected, &reply["patch"]["ops"]);
        let fresh: Value = serde_json::from_str(&a.snapshot().unwrap()).unwrap();
        assert_eq!(projected, fresh["value"]);
        assert_eq!(reply["patch"]["issues"], fresh["issues"]);
        assert!(fresh["issues"].as_array().unwrap().iter().any(|i| i["code"] == "duplicate_id"));
    }
}
