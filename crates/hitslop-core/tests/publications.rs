// Failure: row indexes or patches drift after structural changes/imports, including
// several list changes combined in one import. Oracle: independent patch consumer + fresh
// snapshots, with literal outcomes covered by conformance.rs.
mod support;
use hitslop_core::Document;
use support::{Edit, View};
use serde_json::{json, Value};
use hitslop_core::Origin;
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
        _ => json!({"type":"set","path":["title"],"value":format!("x{r}")}),
    }
}
#[test]
fn seeded_local_and_remote_steps() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut rng = 0x5eeda11u64;
    let mut id = 100u64;
    for _round in 0..support::workload("HITSLOP_PUBLICATIONS_ROUNDS", 100) {
        let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
        let mut projected = View::of(&d);
        for step in 0..support::workload("HITSLOP_PUBLICATIONS_STEPS", 100) {
            let reply = if step % 7 == 0 {
                // A remote peer makes one to four edits; one import publishes them together.
                let mut peer = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
                let base = d.version();
                for _ in 0..1 + next(&mut rng) % 4 {
                    let op = random_op(&mut rng, &mut id, &peer);
                    peer.apply(&json!({"intents":[op]}).to_string()).unwrap();
                }
                d.merge(&peer.export_since(&base).unwrap()).unwrap()
            } else {
                let op = random_op(&mut rng, &mut id, &d);
                d.apply(&json!({"intents":[op]}).to_string()).unwrap()
            };
            projected.publish(&reply);
            projected.check(&d, "publication");
            let reply: Value = serde_json::from_str(&reply).unwrap();
            // Clean lists publish row operations, never a whole-list replacement.
            assert!(!reply["ops"].as_array().unwrap().iter().any(|op| op["path"] == json!(["rows"]) && op["type"] == "set"));
        }
    }
}
#[test]
fn merged_duplicate_ids_publish_exactly_and_stay_flagged() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut a = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let mut b = Document::open(&schema, &a.checkpoint().unwrap(), &[]).unwrap();
    let base = a.version();
    let insert = json!({"intents":[{"type":"insert","path":["rows"],"id":"same","value":{"text":"x","done":false}}]}).to_string();
    a.apply(&insert).unwrap();
    b.apply(&insert).unwrap();
    let mut projected = View::of(&a);
    let remote = b.export_since(&base).unwrap();
    for step in 0..3 {
        let reply = match step {
            0 => a.merge(&remote).unwrap(),
            1 => a.apply(r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000001"},"done"],"value":true}]}"#).unwrap(),
            _ => a.apply(r#"{"intents":[{"type":"move","path":["rows"],"id":"00000000000000000000000000000002","at":{"before":"00000000000000000000000000000001"}}]}"#).unwrap(),
        };
        projected.publish(&reply);
        projected.check(&a, "publication");
        let fresh: Value = serde_json::from_str(&a.snapshot().unwrap()).unwrap();
        assert!(fresh["issues"].as_array().unwrap().iter().any(|i| i["code"] == "duplicate_id"));
        let ids: std::collections::HashSet<_> = fresh["value"]["rows"].as_array().unwrap().iter().map(|row| row["$id"].as_str().unwrap()).collect();
        assert_eq!(ids.len(), 4, "Every merged row must remain uniquely addressable");
    }
    let before: Value = serde_json::from_str(&a.snapshot().unwrap()).unwrap();
    let derived = before["value"]["rows"].as_array().unwrap().iter().find(|row| row["$id"].as_str().unwrap().starts_with("x-")).unwrap()["$id"].as_str().unwrap().to_owned();
    a.apply(&json!({"intents":[{"type":"set","path":["rows",{"id":derived},"done"],"value":true},{"type":"move","path":["rows"],"id":derived,"at":{"before":"same"}}]}).to_string()).unwrap();
    let checkpoint = a.checkpoint().unwrap();
    let reopened = Document::open(&schema, &checkpoint, &[]).unwrap();
    let after: Value = serde_json::from_str(&reopened.snapshot().unwrap()).unwrap();
    let row = after["value"]["rows"].as_array().unwrap().iter().find(|row| row["$id"] == derived).unwrap();
    assert_eq!(row["done"], true);
    let raw = loro::LoroDoc::new();
    raw.import(&checkpoint).unwrap();
    let raw: Value = serde_json::to_value(raw.get_map("data").get_deep_value()).unwrap();
    assert_eq!(raw["rows"].as_array().unwrap().iter().filter(|row| row["$id"] == "same").count(), 2, "Reads and addressed writes preserve stored duplicate registers");

}

// Failure: a batch that changes nothing advances the sequence and marks the document
// dirty. Oracle: no publication and an unchanged sequence and version.
#[test]
fn a_batch_that_changes_nothing_publishes_nothing() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let (sequence, version) = (d.sequence(), d.version());
    let applied = d.apply_batch(r#"{"intents":[]}"#, Origin::Page).unwrap();
    assert!(applied.publication.is_none());
    assert_eq!((applied.sequence, d.sequence(), d.version()), (sequence, sequence, version));
}

// Failure: issues are recomputed and resent for the whole document on every edit once
// any issue exists. Oracle: an edit away from the anomaly publishes no issues; one that
// creates an anomaly carries the complete list, addressed by row ID.
#[test]
fn issues_are_republished_only_when_they_change() {
    let f: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let schema = f["schema"].to_string();
    let mut a = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let mut b = Document::open(&schema, &a.checkpoint().unwrap(), &[]).unwrap();
    let base = a.version();
    let insert = json!({"intents":[{"type":"insert","path":["rows"],"id":"same","value":{"text":"x","done":false}}]}).to_string();
    a.apply(&insert).unwrap();
    b.apply(&insert).unwrap();
    let merged: Value = serde_json::from_str(&a.merge(&b.export_since(&base).unwrap()).unwrap()).unwrap();
    let issues = merged["issues"].as_array().expect("a new anomaly publishes the issues");
    assert!(issues.iter().any(|issue| issue["code"] == "duplicate_id" && issue["path"][1]["id"].is_string()), "{issues:?}");
    let edit: Value = serde_json::from_str(&a.apply(r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000001"},"done"],"value":true}]}"#).unwrap()).unwrap();
    assert!(edit.get("issues").is_none(), "{edit}");
}
