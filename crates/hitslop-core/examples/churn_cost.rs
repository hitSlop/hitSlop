//! What deleted content costs a trimmed document: rows holding text, a nested list and an
//! optional object are inserted and removed, and record entries holding text are set
//! under unique keys and cleared. Reports the full and history-trimmed checkpoint sizes
//! against an untouched document with the same final value, and the containers a trimmed
//! reopen still retains. Run with
//! `cargo run --release -p hitslop-core --example churn_cost`.
use hitslop_core::{Document, Origin};
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};

const ROUNDS: usize = 50;
const PER_ROUND: usize = 20;

fn apply(doc: &mut Document, intents: Vec<Value>) -> Vec<String> {
    doc.apply_batch(&json!({ "intents": intents }).to_string(), Origin::Page).unwrap().ids
}
fn body(round: usize, i: usize) -> String {
    format!("Entry {round}.{i}: ").repeat(60)
}

fn report(name: &str, schema: &str, initial: &str, doc: &Document) -> Value {
    let full = doc.checkpoint().unwrap();
    let replica = LoroDoc::new();
    replica.import(&full).unwrap();
    let trimmed = replica.export(ExportMode::shallow_snapshot(&replica.oplog_frontiers())).unwrap();
    let reopened = Document::open(schema, &trimmed, &[]).unwrap();
    assert_eq!(reopened.value().unwrap(), doc.value().unwrap(), "{name}: a trimmed reopen keeps the value");
    let untouched = Document::create(schema, initial).unwrap().checkpoint().unwrap();
    let inspected = LoroDoc::new();
    inspected.import(&trimmed).unwrap();
    let containers = inspected.analyze().containers;
    json!({
        "workload": name,
        "fullBytes": full.len(),
        "trimmedBytes": trimmed.len(),
        "untouchedBytes": untouched.len(),
        "trimmedContainers": containers.len(),
        "trimmedDropped": containers.values().filter(|c| c.dropped).count(),
        "trimmedMergeable": containers.keys().filter(|id| id.is_mergeable()).count(),
        "everCreated": replica.analyze().containers.len(),
    })
}

/// Rows with text, a nested list and an optional object holding text, all removed again.
fn row_churn() -> Value {
    let schema = json!({"kind":"object","properties":{"rows":{"kind":"list","item":{"kind":"object","properties":{
        "body": {"kind":"text"},
        "tags": {"kind":"list","item":{"kind":"string"}},
        "note": {"kind":"optional","inner":{"kind":"object","properties":{"text":{"kind":"text"}}}},
    }}}}}).to_string();
    let initial = r#"{"rows":[]}"#;
    let mut doc = Document::create(&schema, initial).unwrap();
    for round in 0..ROUNDS {
        let ids = apply(&mut doc, (0..PER_ROUND).map(|i| json!({"type":"insert","path":["rows"],"value":{
            "body": body(round, i),
            "tags": (0..10).map(|t| format!("tag {t}")).collect::<Vec<_>>(),
            "note": {"text": body(round, i)},
        }})).collect());
        apply(&mut doc, ids.iter().map(|id| json!({"type":"remove","path":["rows"],"id":id})).collect());
    }
    report("rowChurn", &schema, initial, &doc)
}

/// Record entries holding text under keys never reused, each cleared again.
fn record_churn() -> Value {
    let schema = json!({"kind":"object","properties":{"entries":{"kind":"record","value":{"kind":"object","properties":{
        "body": {"kind":"text"},
        "n": {"kind":"integer"},
    }}}}}).to_string();
    let initial = r#"{"entries":{}}"#;
    let mut doc = Document::create(&schema, initial).unwrap();
    for round in 0..ROUNDS {
        let keys: Vec<String> = (0..PER_ROUND).map(|i| format!("{round}-{i}")).collect();
        apply(&mut doc, keys.iter().enumerate().map(|(i, key)| json!({"type":"set","path":["entries",key],"value":{"body":body(round, i),"n":i}})).collect());
        apply(&mut doc, keys.iter().map(|key| json!({"type":"clear","path":["entries",key]})).collect());
    }
    report("recordChurn", &schema, initial, &doc)
}

fn main() {
    println!("{}", serde_json::to_string_pretty(&json!([row_churn(), record_churn()])).unwrap());
}
