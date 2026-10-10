//! Merge closure. Replicas of one document edit concurrently from the same seed, each
//! session under its own Loro peer, with every intent kind, refused batches, undo and redo,
//! and exchange whole histories in random order. Failure: a merge of valid edits that the
//! open-time acceptance refuses (the document would never open again), or replicas that read
//! differently after holding the same edits. Inserted rows draw their IDs from a small
//! pool, so replicas also create, remove and restore the same row concurrently.
mod support;
use hitslop_core::{Document, Origin};
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
    for seed in 1..=workload("HITSLOP_MERGE_SEEDS", 40) as u64 {
        let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let origin = Document::create(&spec, &f["initial"].to_string()).unwrap();
        let start = origin.checkpoint().unwrap();
        let mut replicas: Vec<Document> = (0..3).map(|_| Document::open(&spec, &start, &[]).unwrap()).collect();
        let mut serials = [0usize; 3];
        for _ in 0..workload("HITSLOP_MERGE_STEPS", 120) {
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
                    replicas[i] = Document::open(&spec, &replicas[i].checkpoint().unwrap(), &[theirs])
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
        hitslop_core::validate(&f["schema"].to_string(), &snapshot(&forward)["value"].to_string())
            .unwrap_or_else(|e| panic!("{name} seed {seed}: the merged value breaks its schema: {e:?}"));
    }
}

#[test]
fn concurrent_edits_of_every_kind_merge_into_documents_that_open() {
    for name in ["checklist", "collections", "nested", "scalars"] {
        run(name);
    }
}
