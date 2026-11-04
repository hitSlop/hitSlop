//! Merge closure. Replicas of one document edit concurrently from the same seed, each
//! session under its own Loro peer, with every intent kind, refused batches, undo and redo,
//! and exchange whole histories in random order. Failure: a merge of valid edits that the
//! open-time acceptance refuses (the document would never open again), or replicas that read
//! differently after holding the same edits. Inserted rows draw their IDs from a small
//! pool, so replicas also create, remove and restore the same row concurrently.
use crate::support;
use pop_core::{Document, Origin};
use serde_json::{Value, json};
use support::ApplyJson;
use support::generate::intent;
use support::{app, fixture, next, snapshot, workload};

/// Every row list in `value` names each row once.
fn rows_are_unique(value: &Value) -> bool {
    match value {
        Value::Array(items) => {
            let ids: Vec<&str> = items.iter().filter_map(|row| row.get("$id")?.as_str()).collect();
            ids.len() == ids.iter().collect::<std::collections::HashSet<_>>().len() && items.iter().all(rows_are_unique)
        }
        Value::Object(fields) => fields.values().all(rows_are_unique),
        _ => true,
    }
}
fn run(name: &str) {
    let f = fixture(name);
    let spec = app(f["schema"].to_string());
    for seed in 1..=workload("POP_MERGE_SEEDS", 40) as u64 {
        let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let origin = Document::create(&spec, &f["initial"].to_string()).unwrap();
        let start = origin.checkpoint().unwrap();
        let mut replicas: Vec<Document> = (0..3).map(|_| Document::open(&spec, &start, &[]).unwrap()).collect();
        let mut serials = [0usize; 3];
        for _ in 0..workload("POP_MERGE_STEPS", 120) {
            let i = (next(&mut rng) % 3) as usize;
            match next(&mut rng) % 10 {
                0..=5 => {
                    let current = snapshot(&replicas[i])["value"].clone();
                    let ops: Vec<Value> = (0..1 + next(&mut rng) % 2)
                        .map(|_| {
                            let mut op = intent(&mut rng, &mut serials[i], &f["schema"], &current);
                            // Inserted rows draw from a small pool of IDs, so replicas create the
                            // same row concurrently.
                            if op["type"] == "insert" && op["id"].is_string() {
                                op["id"] = json!(format!("row{}", next(&mut rng) % 4));
                            }
                            op
                        })
                        .collect();
                    // Refusals are part of the workload; acceptance of the merge is the oracle.
                    let _ = replicas[i].apply_json(&json!({"intents":ops}).to_string(), Origin::Page);
                }
                6 | 7 => {
                    let _ = replicas[i].undo();
                }
                8 => {
                    let _ = replicas[i].redo();
                }
                _ => {
                    // A delivery: everything replica `j` holds, merged into replica `i`.
                    let j = (i + 1 + (next(&mut rng) % 2) as usize) % 3;
                    let theirs = replicas[j].checkpoint().unwrap();
                    replicas[i]
                        .import_remote(&[&theirs])
                        .unwrap_or_else(|e| panic!("{name} seed {seed}: a merge was refused: {e:?}"));
                    assert!(rows_are_unique(&snapshot(&replicas[i])["value"]), "{name} seed {seed}: a row reads twice");
                }
            }
        }
        let all: Vec<Vec<u8>> = replicas.iter().map(|d| d.checkpoint().unwrap()).collect();
        let forward = Document::open(&spec, &all[0], &all[1..])
            .unwrap_or_else(|e| panic!("{name} seed {seed}: the full merge was refused: {e:?}"));
        let backward = Document::open(&spec, &all[2], &[all[1].clone(), all[0].clone()]).unwrap();
        assert_eq!(snapshot(&forward)["value"], snapshot(&backward)["value"], "{name} seed {seed}: merges differ");
        assert!(rows_are_unique(&snapshot(&forward)["value"]), "{name} seed {seed}: a row reads twice");
        pop_core::validate(&f["schema"].to_string(), &snapshot(&forward)["value"].to_string())
            .unwrap_or_else(|e| panic!("{name} seed {seed}: the merged value breaks its schema: {e:?}"));
    }
}

#[test]
fn concurrent_edits_of_every_kind_merge_into_documents_that_open() {
    for name in ["checklist", "collections", "nested", "scalars"] {
        run(name);
    }
}

