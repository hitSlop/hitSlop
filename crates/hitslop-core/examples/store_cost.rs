//! Storage costs on a real file, with fsync: opening a saved document with a full update
//! log, appended saves, maintenance checkpoints and compactions (each replacing a full
//! log; export and write timed separately), theme saves and duplication. Run with
//! `cargo run --release -p hitslop-core --features storage --example store_cost`.
use hitslop_core::file;
use hitslop_core::store::{Mode, Store};
use hitslop_core::{theme::Change, Document};
use serde_json::{json, Value};
use std::time::Instant;
use hitslop_core::Origin;

fn summary(mut v: Vec<f64>) -> Value {
    v.sort_by(f64::total_cmp);
    let round = |x: f64| (x * 100.0).round() / 100.0;
    json!({ "median": round(v[v.len() / 2]), "max": round(v[v.len() - 1]) })
}
fn ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1e3
}
fn edit(doc: &mut Document, rows: usize, round: usize, i: usize) {
    let id = format!("r{}", (i * 7 + round) % rows);
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["rows",{"id":id},"text"],"value":format!("Edit {round}.{i}")}]}).to_string(), Origin::Page).unwrap();
}
/// Fills the update log with `count` single-edit saves, returning each save's time.
fn fill_log(store: &Store, doc: &mut Document, rows: usize, round: usize, count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| {
            edit(doc, rows, round, i);
            let started = Instant::now();
            let job = store.job(doc, false).unwrap().unwrap();
            assert!(!job.is_checkpoint());
            store.write(&job).unwrap();
            ms(started)
        })
        .collect()
}
/// Saves one more edit as a checkpoint, maintenance or compaction, returning the export
/// and write times.
fn checkpoint(store: &Store, doc: &mut Document, rows: usize, round: usize, compact: bool) -> (f64, f64) {
    edit(doc, rows, round, 999);
    let started = Instant::now();
    let job = store.job(doc, compact).unwrap().unwrap();
    assert!(job.is_checkpoint());
    let export = ms(started);
    let started = Instant::now();
    store.write(&job).unwrap();
    (export, ms(started))
}

fn main() {
    // A tool run never fills the account's `~/.hitslop/live` with lock files.
    hitslop_core::registry::use_folder(&std::env::temp_dir().join("hitslop-test-registry")).unwrap();
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap();
    let defaults = r##"{"accent":"#335577"}"##;
    let mut out = serde_json::Map::new();
    // Short rows at three sizes, plus long incompressible text for a MiB-scale checkpoint
    // (initial JSON is limited to 4 MiB).
    for (rows, text) in [(1_000, 12), (5_000, 12), (20_000, 12), (3_500, 1_000)] {
        let label = format!("{rows}x{text}");
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Cost.slop");
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut letter = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (b'a' + (seed >> 59) as u8 % 26) as char
        };
        let items: Vec<Value> = (0..rows)
            .map(|i| json!({"$id":format!("r{i}"),"text":(0..text).map(|_| letter()).collect::<String>(),"done":false}))
            .collect();
        let initial = json!({"title":"t","rows":items,"hits":0}).to_string();
        let template = dir.path().join("Cost.template.slop");
        let stage = dir.path().join("stage");
        std::fs::create_dir_all(stage.join("assets")).unwrap();
        std::fs::write(stage.join("assets/app.js"), "export default {}").unwrap();
        let manifest = r#"{"author":{"name":"Bench"},"slug":"cost","title":"Cost","description":"Storage costs.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#;
        let app = json!({
            "packageFormat": hitslop_core::PACKAGE_FORMAT,
            "runtimeABI": hitslop_core::RUNTIME_ABI,
            "manifest": serde_json::from_str::<Value>(manifest).unwrap(),
            "descriptor": fixture["schema"],
            "initial": serde_json::from_str::<Value>(&initial).unwrap(),
            "theme": serde_json::from_str::<Value>(defaults).unwrap(),
        });
        std::fs::write(stage.join("app.json"), app.to_string()).unwrap();
        file::pack(&stage, &template).unwrap();
        file::create_document(&template, &root).unwrap();
        let store = Store::open(&root, Mode::Document).unwrap();
        let mut doc = store.document().unwrap();
        let appends = fill_log(&store, &mut doc, rows, 0, 255);
        store.close().unwrap();
        let opens: Vec<f64> = (0..5)
            .map(|_| {
                let started = Instant::now();
                let store = Store::open(&root, Mode::Document).unwrap();
                store.document().unwrap();
                let elapsed = ms(started);
                store.close().unwrap();
                elapsed
            })
            .collect();
        let store = Store::open(&root, Mode::Document).unwrap();
        let mut doc = store.document().unwrap();
        // Every sample replaces a full log, as a real checkpoint does; the first uses the
        // log saved before reopening.
        let (mut exports, mut writes, mut compactions) = (vec![], vec![], vec![]);
        for round in 1..=5 {
            fill_log(&store, &mut doc, rows, round, if round > 1 { 256 } else { 1 });
            let (export, write) = checkpoint(&store, &mut doc, rows, round, false);
            exports.push(export);
            writes.push(write);
        }
        for round in 6..=10 {
            fill_log(&store, &mut doc, rows, round, 255);
            compactions.push(checkpoint(&store, &mut doc, rows, round, true).0);
        }
        fill_log(&store, &mut doc, rows, 11, 255);
        let themes: Vec<f64> = (0..7)
            .map(|i| {
                let values = format!(r##"{{"accent":"#{i:06}"}}"##);
                // A theme change is held in memory; this times the theme-only save job.
                let started = Instant::now();
                store.theme(Change::Set(&values)).unwrap();
                let job = store.job(&mut doc, false).unwrap().expect("a theme-only job");
                store.write(&job).unwrap();
                ms(started)
            })
            .collect();
        let duplicates: Vec<f64> = (0..5)
            .map(|i| {
                let copy = dir.path().join(format!("Copy {i}.slop"));
                let started = Instant::now();
                // The owner copies the open document; saves wait behind it.
                store.copy_to(&copy).unwrap();
                let elapsed = ms(started);
                std::fs::remove_file(&copy).unwrap();
                elapsed
            })
            .collect();
        let checkpoint_bytes: i64 = rusqlite::Connection::open(&root)
            .unwrap()
            .query_row("SELECT coalesce(sum(length(bytes)),0) FROM checkpoint", [], |r| r.get(0))
            .unwrap();
        out.insert(label, json!({
            "appendSaveMS": summary(appends),
            "openWith255UpdatesMS": summary(opens),
            "checkpointExportMS": summary(exports),
            "checkpointWriteMS": summary(writes),
            "compactExportMS": summary(compactions),
            "themeSaveMS": summary(themes),
            "duplicateMS": summary(duplicates),
            "checkpointBytes": checkpoint_bytes,
            "fileBytes": std::fs::metadata(&root).unwrap().len(),
        }));
        store.close().unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&Value::Object(out)).unwrap());
}
