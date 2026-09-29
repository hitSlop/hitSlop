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
    // 20 peers each make 500 edits (toggles and text splices), merged into the owner.
    for p in 0..20 {
        let base = doc.version();
        let mut peer = Document::open(&schema, &doc.checkpoint().unwrap(), &[]).unwrap();
        for k in 0..500 {
            let id = format!("r{}", (p * 997 + k * 31) % 5000);
            let op = if k % 2 == 0 {
                json!({"type":"set","path":["rows",{"id":id},"done"],"value":k % 4 == 0})
            } else {
                json!({"type":"splice","path":["rows",{"id":id},"text"],"base":peer.version(),"index":0,"delete":0,"insert":"x"})
            };
            peer.apply(&json!({"intents":[op]}).to_string()).unwrap();
        }
        doc.import(&peer.export_since(&base).unwrap()).unwrap();
    }
    let path = json!(["rows",{"id":"r2500"},"text"]);
    let text = |d: &Document| -> String {
        let v: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
        v["value"]["rows"].as_array().unwrap().iter().find(|r| r["$id"] == "r2500").unwrap()["text"].as_str().unwrap().to_owned()
    };
    let (mut base, mut from) = (doc.version(), text(&doc));
    let (mut fast, mut slow) = (vec![], vec![]);
    for i in 0..60 {
        let to = format!("{from}{}", i % 10);
        let caret = to.encode_utf16().count();
        let concurrent = i % 2 == 1;
        if concurrent {
            // Another writer prepends to the same field: the next page edit takes the slow path.
            doc.apply(&json!({"intents":[{"type":"splice","path":path,"base":doc.version(),"index":0,"delete":0,"insert":"c"}]}).to_string()).unwrap();
        }
        let request = json!({"base":base,"path":path,"from":from,"to":to,"selectionStart":caret,"selectionEnd":caret}).to_string();
        let started = Instant::now();
        let reply = doc.edit_text(&request).unwrap();
        let ms = started.elapsed().as_secs_f64() * 1e3;
        if concurrent { slow.push(ms) } else { fast.push(ms) }
        assert!(!reply.authored.is_empty());
        // The page adopts the pushed merged text before its next edit.
        (base, from) = (doc.version(), text(&doc));
    }
    let bytes = doc.checkpoint().unwrap().len();
    println!("{}", json!({"rows":5000,"peers":21,"checkpointBytes":bytes,"p95MS":{"fast":p95(fast),"forkSlow":p95(slow)}}));
}
