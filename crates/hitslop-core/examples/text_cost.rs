//! Stateless text cost at 5k rows with a mature history (about 10k operations from 20
//! peers): the fast path (the field is still `from`) and the fork slow path (the same
//! field changed concurrently). Run with `cargo run --release --example text_cost`.
use hitslop_core::Document;
use serde_json::{json, Value};
use std::time::Instant;
fn p95(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[(v.len() * 95 / 100).min(v.len() - 1)]
}
fn main() {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let schema = fixture["schema"].to_string();
    let rows: Vec<Value> = (0..5000).map(|i| json!({"$id":format!("r{i}"),"text":format!("Task number {i}"),"done":false})).collect();
    let mut doc = Document::create(&schema, &json!({"title":"t","rows":rows,"hits":0}).to_string()).unwrap();
    // 20 peers each make 500 edits (toggles and text sets), merged into the owner.
    for p in 0..20 {
        let base = doc.version();
        let mut peer = Document::open(&schema, &doc.checkpoint().unwrap(), &[]).unwrap();
        for k in 0..500 {
            let id = format!("r{}", (p * 997 + k * 31) % 5000);
            let op = if k % 2 == 0 {
                json!({"type":"set","path":["rows",{"id":id},"done"],"value":k % 4 == 0})
            } else {
                json!({"type":"set","path":["rows",{"id":id},"text"],"value":format!("x{k} task")})
            };
            peer.apply_batch(&json!({"intents":[op]}).to_string()).unwrap();
        }
        doc.import(&peer.export_since(&base).unwrap()).unwrap();
    }
    let path = json!(["rows",{"id":"r2500"},"text"]);
    let text = |d: &Document| -> String {
        let v: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
        v["value"]["rows"].as_array().unwrap().iter().find(|r| r["$id"] == "r2500").unwrap()["text"].as_str().unwrap().to_owned()
    };
    let checkpoint = doc.checkpoint().unwrap();
    for length in [1_000, 10_000, 100_000] {
        let mut doc = Document::open(&schema, &checkpoint, &[]).unwrap();
        doc.apply_batch(&json!({"intents":[{"type":"set","path":path,"value":"x".repeat(length)}]}).to_string()).unwrap();
        let (mut base, mut from) = (doc.version(), text(&doc));
        let (mut fast, mut slow, mut sets) = (vec![], vec![], vec![]);
        for i in 0..60 {
            let to = format!("{from}{}", i % 10);
            let caret = to.encode_utf16().count();
            let concurrent = i % 2 == 1;
            if concurrent {
                doc.apply_batch(&json!({"intents":[{"type":"set","path":path,"value":format!("c{from}")}]}).to_string()).unwrap();
            }
            let request = json!({"base":base,"path":path,"from":from,"to":to,"selectionStart":caret,"selectionEnd":caret}).to_string();
            let started = Instant::now();
            let reply = doc.edit_text(&request).unwrap();
            let ms = started.elapsed().as_secs_f64() * 1e3;
            if concurrent { slow.push(ms) } else { fast.push(ms) }
            assert!(!reply.authored.is_empty());
            (base, from) = (doc.version(), text(&doc));
        }
        for i in 0..30 {
            let request = json!({"intents":[{"type":"set","path":path,"value":format!("{}{}", "x".repeat(length), i)}]}).to_string();
            let started = Instant::now();
            doc.apply_batch(&request).unwrap();
            sets.push(started.elapsed().as_secs_f64() * 1e3);
        }
        println!("{}", json!({"characters":length,"rows":5000,"peers":21,"checkpointBytes":doc.checkpoint().unwrap().len(),"p95MS":{"fast":p95(fast),"forkSlow":p95(slow),"set":p95(sets)}}));
    }
}
