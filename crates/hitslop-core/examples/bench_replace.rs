//! Opt-in replacement timings: `cargo run --release -p hitslop-core --example bench_replace`.
use hitslop_core::{AppSpec, Document, Origin};
use serde_json::{Value, json};
use std::time::Instant;

fn main() {
    let app = AppSpec::data(r#"{"kind":"object","properties":{"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"},"done":{"kind":"boolean"}}}}}}"#).unwrap();
    let mut results = vec![];
    for count in [1000, 5000] {
        let rows: Vec<Value> =
            (0..count).map(|i| json!({"$id":format!("row_{i}"),"text":format!("Row {i}"),"done":false})).collect();
        let initial = json!({"rows":rows}).to_string();
        for scenario in ["unchanged", "fields", "append", "reverse", "mixed"] {
            let mut target = rows.clone();
            match scenario {
                "fields" => {
                    for row in &mut target {
                        row["done"] = true.into();
                    }
                }
                "append" => target.push(json!({"$id":"added","text":"Added","done":false})),
                "reverse" => target.reverse(),
                "mixed" => {
                    target.retain(|row| {
                        row["$id"].as_str().unwrap().strip_prefix("row_").unwrap().parse::<usize>().unwrap() % 5 != 0
                    });
                    target.reverse();
                    target.push(json!({"$id":"added","text":"Added","done":false}));
                }
                _ => {}
            }
            let batch = json!({"intents":[{"type":"replace","path":["rows"],"value":target}]}).to_string();
            let mut samples = vec![];
            for sample in 0..6 {
                let mut document = Document::create(&app, &initial).unwrap();
                let start = Instant::now();
                std::hint::black_box(
                    document.apply_batch(hitslop_core::Batch::decode(&batch).unwrap(), Origin::Agent).unwrap(),
                );
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                if sample > 0 {
                    samples.push(elapsed);
                }
            }
            samples.sort_by(f64::total_cmp);
            results.push(
                json!({"rows":count,"scenario":scenario,"medianMs":samples[samples.len()/2],"samplesMs":samples}),
            );
        }
    }
    println!("{}", serde_json::to_string_pretty(&results).unwrap());
}
