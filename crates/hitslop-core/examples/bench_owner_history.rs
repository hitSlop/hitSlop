//! Actual local Owner + persistence worker history workload. Run isolated in release:
//! cargo run --release -p hitslop-core --example bench_owner_history
//! HITSLOP_OWNER_HISTORY_CYCLES defaults to 20; smaller values are smoke checks only.
//! Reopen/value validation runs in a child, outside the measured owner's RSS.
use hitslop_core::{
    Origin, file,
    owner::{Owner, Reply, Request},
    store::{Mode, Store},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
#[path = "../tests/support/mod.rs"]
mod support;

const SCHEMA: &str = r#"{"kind":"object","properties":{
  "rows":{"kind":"list","item":{"kind":"object","properties":{
    "text":{"kind":"text"},"children":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"}}}}
  }}},"notes":{"kind":"record","value":{"kind":"object","properties":{"body":{"kind":"text"}}}},"optional":{"kind":"optional","inner":{"kind":"text"}}
}}"#;

fn call(owner: &Owner, request: Request) -> (Reply, f64) {
    let (send, receive) = mpsc::channel();
    let start = Instant::now();
    owner.submit(
        request,
        None,
        Box::new(move |reply| {
            let _ = send.send(reply);
        }),
    );
    let result = receive.recv_timeout(Duration::from_secs(120)).expect("owner completion").expect("owner accepted");
    (result, start.elapsed().as_secs_f64() * 1000.0)
}
fn apply(owner: &Owner, intents: Value) -> f64 {
    call(
        owner,
        Request::Apply { origin: Origin::Page, batch: serde_json::from_value(json!({"intents":intents})).unwrap() },
    )
    .1
}
/// The history the live owner retains: its full snapshot, and its current state alone.
#[derive(serde::Serialize)]
struct HistoryStats {
    retained_snapshot_bytes: usize,
    current_state_bytes: usize,
    full_export_ms: f64,
}
fn history(owner: &Owner) -> HistoryStats {
    let (send, receive) = mpsc::channel();
    owner.read_document(move |document| {
        let start = Instant::now();
        let _ = send.send((document.checkpoint(), start.elapsed().as_secs_f64() * 1000.0));
    });
    let (checkpoint, full_export_ms) = receive.recv_timeout(Duration::from_secs(120)).unwrap();
    let checkpoint = checkpoint.unwrap();
    let doc = loro::LoroDoc::new();
    doc.import(&checkpoint).unwrap();
    let state = doc.export(loro::ExportMode::shallow_snapshot(&doc.oplog_frontiers())).unwrap();
    HistoryStats { retained_snapshot_bytes: checkpoint.len(), current_state_bytes: state.len(), full_export_ms }
}
/// Waits for a due rebuild: it starts once editing pauses, and holds edits while it runs,
/// so an empty edit after the pause returns once it is done.
fn settle_rebuild(owner: &Owner) -> f64 {
    std::thread::sleep(Duration::from_millis(2_500));
    apply(owner, json!([]))
}
fn stored(conn: &Connection) -> (i64, i64, i64) {
    conn.query_row("SELECT (SELECT count(*) FROM updates),(SELECT coalesce(sum(length(bytes)),0) FROM updates),(SELECT length(bytes) FROM checkpoint WHERE id=1)", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap()
}
fn samples(values: &[f64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    if sorted.is_empty() {
        return json!({"samples":0});
    }
    json!({"samples":sorted.len(),"p50Ms":sorted[(sorted.len()-1)/2],"p95Ms":sorted[((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)],"maxMs":sorted.last().unwrap()})
}
fn max_rss() -> usize {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes the pointed-to struct on success.
    assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) }, 0);
    // SAFETY: the successful getrusage call above initialized the entire struct.
    let rss = unsafe { usage.assume_init() }.ru_maxrss as usize;
    if cfg!(target_os = "macos") { rss } else { rss * 1024 }
}
fn validate(path: &Path, expected: &Path) {
    let store = Store::open(path, Mode::Snapshot).unwrap();
    let doc = store.document().unwrap();
    let actual: Value = serde_json::from_str(&doc.value()).unwrap();
    let expected: Value = serde_json::from_slice(&std::fs::read(expected).unwrap()).unwrap();
    assert_eq!(actual, expected, "constant live content survives owner/save/reopen");
}
fn prepare(root: &Path) {
    let stage = root.join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/ui.js"), "export default {}").unwrap();
    let rows: Vec<_> = (0..4_000).map(|i| json!({"$id":format!("row-{i}"),"text":format!("Row {i} café"),"children":[{"$id":format!("child-{i}"),"text":"Nested child"}]})).collect();
    let initial = json!({"rows":rows,"notes":{}}).to_string();
    let expected = root.join("expected.json");
    std::fs::write(&expected, &initial).unwrap();
    support::write_app(&stage, support::App::new(SCHEMA, &initial));
    let template = root.join("Template.slop");
    support::pack(&stage, &template).unwrap();
    let path = root.join("Owner.slop");
    file::create_document(&template, &path).unwrap();
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--validate") {
        validate(Path::new(&args[2]), Path::new(&args[3]));
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--prepare") {
        prepare(Path::new(&args[2]));
        return;
    }
    let target_cycles: usize =
        std::env::var("HITSLOP_OWNER_HISTORY_CYCLES").ok().map(|value| value.parse().unwrap()).unwrap_or(20);
    assert!(target_cycles > 0);
    let root = tempfile::Builder::new().prefix("hitslop-owner-history-").tempdir().unwrap().keep();
    hitslop_core::registry::use_folder(&root.join("registry")).unwrap();
    assert!(
        std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--prepare")
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let path = root.join("Owner.slop");
    let expected = root.join("expected.json");
    let open_start = Instant::now();
    let owner = Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap();
    let open_ms = open_start.elapsed().as_secs_f64() * 1000.0;
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let initial_history = history(&owner);
    let mut previous_rows = stored(&conn).0;
    let mut cycles = vec![];
    let mut edit_ms = vec![];
    let mut flush_ms = vec![];
    let mut checkpoint_flush_ms = vec![];
    let mut checkpoint_pending = None;
    let mut rng = 42u64;
    let start = Instant::now();
    let mut rounds = 0usize;
    while cycles.len() < target_cycles {
        rounds += 1;
        assert!(rounds < target_cycles * 2_000, "workload must reach checkpoint boundaries");
        let text: String = (0..16_384)
            .map(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                char::from(b'a' + (rng % 26) as u8)
            })
            .collect();
        for intents in [
            json!([{"type":"set","path":["notes","current"],"value":{"body":text}},{"type":"set","path":["optional"],"value":"Transient"}]),
            json!([{"type":"clear","path":["notes","current"]},{"type":"clear","path":["optional"]}]),
        ] {
            edit_ms.push(apply(&owner, intents));
            let flush = call(&owner, Request::Flush).1;
            flush_ms.push(flush);
            let sizes = stored(&conn);
            if sizes.0 < previous_rows {
                checkpoint_flush_ms.push(flush);
                checkpoint_pending = Some(flush);
            }
            previous_rows = sizes.0;
        }
        // Sample only after the delete, when every measurement has the same live state.
        if let Some(checkpoint_flush) = checkpoint_pending.take() {
            // Issued before fencing a possible background rebuild, to measure access
            // to the still-authoritative old state while preparation is in flight.
            let (_, post_checkpoint_read_ms) = call(&owner, Request::State);
            // A checkpoint can schedule a follow-on live rebuild. Fence that work too
            // before sampling retained history (the timed mutation flush stays above).
            let maintenance_fence_ms = settle_rebuild(&owner);
            let sizes = stored(&conn);
            let before_probe_rss = max_rss();
            let live = history(&owner);
            let (_, read_ms) = call(&owner, Request::State);
            cycles.push(json!({"cycle":cycles.len()+1,"rounds":rounds,"elapsedSeconds":start.elapsed().as_secs_f64(),
                "checkpointFlushMs":checkpoint_flush,"postCheckpointReadMs":post_checkpoint_read_ms,"maintenanceFenceMs":maintenance_fence_ms,"stateReadMs":read_ms,"history":live,
                "savedUpdateRows":sizes.0,"savedUpdateBytes":sizes.1,"savedCheckpointBytes":sizes.2,
                "databaseBytes":std::fs::metadata(&path).unwrap().len(),"maxRssBeforeProbeBytes":before_probe_rss,"maxRssAfterProbeBytes":max_rss()}));
            eprintln!(
                "checkpoint {}: round {rounds}, saved {} bytes, retained {} bytes",
                cycles.len(),
                sizes.1 + sizes.2,
                live.retained_snapshot_bytes
            );
        }
    }
    call(&owner, Request::Flush);
    // Prove the most recent user action remains undoable through automatic rebuilds.
    // Undo/redo and these verification reads are excluded from the timed workload.
    let (Reply::State { sequence: before_undo, .. }, _) = call(&owner, Request::State) else {
        panic!("state");
    };
    let (Reply::Applied { sequence: after_undo, .. }, _) = call(&owner, Request::Undo { redo: false }) else {
        panic!("undo");
    };
    assert!(after_undo > before_undo, "maintenance preserves a real undo step");
    let (Reply::Applied { sequence: after_redo, .. }, _) = call(&owner, Request::Undo { redo: true }) else {
        panic!("redo");
    };
    assert!(after_redo > after_undo, "maintenance preserves redo");
    call(&owner, Request::Flush);
    let before_close = stored(&conn);
    let live_rss = max_rss();
    let validation = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--validate")
        .arg(&path)
        .arg(&expected)
        .status()
        .unwrap();
    assert!(validation.success());
    let close_ms = call(&owner, Request::Close { preview: None, icon: None }).1;
    let after_close = stored(&conn);
    assert!(
        std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--validate")
            .arg(&path)
            .arg(&expected)
            .status()
            .unwrap()
            .success()
    );
    let report = json!({"profile":if cfg!(debug_assertions) {"debug"} else {"release"},"coreBuild":hitslop_core::BUILD_ID,"ownerRole":"local","rows":4000,"seed":42,"textBytesPerRound":16384,
        "savePolicy":"Explicit flush after every mutation; measures durable per-edit worst case without debounce/group commit",
        "cyclesTarget":target_cycles,"cyclesCompleted":cycles.len(),"rounds":rounds,"openMs":open_ms,"initialHistory":initial_history,
        "edit":samples(&edit_ms),"flushIncludingCheckpointConstructionAndSQLite":samples(&flush_ms),"checkpointFlush":samples(&checkpoint_flush_ms),
        "cycles":cycles,"maxOwnerProcessRssBytes":live_rss,"closeMs":close_ms,"beforeClose":{"rows":before_close.0,"updateBytes":before_close.1,"checkpointBytes":before_close.2},
        "afterClose":{"rows":after_close.0,"updateBytes":after_close.1,"checkpointBytes":after_close.2},"reopenValidation":"passed in separate processes before and after close", "retainedUndoRedo":"both produced publications after the measured workload and restored the original live state",
        "artifacts":root,"measurementLimits":["Single workload run; quantiles are per-operation samples, not repeated-run confidence bounds", "Flush includes owner scheduling, checkpoint construction when due, SQLite write and fsync; write-only time is not isolated", "RSS includes temporary checkpoint export buffers; fixture packing and validation run in separate child processes", "No WebKit page; State request measures owner projection, not UI frame latency"]});
    let report = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(root.join("report.json"), &report).unwrap();
    println!("{report}");
}
