//! Storage costs on a real file, with fsync: opening a saved document with a full update
//! log, appended saves, checkpoint saves (each compacting a full log; export and write
//! timed separately), theme saves and duplication. Run with
//! `cargo run --release -p hitslop-core --features storage --example store_cost`.
use hitslop_core::store::{self, Mode, Store};
use hitslop_core::{theme::Change, Document};
use serde_json::{json, Value};
use std::time::Instant;

fn summary(mut v: Vec<f64>) -> Value {
    v.sort_by(f64::total_cmp);
    let round = |x: f64| (x * 100.0).round() / 100.0;
    json!({ "median": round(v[v.len() / 2]), "max": round(v[v.len() - 1]) })
}
fn ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1e3
}
/// Fills the update log with 255 single-edit saves, returning each save's time.
fn fill_log(store: &Store, doc: &mut Document, rows: usize, round: usize) -> Vec<f64> {
    (0..255)
        .map(|i| {
            let id = format!("r{}", (i * 7 + round) % rows);
            doc.apply_batch(&json!({"intents":[{"type":"set","path":["rows",{"id":id},"text"],"value":format!("Edit {round}.{i}")}]}).to_string()).unwrap();
            let started = Instant::now();
            let job = store.job(doc, false).unwrap().unwrap();
            assert!(!job.is_checkpoint());
            store.write(&job).unwrap();
            ms(started)
        })
        .collect()
}

fn main() {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let schema = fixture["schema"].to_string();
    let defaults = r##"{"accent":"#335577"}"##;
    let mut out = serde_json::Map::new();
    // Short rows at three sizes, plus long incompressible text for a MiB-scale checkpoint
    // (initial JSON is limited to 4 MiB).
    for (rows, text) in [(1_000, 12), (5_000, 12), (20_000, 12), (3_500, 1_000)] {
        let label = format!("{rows}x{text}");
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Cost.slop");
        std::fs::create_dir(&root).unwrap();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut letter = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (b'a' + (seed >> 59) as u8 % 26) as char
        };
        let items: Vec<Value> = (0..rows)
            .map(|i| json!({"$id":format!("r{i}"),"text":(0..text).map(|_| letter()).collect::<String>(),"done":false}))
            .collect();
        let initial = json!({"title":"t","rows":items,"hits":0}).to_string();
        let key = hitslop_core::validate(&schema, &initial).unwrap();
        let store = Store::open(&root, Mode::Document).unwrap();
        let mut doc = store.document(&key, &initial).unwrap();
        let appends = fill_log(&store, &mut doc, rows, 0);
        store.close().unwrap();
        let opens: Vec<f64> = (0..5)
            .map(|_| {
                let started = Instant::now();
                let store = Store::open(&root, Mode::Document).unwrap();
                store.document(&key, &initial).unwrap();
                let elapsed = ms(started);
                store.close().unwrap();
                elapsed
            })
            .collect();
        let store = Store::open(&root, Mode::Document).unwrap();
        let mut doc = store.document(&key, &initial).unwrap();
        let (mut exports, mut writes) = (vec![], vec![]);
        for round in 1..=5 {
            // Every sample compacts a full log, as a real checkpoint does; the first
            // uses the log saved before reopening.
            if round > 1 {
                fill_log(&store, &mut doc, rows, round);
            }
            let started = Instant::now();
            let job = store.job(&doc, true).unwrap().unwrap();
            exports.push(ms(started));
            let started = Instant::now();
            store.write(&job).unwrap();
            writes.push(ms(started));
        }
        fill_log(&store, &mut doc, rows, 6);
        let themes: Vec<f64> = (0..7)
            .map(|i| {
                let values = format!(r##"{{"accent":"#{i:06}"}}"##);
                let started = Instant::now();
                store.theme(defaults, Change::Set(&values)).unwrap();
                ms(started)
            })
            .collect();
        let duplicates: Vec<f64> = (0..5)
            .map(|i| {
                let copy = dir.path().join(format!("Copy {i}.slop"));
                std::fs::create_dir(&copy).unwrap();
                let started = Instant::now();
                store::duplicate(&root, &copy).unwrap();
                let elapsed = ms(started);
                std::fs::remove_dir_all(&copy).unwrap();
                elapsed
            })
            .collect();
        let meta = store.metadata().unwrap();
        out.insert(label, json!({
            "appendSaveMS": summary(appends),
            "openWith255UpdatesMS": summary(opens),
            "checkpointExportMS": summary(exports),
            "checkpointWriteMS": summary(writes),
            "themeSaveMS": summary(themes),
            "duplicateMS": summary(duplicates),
            "checkpointBytes": meta.checkpoint_bytes,
            "fileBytes": std::fs::metadata(root.join("state/document.sqlite")).unwrap().len(),
        }));
        store.close().unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&Value::Object(out)).unwrap());
}