// Concurrent creations share their semantic container, including nested rows.
#[test]
fn concurrent_creation_merges_children_and_keeps_them_editable() {
    let schema = json!({"kind":"object","properties":{
        "title":{"kind":"text"},
        "note":{"kind":"optional","inner":{"kind":"text"}},
        "entries":{"kind":"record","value":{"kind":"object","properties":{
            "text":{"kind":"text"},"label":{"kind":"string"}
        }}},
        "rows":{"kind":"list","item":{"kind":"object","properties":{
            "text":{"kind":"text"},
            "children":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"}}}}
        }}}
    }});
    let spec = app(schema.to_string());
    let seed = Document::create(&spec, &json!({"title":"base","entries":{},"rows":[]}).to_string())
        .unwrap()
        .checkpoint()
        .unwrap();
    let mut a = Document::open(&spec, &seed, &[]).unwrap();
    let mut b = Document::open(&spec, &seed, &[]).unwrap();
    let mut av = support::View::of(&a);
    let mut bv = support::View::of(&b);
    for (doc, view, label, title) in [(&mut a, &mut av, "Left", "Lbase"), (&mut b, &mut bv, "Right", "baseR")] {
        let applied = doc
            .apply_json(
                &json!({"intents":[
                    {"type":"set","path":["title"],"value":title},
                    {"type":"set","path":["note"],"value":label},
                    {"type":"set","path":["entries","same"],"value":{"text":label,"label":label}},
                    {"type":"insert","path":["rows"],"id":"same","value":{
                        "text":label,"children":[{"$id":"child","text":label}]
                    }}
                ]})
                .to_string(),
                Origin::Page,
            )
            .unwrap();
        view.publish(&applied.publication.unwrap());
        view.check(doc, "local creation");
    }
    let left = a.checkpoint().unwrap();
    let right = b.checkpoint().unwrap();
    av.publish(&a.import_remote(&[&right]).unwrap().unwrap().publication.unwrap());
    bv.publish(&b.import_remote(&[&left]).unwrap().unwrap().publication.unwrap());
    av.check(&a, "left after creation race");
    bv.check(&b, "right after creation race");
    let merged = support::value(&a);
    assert_eq!(merged, support::value(&b));
    assert_eq!(merged["title"], "LbaseR");
    for child in [&merged["note"], &merged["entries"]["same"]["text"], &merged["rows"][0]["text"]] {
        assert!(child == "LeftRight" || child == "RightLeft", "both creations must survive: {child}");
    }
    assert!(merged["entries"]["same"]["label"] == "Left" || merged["entries"]["same"]["label"] == "Right");
    assert_eq!(merged["rows"].as_array().unwrap().len(), 1);
    assert_eq!(merged["rows"][0]["$id"], "same");
    for path in [json!(["note"]), json!(["entries", "same", "text"]), json!(["rows",{"id":"same"},"text"])] {
        let applied = a
            .apply_json(&json!({"intents":[{"type":"set","path":path,"value":"Edited"}]}).to_string(), Origin::Page)
            .unwrap();
        av.publish(&applied.publication.unwrap());
        av.check(&a, "editing a winning child");
    }
    let applied = a
        .apply_json(
            &json!({"intents":[
                {"type":"insert","path":["rows",{"id":"same"},"children"],"id":"second","value":{"text":"Second"}},
                {"type":"remove","path":["rows",{"id":"same"},"children"],"id":"child"}
            ]})
            .to_string(),
            Origin::Page,
        )
        .unwrap();
    av.publish(&applied.publication.unwrap());
    av.check(&a, "editing the winning row's nested list");
    bv.publish(&b.import_remote(&[&a.checkpoint().unwrap()]).unwrap().unwrap().publication.unwrap());
    bv.check(&b, "remote edits after replacement");
    assert_eq!(support::value(&a), support::value(&b));
    let reopened = Document::open(&spec, &b.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(support::value(&a), support::value(&reopened));
}

#[test]
fn concurrent_creation_does_not_write_canonical_defaults_over_other_fields() {
    let fields = json!({"kind":"object","properties":{
        "left":{"kind":"text"}, "right":{"kind":"text"},
        "enabled":{"kind":"boolean","default":false}
    }});
    let schema = json!({"kind":"object","properties":{
        "extra":{"kind":"optional","inner":fields},
        "entries":{"kind":"record","value":fields},
        "rows":{"kind":"list","item":fields}
    }});
    let spec = app(schema.to_string());
    let seed = Document::create(&spec, "{}").unwrap().checkpoint().unwrap();
    let mut replicas = [Document::open(&spec, &seed, &[]).unwrap(), Document::open(&spec, &seed, &[]).unwrap()];
    for (doc, value) in replicas.iter_mut().zip([json!({"left":"L", "enabled":true}), json!({"right":"R"})]) {
        doc.apply_json(
            &json!({"intents":[
                {"type":"set","path":["extra"],"value":value},
                {"type":"set","path":["entries","same"],"value":value},
                {"type":"insert","path":["rows"],"id":"same","value":value}
            ]})
            .to_string(),
            Origin::Page,
        )
        .unwrap();
    }
    let checkpoints: Vec<_> = replicas.iter().map(|doc| doc.checkpoint().unwrap()).collect();
    for (first, second) in [(0, 1), (1, 0)] {
        let mut merged = Document::open(&spec, &checkpoints[first], &[]).unwrap();
        let mut view = support::View::of(&merged);
        let result = merged.import_remote(&[&checkpoints[second]]).unwrap().unwrap();
        view.publish(&result.publication.unwrap());
        view.check(&merged, "concurrent creation with defaults");
        let state = support::value(&merged);
        let expected = json!({"left":"L", "right":"R", "enabled":true});
        assert_eq!(state["extra"], expected);
        assert_eq!(state["entries"]["same"], expected);
        assert_eq!(state["rows"], json!([{"$id":"same","left":"L","right":"R","enabled":true}]));
        let reopened = Document::open(&spec, &merged.checkpoint().unwrap(), &[]).unwrap();
        assert_eq!(support::value(&reopened), state);
    }
}

#[test]
fn cleared_generations_hide_stale_writes_and_recreation_starts_fresh() {
    let fields = json!({"kind":"object","properties":{"text":{"kind":"text"}}});
    let schema = json!({"kind":"object","properties":{
        "note":{"kind":"optional","inner":{"kind":"text"}},
        "extra":{"kind":"optional","inner":fields},
        "entries":{"kind":"record","value":fields},
        "rows":{"kind":"list","item":fields}
    }});
    let spec = app(schema.to_string());
    let initial = json!({"note":"old", "extra":{"text":"old"},
        "entries":{"same":{"text":"old"}}, "rows":[{"$id":"same","text":"old"}]});
    let seed = Document::create(&spec, &initial.to_string()).unwrap().checkpoint().unwrap();
    let mut cleared = Document::open(&spec, &seed, &[]).unwrap();
    let mut stale = Document::open(&spec, &seed, &[]).unwrap();
    cleared
        .apply_json(
            &json!({"intents":[
                {"type":"clear","path":["note"]}, {"type":"clear","path":["extra"]},
                {"type":"clear","path":["entries","same"]}, {"type":"remove","path":["rows"],"id":"same"}
            ]})
            .to_string(),
            Origin::Page,
        )
        .unwrap();
    for path in [
        json!(["note"]),
        json!(["extra", "text"]),
        json!(["entries", "same", "text"]),
        json!(["rows",{"id":"same"},"text"]),
    ] {
        stale
            .apply_json(&json!({"intents":[{"type":"set","path":path,"value":"stale"}]}).to_string(), Origin::Page)
            .unwrap();
    }
    let deleted = cleared.checkpoint().unwrap();
    let edited = stale.checkpoint().unwrap();
    let empty = json!({"entries":{},"rows":[]});
    for (first, second) in [(&deleted, &edited), (&edited, &deleted)] {
        let mut merged = Document::open(&spec, first, &[]).unwrap();
        let mut view = support::View::of(&merged);
        let applied = merged.import_remote(&[second]).unwrap().unwrap();
        view.publish(&applied.publication.unwrap());
        view.check(&merged, "clear against stale writes");
        assert_eq!(support::value(&merged), empty);
        let created = merged
            .apply_json(
                &json!({"intents":[
                    {"type":"set","path":["note"],"value":"fresh"},
                    {"type":"set","path":["extra"],"value":{"text":"fresh"}},
                    {"type":"set","path":["entries","same"],"value":{"text":"fresh"}},
                    {"type":"insert","path":["rows"],"id":"same","value":{"text":"fresh"}}
                ]})
                .to_string(),
                Origin::Page,
            )
            .unwrap();
        view.publish(&created.publication.unwrap());
        view.check(&merged, "fresh generation after clear");
        let expected = json!({"note":"fresh","extra":{"text":"fresh"},
            "entries":{"same":{"text":"fresh"}},"rows":[{"$id":"same","text":"fresh"}]});
        assert_eq!(support::value(&merged), expected);
        assert_eq!(support::value(&Document::open(&spec, &merged.checkpoint().unwrap(), &[]).unwrap()), expected);
    }
    let mut view = support::View::of(&cleared);
    view.publish(&cleared.undo().unwrap().publication.unwrap());
    view.check(&cleared, "undo restores selected generation");
    assert_eq!(support::value(&cleared), initial);
    view.publish(&cleared.redo().unwrap().publication.unwrap());
    view.check(&cleared, "redo hides old generation");
    assert_eq!(support::value(&cleared), empty);
    view.publish(&cleared.import_remote(&[&edited]).unwrap().unwrap().publication.unwrap());
    view.publish(&cleared.undo().unwrap().publication.unwrap());
    view.check(&cleared, "undo restores retained remote edits");
    assert_eq!(
        support::value(&cleared),
        json!({"note":"stale","extra":{"text":"stale"},
        "entries":{"same":{"text":"stale"}},"rows":[{"$id":"same","text":"stale"}]})
    );
}

#[test]
fn rebuilt_history_refuses_stale_epochs_and_accepts_descendants_out_of_order() {
    use pop_core::{Code, HistoryVersion};
    let spec = app(r#"{"kind":"object","properties":{"title":{"kind":"text"}}}"#);
    let mut old = Document::create(&spec, r#"{"title":"base"}"#).unwrap();
    old.apply_json(r#"{"intents":[{"type":"set","path":["title"],"value":"old"}]}"#, Origin::Page).unwrap();
    let fresh = old.clean_checkpoint().unwrap();
    let rebuilt = Document::open(&spec, &fresh, &[]).unwrap();
    assert_eq!(
        old.history_updates_since(&rebuilt.history_version()).unwrap_err().code,
        Code::MissingDependencies,
        "an old live owner cannot append deltas to a rebuilt durable history"
    );
    for stale in [old.checkpoint().unwrap(), old.history_updates_since(&HistoryVersion::default()).unwrap()] {
        let mut receiver = Document::open(&spec, &fresh, &[]).unwrap();
        let before = receiver.state().unwrap();
        let error = receiver.import_remote(&[&stale]).expect_err("old history must not merge into rebuilt state");
        assert_eq!(error.code, Code::MissingDependencies);
        assert_eq!(receiver.state().unwrap(), before, "epoch refusal precedes publication");
        assert!(Document::open(&spec, &fresh, &[stale]).is_err(), "saved updates obey the same epoch boundary");
    }
    let mut writer = Document::open(&spec, &fresh, &[]).unwrap();
    let start = writer.history_version();
    writer.apply_json(r#"{"intents":[{"type":"set","path":["title"],"value":"first"}]}"#, Origin::Page).unwrap();
    let first = writer.history_updates_since(&start).unwrap();
    let middle = writer.history_version();
    writer.apply_json(r#"{"intents":[{"type":"set","path":["title"],"value":"second"}]}"#, Origin::Page).unwrap();
    let second = writer.history_updates_since(&middle).unwrap();
    for updates in [[&first, &second], [&second, &first]] {
        let mut receiver = Document::open(&spec, &fresh, &[]).unwrap();
        let mut view = support::View::of(&receiver);
        for update in updates {
            if let Some(applied) = receiver.import_remote(&[update]).unwrap() {
                view.publish(&applied.publication.unwrap());
                view.check(&receiver, "same epoch out-of-order delivery");
            }
        }
        assert_eq!(support::value(&receiver), json!({"title":"second"}));
        assert_eq!(
            support::value(&Document::open(&spec, &receiver.checkpoint().unwrap(), &[]).unwrap()),
            support::value(&receiver)
        );
    }
}
