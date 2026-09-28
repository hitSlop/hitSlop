//! Per-operation native core cost (parse, apply, commit, event publication, response
//! encoding) at three document sizes. Excludes FFI, bridge and renderer. The goal is
//! the shape: row edits should stay flat as the document grows.
use hitslop_core::Document;
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf, time::Instant};
fn p95(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[(v.len() * 95 / 100).min(v.len() - 1)]
}
fn time(samples: usize, mut f: impl FnMut(usize)) -> f64 {
    let mut out = vec![];
    for i in 0..samples {
        let t = Instant::now();
        f(i);
        out.push(t.elapsed().as_secs_f64() * 1e3);
    }
    p95(out)
}
fn main() {
    let assets = PathBuf::from(env::args().nth(1).expect("usage: cost_attribution ASSETS"));
    let schema = fs::read_to_string(assets.join("checklist.schema.json")).unwrap();
    for rows in [1000, 5000, 40000] {
        let seed = fs::read(assets.join(format!("checklist-{rows}.snapshot"))).unwrap();
        let t = Instant::now();
        let mut doc = Document::open(&schema, &seed, &[]).unwrap();
        let open = t.elapsed().as_secs_f64() * 1e3;
        let view: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        let ids: Vec<Value> = view["value"]["rows"].as_array().unwrap().iter().map(|r| r["$id"].clone()).collect();
        let (first, mid, last) = (ids[0].clone(), ids[rows / 2].clone(), ids[rows - 1].clone());
        let apply = |doc: &mut Document, op: Value| doc.apply(&json!({"intents":[op]}).to_string()).unwrap();
        let checkbox = time(40, |i| { apply(&mut doc, json!({"type":"set","path":["rows",{"id":mid},"done"],"value":i%2==0})); });
        let text = time(40, |_| { let v = doc.version(); apply(&mut doc, json!({"type":"splice","path":["title"],"base":v,"index":0,"delete":0,"insert":"x"})); });
        let row_text = time(40, |_| { let v = doc.version(); apply(&mut doc, json!({"type":"splice","path":["rows",{"id":mid},"text"],"base":v,"index":0,"delete":0,"insert":"y"})); });
        let insert = time(40, |i| { apply(&mut doc, json!({"type":"insert","path":["rows"],"id":format!("ins{rows}x{i}"),"value":{"text":"new","done":false},"at":{"before":mid}})); });
        let remove = time(40, |i| { apply(&mut doc, json!({"type":"remove","path":["rows"],"id":format!("ins{rows}x{i}")})); });
        let mv = time(40, |i| { let (id, to) = if i % 2 == 0 { (&first, json!({"after":last})) } else { (&first, json!({"before":mid})) }; apply(&mut doc, json!({"type":"move","path":["rows"],"id":id,"at":to})); });
        // Remote: a peer toggles one checkbox / makes 100 checkbox edits; one import each.
        let mut peer = Document::open(&schema, &doc.checkpoint().unwrap(), &[]).unwrap();
        let mut import1_samples = vec![];
        for i in 0..20 {
            let base = peer.version();
            peer.apply(&json!({"intents":[{"type":"set","path":["rows",{"id":last},"done"],"value":i%2==0}]}).to_string()).unwrap();
            let bytes = peer.export_since(&base).unwrap();
            let t = Instant::now(); doc.import(&bytes).unwrap(); import1_samples.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let import1 = p95(import1_samples);
        let mut import100_samples = vec![];
        for round in 0..10 {
            let base = peer.version();
            for k in 0..100 { let id = &ids[(k * 37 + round) % rows]; peer.apply(&json!({"intents":[{"type":"set","path":["rows",{"id":id},"done"],"value":round%2==0}]}).to_string()).unwrap(); }
            let bytes = peer.export_since(&base).unwrap();
            let t = Instant::now(); doc.import(&bytes).unwrap(); import100_samples.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let snapshot = time(5, |_| { doc.snapshot().unwrap(); });
        println!("{}", json!({"rows":rows,"openMS":open,"p95MS":{"checkbox":checkbox,"titleSplice":text,"rowTextSplice":row_text,"insertRow":insert,"removeRow":remove,"moveRow":mv,"importCheckbox":import1,"import100":p95(import100_samples),"fullSnapshot":snapshot}}));
    }
}
