//! Core costs that scale with document size: snapshot, reopen with a full update log,
//! a multi-insert batch, the largest move and insert batches, edits on a document that carries an issue, and the publication
//! size of one keystroke in a long text. Run with
//! `cargo run --release -p hitslop-core --example core_cost`.
use hitslop_core::Document;
use serde_json::{json, Value};
use std::time::Instant;
use hitslop_core::Origin;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}
fn time(samples: usize, mut f: impl FnMut()) -> f64 {
    f();
    median((0..samples).map(|_| {
        let started = Instant::now();
        f();
        started.elapsed().as_secs_f64() * 1e3
    }).collect())
}
fn checklist(rows: usize) -> (String, Document) {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let schema = fixture["schema"].to_string();
    let rows: Vec<Value> = (0..rows).map(|i| json!({"$id":format!("r{i}"),"text":format!("Task number {i}"),"done":false})).collect();
    let doc = Document::create(&schema, &json!({"title":"t","rows":rows,"hits":0}).to_string()).unwrap();
    (schema, doc)
}
fn batch(intents: Value) -> String {
    json!({ "intents": intents }).to_string()
}

/// Reorder exactly 1,000 rows using 999 moves. Each direction gets one warm-up and
/// five measured samples; only apply_batch is timed, not JSON construction or checks.
fn reorder_cost(rows: usize) -> Value {
    let (_, mut doc) = checklist(rows);
    let requests = ["before", "after"].map(|anchor| batch(json!((1..1000)
        .map(|i| json!({"type":"move","path":["rows"],"id":format!("r{i}"),"at":{(anchor):format!("r{}", i - 1)}}))
        .collect::<Vec<_>>())));
    let mut times = [vec![], vec![]];
    for round in 0..6 {
        for direction in 0..2 {
            let previous = doc.sequence();
            let started = Instant::now();
            let applied = doc.apply_batch(&requests[direction], Origin::Page).unwrap();
            let elapsed = started.elapsed().as_secs_f64() * 1e3;
            assert_eq!(applied.sequence, previous + 1, "each sample must change order");
            assert!(applied.publication.is_some());
            let value: Value = serde_json::from_str(&doc.value().unwrap()).unwrap();
            let actual: Vec<_> = value["rows"].as_array().unwrap().iter()
                .map(|row| row["$id"].as_str().unwrap()).collect();
            let expected: Vec<_> = (0..rows)
                .map(|i| format!("r{}", if direction == 0 && i < 1000 { 999 - i } else { i }))
                .collect();
            assert_eq!(actual, expected, "complete order, including the unchanged suffix");
            if round > 0 {
                times[direction].push(elapsed);
            }
        }
    }
    let summary = |mut samples: Vec<f64>| {
        samples.sort_by(f64::total_cmp);
        json!({"median":samples[samples.len() / 2],"max":samples[samples.len() - 1]})
    };
    let [reverse, restore] = times;
    json!({"reverse":summary(reverse),"restore":summary(restore)})
}

fn main() {
    let mut out = serde_json::Map::new();
    for rows in [1_000, 5_000] {
        let (schema, mut doc) = checklist(rows);
        out.insert(format!("snapshotMS{rows}"), json!(time(10, || { doc.snapshot().unwrap(); })));
        // A full update log: 256 single edits saved since the checkpoint.
        let checkpoint = doc.checkpoint().unwrap();
        let mut updates = vec![];
        for i in 0..256 {
            let before = doc.version();
            doc.apply_batch(&batch(json!([{"type":"set","path":["rows",{"id":format!("r{}", i * 7 % rows)},"done"],"value":i % 2 == 0}])), Origin::Page).unwrap();
            updates.push(doc.export_since(&before).unwrap());
        }
        out.insert(format!("openWith256UpdatesMS{rows}"), json!(time(5, || { Document::open(&schema, &checkpoint, &updates).unwrap(); })));
        // Ten inserts into one list in one batch, each anchored after the previous.
        let mut n = 0;
        out.insert(format!("tenInsertBatchMS{rows}"), json!(time(10, || {
            let mut intents = vec![json!({"type":"insert","path":["rows"],"id":format!("n{n}-0"),"value":{"text":"new","done":false},"at":{"after":"r0"}})];
            for k in 1..10 {
                intents.push(json!({"type":"insert","path":["rows"],"id":format!("n{n}-{k}"),"value":{"text":"new","done":false},"at":{"after":format!("n{n}-{}", k - 1)}}));
            }
            n += 1;
            doc.apply_batch(&batch(json!(intents)), Origin::Page).unwrap();
        })));
        // Reorders use an isolated document, unaffected by the insert benchmarks.
        out.insert(format!("thousandRowReorderMS{rows}"), reorder_cost(rows));
        let mut round = 0;
        out.insert(format!("thousandInsertBatchMS{rows}"), json!(time(3, || {
            round += 1;
            let intents: Vec<Value> = (0..1000)
                .map(|i| json!({"type":"insert","path":["rows"],"id":format!("a{round}-{i}"),"value":{"text":"new","done":false}}))
                .collect();
            doc.apply_batch(&batch(json!(intents)), Origin::Page).unwrap();
        })));
        // One merged anomaly (two peers insert the same `$id`), then ordinary edits.
        let base = doc.version();
        let saved = doc.checkpoint().unwrap();
        for peer in 0..2 {
            let mut replica = Document::open(&schema, &saved, &[]).unwrap();
            replica.apply_batch(&batch(json!([{"type":"insert","path":["rows"],"id":"dup","value":{"text":format!("peer {peer}"),"done":false}}])), Origin::Page).unwrap();
            doc.import(&replica.export_since(&base).unwrap()).unwrap();
        }
        let issues: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        assert!(!issues["issues"].as_array().unwrap().is_empty(), "fixture must carry an issue");
        let mut flag = false;
        out.insert(format!("editWithIssueMS{rows}"), json!(time(20, || {
            flag = !flag;
            doc.apply_batch(&batch(json!([{"type":"set","path":["rows",{"id":"r1"},"done"],"value":flag}])), Origin::Page).unwrap();
        })));
    }
    // Publication bytes for one keystroke in a long text field.
    for length in [10_000, 100_000] {
        let (_, mut doc) = checklist(10);
        let from = "x".repeat(length);
        doc.apply_batch(&batch(json!([{"type":"set","path":["title"],"value":from}])), Origin::Page).unwrap();
        let to = format!("{from}y");
        let caret = to.len();
        let request = json!({"base":doc.version(),"path":["title"],"from":from,"to":to,"selectionStart":caret,"selectionEnd":caret}).to_string();
        let started = Instant::now();
        let edit = doc.edit_text(&request).unwrap();
        out.insert(format!("keystrokeMS{length}"), json!(started.elapsed().as_secs_f64() * 1e3));
        out.insert(format!("keystrokePublicationBytes{length}"), json!(edit.publication.unwrap().len()));
    }
    println!("{}", serde_json::to_string_pretty(&Value::Object(out)).unwrap());
}
