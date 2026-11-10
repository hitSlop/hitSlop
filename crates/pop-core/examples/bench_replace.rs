//! Opt-in replacement timings: `cargo run --release -p pop-core --example bench_replace`.
use pop_core::{AppSpec, Document, Origin};
use serde_json::{Value, json};
use std::time::Instant;

fn main() {
    let app = AppSpec::new(r#"{"kind":"object","properties":{"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"},"done":{"kind":"boolean"}}}}}}"#, "", "{}").unwrap();
    let mut results = vec![];
    for count in [1000, 5000] {
        let rows: Vec<Value> =
            (0..count).map(|i| json!({"$id":format!("row_{i}"),"text":format!("Row {i}"),"done":false})).collect();
        let initial = json!({"rows":rows}).to_string();
        for scenario in ["unchanged", "fields", "append", "reverse", "mixed", "row-addressed-set"] {
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
                "row-addressed-set" => target[count - 1]["done"] = true.into(),
                _ => {}
            }
            let batch = if scenario == "row-addressed-set" {
                json!({"intents":[{"type":"set","path":["rows",{"id":format!("row_{}",count-1)},"done"],"value":true}]})
            } else {
                json!({"intents":[{"type":"replace","path":["rows"],"value":target}]})
            }
            .to_string();
            let expected = json!({"rows":target});
            let mut samples = vec![];
            let mut first = 0.0;
            let mut publication_bytes = 0;
            for sample in 0..6 {
                let mut document = Document::create(&app, &initial).unwrap();
                let batch = pop_contracts::wire::Batch::decode(&batch).unwrap();
                let start = Instant::now();
                let applied = std::hint::black_box(document.apply_batch(batch, Origin::Agent).unwrap());
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                // Verify outside the timed section against the independently edited JSON.
                assert_eq!(document.reading().unwrap().value, expected, "{count} rows, {scenario}");
                publication_bytes = applied.publication.as_ref().map_or(0, String::len);
                if sample == 0 {
                    first = elapsed;
                } else {
                    samples.push(elapsed);
                }
            }
            samples.sort_by(f64::total_cmp);
            results.push(json!({"rows":count,"scenario":scenario,"firstMs":first,
                    "medianMs":samples[samples.len()/2],"p95Ms":samples[samples.len()-1],
                    "publicationBytes":publication_bytes,"samplesMs":samples}));
        }
    }
    // Typical page change(): two small edits, independent of the document's size.
    let rows: Vec<Value> =
        (0..4096).map(|i| json!({"$id":format!("row_{i}"),"text":"x".repeat(960),"done":false})).collect();
    let initial = json!({"rows":rows}).to_string();
    let batch = json!({"intents":[
        {"type":"set","path":["rows",{"id":"row_0"},"done"],"value":true},
        {"type":"set","path":["rows",{"id":"row_1"},"done"],"value":true}
    ]})
    .to_string();
    let mut samples = vec![];
    let mut expected: Value = serde_json::from_str(&initial).unwrap();
    expected["rows"][0]["done"] = true.into();
    expected["rows"][1]["done"] = true.into();
    let mut first = 0.0;
    let mut publication_bytes = 0;
    for sample in 0..11 {
        let mut document = Document::create(&app, &initial).unwrap();
        let batch = pop_contracts::wire::Batch::decode(&batch).unwrap();
        let start = Instant::now();
        let applied = std::hint::black_box(document.apply_batch(batch, Origin::Page).unwrap());
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(document.reading().unwrap().value, expected);
        publication_bytes = applied.publication.as_ref().map_or(0, String::len);
        if sample == 0 {
            first = elapsed;
        } else {
            samples.push(elapsed);
        }
    }
    samples.sort_by(f64::total_cmp);
    results.push(json!({"rows":4096,"stateBytes":initial.len(),"scenario":"two-intent-page-batch",
        "firstMs":first,"medianMs":samples[samples.len()/2],"p95Ms":samples[samples.len()-1],
        "publicationBytes":publication_bytes,"samplesMs":samples}));
    println!("{}", serde_json::to_string_pretty(&results).unwrap());
}
