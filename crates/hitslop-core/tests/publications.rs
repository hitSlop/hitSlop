// Failure: row indexes or patches drift after structural changes, including several list
// changes combined in one batch. Oracle: independent patch consumer + fresh snapshots,
// with literal outcomes covered by conformance.rs.
mod support;
use hitslop_core::Document;
use support::{app, Edit, View, fixture, next};
use serde_json::{json, Value};
use hitslop_core::Origin;
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
fn seeded_single_and_combined_steps() {
    let f: Value = fixture("checklist");
    let schema = f["schema"].to_string();
    let mut rng = 0x5eeda11u64;
    let mut id = 100u64;
    for _round in 0..support::workload("HITSLOP_PUBLICATIONS_ROUNDS", 100) {
        let mut d = Document::create(&app(&schema), &f["initial"].to_string()).unwrap();
        let mut projected = View::of(&d);
        for step in 0..support::workload("HITSLOP_PUBLICATIONS_STEPS", 100) {
            let reply = if step % 7 == 0 {
                // One to four edits in one batch publish together; each is chosen against
                // the state the previous one left, on a scratch copy.
                let mut scratch = Document::open(&app(&schema), &d.checkpoint().unwrap(), &[]).unwrap();
                let mut ops = vec![];
                for _ in 0..1 + next(&mut rng) % 4 {
                    let op = random_op(&mut rng, &mut id, &scratch);
                    scratch.apply(&json!({"intents":[op.clone()]}).to_string()).unwrap();
                    ops.push(op);
                }
                d.apply(&json!({"intents":ops}).to_string()).unwrap()
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

// Failure: Loro composes a row inserted and then moved in one commit into a single move of
// a row the list never held, so the whole list was republished. Oracle: literal ops.
#[test]
fn a_row_inserted_and_moved_in_one_batch_publishes_as_an_insertion() {
    let f: Value = fixture("checklist");
    let mut d = Document::create(&app(f["schema"].to_string()), &f["initial"].to_string()).unwrap();
    let mut projected = View::of(&d);
    let first = projected.value["rows"][0]["$id"].clone();
    let reply = d.apply(&json!({"intents":[
        {"type":"insert","path":["rows"],"id":"new","value":{"text":"new","done":false}},
        {"type":"move","path":["rows"],"id":"new","at":{"before":first}},
    ]}).to_string()).unwrap();
    projected.publish(&reply);
    projected.check(&d, "insert then move");
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["ops"], json!([{"type":"insertRow","path":["rows"],"index":0,"value":{"$id":"new","text":"new","done":false}}]));
}

// Failure: a batch that changes nothing advances the sequence and marks the document
// dirty. Oracle: no publication and an unchanged sequence and version.
#[test]
fn a_batch_that_changes_nothing_publishes_nothing() {
    let f: Value = fixture("checklist");
    let mut d = Document::create(&app(f["schema"].to_string()), &f["initial"].to_string()).unwrap();
    let (sequence, version) = (d.sequence(), d.version());
    let applied = d.apply_batch(r#"{"intents":[]}"#, Origin::Page).unwrap();
    assert!(applied.publication.is_none());
    assert_eq!((applied.sequence, d.sequence(), d.version()), (sequence, sequence, version));
}

