//! Lower-bound probe: if native publication construction alone exceeds 2 ms,
//! the build + transfer + projection budget cannot pass. No UI/FFI cost is claimed.
use hitslop_core::Document;
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};
fn main() {
    let assets = PathBuf::from(env::args().nth(1).expect("usage: publication_cost ASSETS"));
    let schema = fs::read_to_string(assets.join("checklist.schema.json")).unwrap();
    let mut results = vec![];
    for rows in [1000, 5000] {
        let seed = fs::read(assets.join(format!("checklist-{rows}.snapshot"))).unwrap();
        let mut doc = Document::open(&schema, &seed, &[]).unwrap();
        let view: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        let id = view["value"]["rows"].as_array().unwrap().last().unwrap()["$id"].clone();
        let mut samples = vec![];
        for i in 0..20 {
            let started = std::time::Instant::now();
            let reply:Value=serde_json::from_str(&doc.apply(&json!({"intents":[{"type":"set","path":["rows",{"id":id},"done"],"value":i%2==0}]}).to_string()).unwrap()).unwrap();
            samples.push(started.elapsed().as_secs_f64() * 1e3);
            assert_eq!(reply["ops"].as_array().unwrap().len(), 1);
        }
        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);
        results.push(json!({"rows":rows,"samples":samples,"medianMS":sorted[10],"p95MS":sorted[18],"maxMS":sorted[19]}));
    }
    println!(
        "{}",
        json!({"method":"native construction only, excludes response encoding, bridge and renderer; a lower bound on total publication cost","results":results})
    );
}
