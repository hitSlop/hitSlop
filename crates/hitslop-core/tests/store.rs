//! Durable storage: identity, limits, crash outcomes and the writer lock, over one
//! document file. Faults are real ones: another connection holding the database, damage
//! written from outside, a moved file.
use hitslop_core::file;
use hitslop_core::registry::Lease;
use hitslop_core::store::{Error, Mode, Store};
use hitslop_core::{STORAGE_BYTES, STORAGE_ROWS};
use hitslop_core::Document;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use hitslop_core::Origin;
mod support;
use support::{app, isolate_registry, type_text, write_app, App};

const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"string"},"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"string"}}}}}}"#;
const INITIAL: &str = r#"{"title":"Saved","rows":[]}"#;

/// A new document file, created from a packed template of `schema` and `initial`, alone
/// in its folder.
fn document_with(schema: &str, initial: &str) -> (tempfile::TempDir, PathBuf) {
    isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/app.js"), "export default {}").unwrap();
    write_app(&stage, App::new(schema, initial));
    let template = dir.path().join("Doc.template.slop");
    file::pack(&stage, &template).unwrap();
    let path = dir.path().join("Doc.slop");
    file::create_document(&template, &path).unwrap();
    std::fs::remove_dir_all(&stage).unwrap();
    std::fs::remove_file(&template).unwrap();
    (dir, path)
}
fn document() -> (tempfile::TempDir, PathBuf) {
    document_with(SCHEMA, INITIAL)
}
fn open(path: &Path) -> (Store, Document) {
    let store = Store::open(path, Mode::Document).unwrap();
    let doc = store.document().unwrap();
    (store, doc)
}
fn sql(path: &Path) -> Connection {
    Connection::open(path).unwrap()
}
/// What the file holds: saved update rows, update bytes and checkpoint bytes.
#[derive(Debug, PartialEq)]
struct Stored {
    rows: i64,
    update_bytes: i64,
    checkpoint_bytes: i64,
}
fn stored(path: &Path) -> Stored {
    sql(path)
        .query_row(
            "SELECT (SELECT count(*) FROM updates),(SELECT coalesce(sum(length(bytes)),0) FROM updates),(SELECT coalesce(sum(length(bytes)),0) FROM checkpoint)",
            [],
            |r| Ok(Stored { rows: r.get(0)?, update_bytes: r.get(1)?, checkpoint_bytes: r.get(2)? }),
        )
        .unwrap()
}
/// Another connection holding the database, as a backup or another SQLite program would.
fn hold(path: &Path) -> Connection {
    let other = sql(path);
    other.busy_timeout(std::time::Duration::ZERO).unwrap();
    other.execute_batch("BEGIN EXCLUSIVE").unwrap();
    other
}
fn title(doc: &Document) -> String {
    let value: Value = serde_json::from_str(&doc.value().unwrap()).unwrap();
    value["title"].as_str().unwrap().into()
}
fn set_title(doc: &mut Document, title: &str) {
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["title"],"value":title}]}).to_string(), Origin::Page).unwrap();
}
fn save(store: &Store, doc: &mut Document) -> Option<bool> {
    let job = store.job(doc, false).unwrap()?;
    store.write(&job).unwrap();
    Some(job.is_checkpoint())
}

// Failure: an absent checkpoint was treated as initial state even with saved updates,
// and opening then deleted those updates. Every open refuses the file and preserves it.
fn missing_checkpoint_is_refused(mode: Mode, saved_edits: bool) {
    let (_dir, path) = document();
    if saved_edits {
        let (store, mut doc) = open(&path);
        for title in ["First", "Second", "Third"] {
            set_title(&mut doc, title);
            save(&store, &mut doc);
        }
        store.close().unwrap();
    }
    sql(&path).execute("DELETE FROM checkpoint", []).unwrap();
    let before = std::fs::read(&path).unwrap();
    let error = Store::open(&path, mode).err().expect("a document without saved state is refused");
    assert!(error.to_string().contains("keep it for recovery"), "{error}");
    assert_eq!(std::fs::read(&path).unwrap(), before, "nothing is written");
}

#[test]
fn opening_refuses_saved_updates_without_a_checkpoint() {
    missing_checkpoint_is_refused(Mode::Document, true);
    missing_checkpoint_is_refused(Mode::Snapshot, true);
}

#[test]
fn opening_refuses_a_document_with_no_checkpoint_or_updates() {
    missing_checkpoint_is_refused(Mode::Document, false);
    missing_checkpoint_is_refused(Mode::Snapshot, false);
}

#[test]
fn newly_created_document_snapshots_share_initial_row_ids_and_history() {
    let (_dir, path) = document_with(SCHEMA, r#"{"title":"Saved","rows":[{"text":"One"},{"text":"Two"}]}"#);
    let bytes = std::fs::read(&path).unwrap();
    let read = || Store::open(&path, Mode::Snapshot).unwrap().document().unwrap();
    let first = read();
    let second = read();
    assert_eq!(first.state().unwrap(), second.state().unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "initial snapshots never write");
    let (store, doc) = open(&path);
    assert_eq!(doc.state().unwrap(), first.state().unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "opening never initializes document state");
    store.close().unwrap();
}

#[test]
fn a_new_document_saves_its_first_checkpoint_and_reopens_with_its_edits() {
    let (_dir, path) = document();
    assert!(stored(&path).checkpoint_bytes > 0, "creation publishes the initial checkpoint before any open");
    let (store, mut doc) = open(&path);
    assert_eq!(stored(&path).rows, 0);
    assert!(stored(&path).checkpoint_bytes > 0);
    assert!(store.job(&mut doc, false).unwrap().is_none(), "a clean document has nothing to save");
    set_title(&mut doc, "Edited");
    assert_eq!(save(&store, &mut doc), Some(false), "an edit appends");
    assert_eq!(stored(&path).rows, 1);
    assert!(store.job(&mut doc, false).unwrap().is_none());
    store.close().unwrap();
    let (_, doc) = open(&path);
    assert_eq!(title(&doc), "Edited");
}

#[test]
fn a_long_log_checkpoints_and_compaction_is_always_a_checkpoint() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    let first = doc.version();
    // Saves append, until a long log is replaced by a checkpoint well before the row limit.
    let appended = (0..STORAGE_ROWS).take_while(|i| {
        set_title(&mut doc, &format!("Edit {i}"));
        save(&store, &mut doc) == Some(false)
    });
    let appended = appended.count();
    assert!(appended > 1 && appended < STORAGE_ROWS, "{appended} appends before a checkpoint");
    assert_eq!(stored(&path).rows, 0);
    store.close().unwrap();
    let (store, mut doc) = open(&path);
    assert!(!stale(&doc, &first), "a small checkpoint keeps its whole history");
    let job = store.job(&mut doc, true).unwrap().expect("a requested checkpoint is written even when clean");
    assert!(job.is_checkpoint());
    store.write(&job).unwrap();
    store.close().unwrap();
    assert_eq!(title(&open(&path).1), format!("Edit {appended}"));
}

/// Whether `doc` refuses `version` as a text base, as history before its retained start.
fn stale(doc: &Document, version: &str) -> bool {
    let mut scratch = Document::open(&app(SCHEMA), &doc.checkpoint().unwrap(), &[]).unwrap();
    matches!(type_text(&mut scratch, version, json!(["title"]), "", "", 0), Err(e) if e.code.as_str() == "stale_base")
}

/// Incompressible text of `len` letters.
fn noise(seed: &mut u64, len: usize) -> String {
    (0..len)
        .map(|_| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (b'a' + (*seed >> 59) as u8 % 26) as char
        })
        .collect()
}

/// Closes like the owner does: the last save, the closing checkpoint, then the lock.
fn close(store: Store, doc: &mut Document) -> Option<bool> {
    save(&store, doc);
    let job = store.close_job(doc).unwrap();
    if let Some(job) = &job {
        store.write(job).unwrap();
    }
    store.close().unwrap();
    job.map(|job| job.is_checkpoint())
}
/// One editing session of `count` large edits; returns the version after its first edit.
fn session(path: &Path, seed: &mut u64, count: usize) -> String {
    let (store, mut doc) = open(path);
    set_title(&mut doc, &noise(seed, 32 * 1024));
    let first = doc.version();
    save(&store, &mut doc);
    for _ in 1..count {
        set_title(&mut doc, &noise(seed, 32 * 1024));
        save(&store, &mut doc);
    }
    close(store, &mut doc);
    first
}

// Failure: every checkpoint kept the full history, so a document's file grew with every
// change it ever saw. Oracle: a session that edited a large document closes with no
// history, a session that only reads trims nothing, and compaction keeps no history.
#[test]
fn closing_a_large_document_keeps_no_history_and_compaction_trims_to_now() {
    let (_dir, path) = document();
    let mut seed = 7;
    let first = session(&path, &mut seed, 100);
    let second = session(&path, &mut seed, 100);
    let (store, mut doc) = open(&path);
    assert!(stale(&doc, &first) && stale(&doc, &second), "the large document closed with no history");
    let before = stored(&path);
    assert_eq!(close(store, &mut doc), None, "a session that only reads trims nothing");
    assert_eq!(stored(&path), before);
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Edited");
    let edited = doc.version();
    set_title(&mut doc, "Compacted");
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let latest = doc.version();
    store.close().unwrap();
    let doc = open(&path).1;
    assert!(stale(&doc, &edited), "compaction keeps no history");
    assert!(!stale(&doc, &latest));
    assert_eq!(title(&doc), "Compacted");
}

// Failure: a cut at the session's start kept every row deleted before it, so a document
// that deletes a lot never shrank. Oracle: it closes to its live value with no history.
#[test]
fn a_session_too_large_to_keep_closes_with_no_history() {
    let (_dir, path) = document();
    let mut seed = 3;
    let mut opened = String::new();
    for _ in 0..2 {
        let (store, mut doc) = open(&path);
        opened = doc.version();
        for _ in 0..160 {
            let applied = doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"value":{"text":noise(&mut seed, 32 * 1024)}}]}).to_string(), Origin::Page).unwrap();
            doc.apply_batch(&json!({"intents":[{"type":"remove","path":["rows"],"id":applied.ids[0]}]}).to_string(), Origin::Page).unwrap();
            save(&store, &mut doc);
        }
        close(store, &mut doc);
    }
    let (_store, doc) = open(&path);
    let after = stored(&path);
    assert!(after.checkpoint_bytes + after.update_bytes < 64 * 1024, "{after:?}");
    assert!(stale(&doc, &opened), "no history is kept");
    assert_eq!(title(&doc), "Saved");
}

#[test]
fn a_small_document_keeps_its_history_when_closed() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    let first = doc.version();
    for i in 0..300 {
        set_title(&mut doc, &format!("Edit {i}"));
        save(&store, &mut doc);
    }
    assert_eq!(close(store, &mut doc), None);
    assert!(!stale(&open(&path).1, &first));
}

// Failure: a session that never closes (a window left open for weeks) grew without bound.
// Oracle: past the session limit the checkpoint trims while open, and edits continue.
#[test]
fn a_session_past_its_limit_trims_while_open() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    let mut seed = 5;
    set_title(&mut doc, &noise(&mut seed, 1024 * 1024));
    let early = doc.version();
    // More than the storage limit's worth of edits keeps saving, and stays within it.
    for _ in 0..(STORAGE_BYTES >> 20) + 8 {
        set_title(&mut doc, &noise(&mut seed, 1024 * 1024));
        save(&store, &mut doc);
        let now = stored(&path);
        assert!(now.checkpoint_bytes + now.update_bytes <= STORAGE_BYTES as i64);
    }
    store.close().unwrap();
    assert!(stale(&open(&path).1, &early), "the trimmed history stays trimmed");
}

#[test]
fn undo_survives_compaction() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Before");
    set_title(&mut doc, "After");
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    assert!(doc.undo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "Before");
    save(&store, &mut doc);
    let snapshot = Store::open(&path, Mode::Snapshot).unwrap();
    assert_eq!(title(&snapshot.document().unwrap()), "Before");
    assert!(doc.redo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "After");
    close(store, &mut doc);
    assert_eq!(title(&open(&path).1), "After");
}

// Restoring a row deleted before a checkpoint must produce self-contained updates,
// not references to the history that compaction removed from the saved document.
#[test]
fn restoring_a_deleted_row_after_compaction_survives_reopen() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"id":"row","value":{"text":"Saved row"}}]}).to_string(), Origin::Page).unwrap();
    save(&store, &mut doc);
    let with_row = doc.value().unwrap();
    doc.apply_batch(&json!({"intents":[{"type":"remove","path":["rows"],"id":"row"}]}).to_string(), Origin::Page).unwrap();
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    assert!(doc.undo().unwrap().publication.is_some());
    save(&store, &mut doc);
    let snapshot = Store::open(&path, Mode::Snapshot).unwrap();
    assert_eq!(snapshot.document().unwrap().value().unwrap(), with_row);
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["rows",{"id":"row"},"text"],"value":"Edited after restoring"}]}).to_string(), Origin::Page).unwrap();
    let expected = doc.value().unwrap();
    close(store, &mut doc);
    assert_eq!(open(&path).1.value().unwrap(), expected);
}

// A redo restores a version from before the compaction; the live document still holds
// it, and the restore must save as updates the trimmed checkpoint can replay.
#[test]
fn redo_across_compaction_survives_reopen() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Before");
    set_title(&mut doc, "After");
    assert!(doc.undo().unwrap().publication.is_some());
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    assert!(doc.redo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "After");
    close(store, &mut doc);
    assert_eq!(title(&open(&path).1), "After");
}

// A compaction whose write fails leaves the saved state as it was; undo and the retried
// save must still reopen to what the window showed.
#[test]
fn undo_after_a_failed_compaction_saves_on_retry() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Before");
    save(&store, &mut doc);
    set_title(&mut doc, "After");
    let job = store.job(&mut doc, true).unwrap().unwrap();
    let other = hold(&path);
    assert!(matches!(store.write(&job), Err(Error::Busy)));
    other.execute_batch("COMMIT").unwrap();
    assert!(doc.undo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "Before");
    save(&store, &mut doc);
    store.close().unwrap();
    assert_eq!(title(&open(&path).1), "Before");
}

// Failure: a concurrent text edit branched from a version a checkpoint had just trimmed,
// and saved an update that depends on it; the document could never be opened again.
#[test]
fn a_stale_text_base_cannot_make_the_document_unopenable() {
    let schema = r#"{"kind":"object","properties":{"title":{"kind":"text"}}}"#;
    let initial = r#"{"title":"abc"}"#;
    let (_dir, path) = document_with(schema, initial);
    let store = Store::open(&path, Mode::Document).unwrap();
    let mut doc = store.document().unwrap();
    let base = doc.version();
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["title"],"value":"Rabc"}]}).to_string(), Origin::Page).unwrap();
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let result = type_text(&mut doc, &base, json!(["title"]), "abc", "abcX", 4).map(|_| ());
    save(&store, &mut doc);
    store.close().unwrap();
    let store = Store::open(&path, Mode::Document).unwrap();
    let reopened = store.document().expect("the document opens");
    assert_eq!(result.unwrap_err().code.as_str(), "stale_base");
    let value: Value = serde_json::from_str(&reopened.value().unwrap()).unwrap();
    assert_eq!(value["title"], "Rabc");
}

// Failure: a drawing app that saves large strokes and erases them reached the storage
// limit in weeks while its live value stayed tiny. Oracle: after each session closes,
// stored bytes stay within one session's growth and the live state.
#[test]
fn doodle_like_use_stays_bounded() {
    let (_dir, path) = document();
    let mut seed = 11;
    let bound = 4 * 1024 * 1024 + 2 * 26 * 32 * 1024;
    for day in 0..8 {
        let (store, mut doc) = open(&path);
        for stroke in 0..40 {
            let applied = doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"value":{"text":noise(&mut seed, 32 * 1024)}}]}).to_string(), Origin::Page).unwrap();
            save(&store, &mut doc);
            doc.apply_batch(&json!({"intents":[{"type":"set","path":["rows",{"id":applied.ids[0]},"text"],"value":noise(&mut seed, 32 * 1024)}]}).to_string(), Origin::Page).unwrap();
            save(&store, &mut doc);
            if stroke % 25 == 24 {
                let ids: Vec<Value> = serde_json::from_str::<Value>(&doc.value().unwrap()).unwrap()["rows"]
                    .as_array().unwrap().iter().map(|row| json!({"type":"remove","path":["rows"],"id":row["$id"]})).collect();
                doc.apply_batch(&json!({"intents":ids}).to_string(), Origin::Page).unwrap();
            }
        }
        let value = doc.value().unwrap();
        close(store, &mut doc);
        let after = stored(&path);
        let bytes = after.checkpoint_bytes + after.update_bytes;
        assert!(bytes <= bound, "day {day}: {bytes} bytes stored");
        assert_eq!(open(&path).1.value().unwrap(), value);
    }
}

#[test]
fn snapshots_never_lock_or_modify_the_document() {
    let (_dir, path) = document();
    // A snapshot reads the initial checkpoint without taking ownership.
    let before = std::fs::read(&path).unwrap();
    let snapshot = Store::open(&path, Mode::Snapshot).unwrap();
    let mut doc = snapshot.document().unwrap();
    assert_eq!(title(&doc), "Saved");
    assert_eq!(std::fs::read(&path).unwrap(), before, "a render writes nothing");
    set_title(&mut doc, "In memory");
    let job = snapshot.job(&mut doc, false).unwrap().unwrap();
    assert!(matches!(snapshot.write(&job), Err(Error::Closed)), "snapshots own nothing");
    set_accent(&mut doc, "#123456");
    assert!(matches!(snapshot.write(&snapshot.job(&mut doc, false).unwrap().unwrap()), Err(Error::Closed)));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(hitslop_core::registry::Lease::acquire(&path).expect("a snapshot holds no lock"));
}

#[test]
fn a_symlinked_document_is_refused() {
    let (dir, path) = document();
    let (store, _) = open(&path);
    store.close().unwrap();
    let real = dir.path().join("Real.slop");
    std::fs::rename(&path, &real).unwrap();
    std::os::unix::fs::symlink(&real, &path).unwrap();
    assert!(Store::open(&path, Mode::Document).is_err());
    assert!(Store::open(&path, Mode::Snapshot).is_err());
}

#[test]
fn oversized_documents_are_refused_before_any_blob_is_read() {
    let (_dir, path) = document();
    let (store, _) = open(&path);
    store.close().unwrap();
    sql(&path)
        .execute_batch(&format!("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<{}) INSERT INTO updates(bytes) SELECT x'00' FROM n;", STORAGE_ROWS + 1))
        .unwrap();
    for mode in [Mode::Document, Mode::Snapshot] {
        let error = Store::open(&path, mode).unwrap().document().err().expect("refused");
        assert!(error.to_string().contains("exceeds storage limits"), "{error}");
    }
}

#[test]
fn a_busy_database_fails_the_save_and_a_retry_succeeds() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Retried");
    let other = hold(&path);
    let job = store.job(&mut doc, false).unwrap().unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(store.write(&job), Err(Error::Busy)));
    assert!(started.elapsed() >= std::time::Duration::from_secs(1), "waits for the busy timeout");
    other.execute_batch("COMMIT").unwrap();
    store.write(&store.job(&mut doc, false).unwrap().unwrap()).unwrap();
    store.close().unwrap();
    assert_eq!(title(&open(&path).1), "Retried");
}

// A refused write never advances the saved version: the next job saves the same edits
// again, as an append or as a checkpoint.
#[test]
fn a_failed_write_never_advances_the_saved_version() {
    for checkpoint in [false, true] {
        let (_dir, path) = document();
        let (store, mut doc) = open(&path);
        set_title(&mut doc, "Retried");
        let job = store.job(&mut doc, checkpoint).unwrap().unwrap();
        let other = hold(&path);
        assert!(store.write(&job).is_err());
        other.execute_batch("COMMIT").unwrap();
        let retry = store.job(&mut doc, false).unwrap().expect("the refused edit is saved again");
        store.write(&retry).unwrap();
        store.close().unwrap();
        assert_eq!(title(&open(&path).1), "Retried", "checkpoint: {checkpoint}");
    }
}

#[test]
fn a_moved_or_linked_document_refuses_writes() {
    let (dir, path) = document();
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Unsaved");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    std::fs::rename(&path, dir.path().join("Moved.slop")).unwrap();
    std::fs::write(&path, b"").unwrap();
    assert!(matches!(store.write(&job), Err(Error::Moved)));
    assert!(matches!(store.check(false), Err(Error::Moved)));
    // A second hard link would split the journal's name: the writer stops, and no writer
    // takes a linked file.
    let (dir, path) = document();
    let (store, _) = open(&path);
    std::fs::hard_link(&path, dir.path().join("Link.slop")).unwrap();
    assert!(matches!(store.check(false), Err(Error::Moved)));
    store.close().unwrap();
    assert!(matches!(Lease::acquire(&path), Err(Error::Rejected(e)) if e.code == hitslop_core::Code::InvalidRequest));
}

// Failure: the platform SQLite stops a connection for good once its file is renamed, and
// the reload that discards unsaved edits read on that connection, so a document moved
// away and back could save but never be reloaded.
#[test]
fn a_document_moved_away_and_back_saves_and_reloads() {
    let (dir, path) = document();
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Kept");
    let moved = dir.path().join("Moved.slop");
    std::fs::rename(&path, &moved).unwrap();
    std::fs::rename(&moved, &path).unwrap();
    let job = store.job(&mut doc, false).unwrap().unwrap();
    store.write(&job).unwrap();
    set_title(&mut doc, "Discarded");
    std::fs::rename(&path, &moved).unwrap();
    std::fs::rename(&moved, &path).unwrap();
    let mut doc = store.document().unwrap();
    assert_eq!(title(&doc), "Kept");
    // A hard link added and removed again leaves the writer saving.
    std::fs::hard_link(&path, &moved).unwrap();
    std::fs::remove_file(&moved).unwrap();
    set_title(&mut doc, "Unlinked");
    save(&store, &mut doc);
    store.put_attachment(b"after the link").unwrap();
    store.close().unwrap();
    assert_eq!(title(&open(&path).1), "Unlinked");
}

#[test]
fn a_full_document_refuses_appends_and_keeps_saved_state() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    // Pad the log past the byte limit from outside, as an old save would have.
    let room = STORAGE_BYTES as i64 - stored(&path).checkpoint_bytes;
    sql(&path).execute("INSERT INTO updates(bytes) VALUES(zeroblob(?))", [room - 8]).unwrap();
    set_title(&mut doc, "Too much");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    let result = store.write(&job);
    assert!(matches!(result, Err(Error::Full)), "{result:?}");
    assert!(stored(&path).update_bytes > 0, "saved state is intact");
    // Sizes were re-read, so the next save checkpoints, which fits.
    let retry = store.job(&mut doc, false).unwrap().unwrap();
    assert!(retry.is_checkpoint());
    store.write(&retry).unwrap();
}

#[test]
fn checkpoints_reclaim_free_pages() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    // Large appends grow the file; the checkpoint that replaces them frees their pages.
    let text = "x".repeat(64 * 1024);
    for i in 0..48 {
        set_title(&mut doc, &format!("{i}{text}"));
        save(&store, &mut doc);
    }
    assert!(std::fs::metadata(&path).unwrap().len() > 3 * 1024 * 1024);
    for _ in 0..48 {
        doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"value":{"text":"row"}}]}).to_string(), Origin::Page).unwrap();
        save(&store, &mut doc);
    }
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let logical = stored(&path).checkpoint_bytes as u64;
    let file = std::fs::metadata(&path).unwrap().len();
    assert!(file < logical + 256 * 1024, "file {file} bytes for {logical} logical bytes");
}

#[test]
fn a_copy_has_the_same_history_and_theme() {
    let (dir, path) = document();
    let (store, mut doc) = open(&path);
    set_title(&mut doc, "Copied");
    set_accent(&mut doc, "#123456");
    save(&store, &mut doc);
    let copy = dir.path().join("Copy.slop");
    // The source stays open: the copy is an online backup through its writer.
    store.copy_to(&copy, true).unwrap();
    let (_copied_store, copied) = open(&copy);
    assert_eq!(title(&copied), "Copied");
    assert_eq!(accent(&copied), "#123456");
    assert_eq!(copied.version(), doc.version());
}

fn accent(doc: &Document) -> String {
    let theme = doc.theme_state().unwrap();
    serde_json::from_str::<Value>(&theme.effective).unwrap()["accent"].as_str().unwrap().into()
}
fn saved_accent(path: &Path) -> String {
    let snapshot = Store::open(path, Mode::Snapshot).unwrap();
    accent(&snapshot.document().unwrap())
}
/// Applies one palette intent from the window; whether it changed the document.
fn palette(doc: &mut Document, intent: Value) -> Result<bool, hitslop_core::Error> {
    doc.apply_batch(&json!({ "intents": [intent] }).to_string(), hitslop_core::Origin::Window).map(|a| a.publication.is_some())
}
fn set_accent(doc: &mut Document, color: &str) -> bool {
    palette(doc, json!({"type":"setTheme","values":{ "accent": color }})).unwrap()
}

#[test]
fn a_theme_change_is_saved_by_the_next_job() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    assert_eq!(accent(&doc), "#335577");
    assert!(set_accent(&mut doc, "#111111"));
    // Accepted in memory like an edit; durable once a job writes it.
    assert_eq!(accent(&doc), "#111111");
    assert_eq!(saved_accent(&path), "#335577");
    let before = stored(&path);
    assert_eq!(save(&store, &mut doc), Some(false));
    assert_eq!(stored(&path).rows, before.rows + 1, "theme changes share the Loro update log");
    assert_eq!(saved_accent(&path), "#111111");
    // A theme change and an edit are saved in one transaction.
    set_accent(&mut doc, "#222222");
    set_title(&mut doc, "Both");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    let other = hold(&path);
    assert!(store.write(&job).is_err());
    other.execute_batch("COMMIT").unwrap();
    assert_eq!(saved_accent(&path), "#111111");
    save(&store, &mut doc);
    assert_eq!(saved_accent(&path), "#222222");
    let snapshot = Store::open(&path, Mode::Snapshot).unwrap();
    assert_eq!(title(&snapshot.document().unwrap()), "Both");
    assert!(palette(&mut doc, json!({"type":"setTheme","values":{"unknown":"#000000"}})).is_err());
    store.close().unwrap();
    assert_eq!(accent(&open(&path).1), "#222222");
}

// Failure: a theme change checked ownership under the mutex a save holds for its whole
// transaction, so one slow save stalled the theme change and every edit queued behind it
// on the owner's edit queue.
#[test]
fn a_theme_change_never_waits_for_a_save_in_progress() {
    use std::time::{Duration, Instant};
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    let store = Arc::new(store);
    set_title(&mut doc, "Slow");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    // The save waits on another connection's lock, inside its transaction.
    let other = hold(&path);
    let writer = { let store = store.clone(); std::thread::spawn(move || store.write(&job)) };
    std::thread::sleep(Duration::from_millis(200));
    let began = Instant::now();
    assert!(set_accent(&mut doc, "#808080"));
    assert!(began.elapsed() < Duration::from_secs(1), "the theme change waited for the save");
    assert!(!writer.is_finished(), "the save was still in progress");
    other.execute_batch("COMMIT").unwrap();
    writer.join().unwrap().unwrap();
    assert_eq!(save(&store, &mut doc), Some(false));
    assert_eq!(saved_accent(&path), "#808080");
}

#[test]
fn a_failed_theme_save_keeps_the_change_for_a_retry() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    set_accent(&mut doc, "#303030");
    let other = hold(&path);
    let job = store.job(&mut doc, false).unwrap().expect("a theme-only job");
    assert!(matches!(store.write(&job), Err(Error::Busy)));
    other.execute_batch("COMMIT").unwrap();
    assert_eq!(accent(&doc), "#303030");
    assert_eq!(saved_accent(&path), "#335577");
    assert_eq!(save(&store, &mut doc), Some(false));
    assert_eq!(saved_accent(&path), "#303030");
}

// Failure: a theme command that changed nothing still wrote the document row, so it
// could fail on a busy database (and rewrote saved state for nothing).
#[test]
fn an_unchanged_theme_is_not_written() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    assert!(!palette(&mut doc, json!({"type":"setTheme","values":{},"replace":true})).unwrap());
    assert!(!set_accent(&mut doc, "#335577"), "setting the default changes nothing");
    assert_eq!(save(&store, &mut doc), None);
}

#[test]
fn reloading_after_a_discard_drops_unsaved_theme_changes() {
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    set_accent(&mut doc, "#404040");
    save(&store, &mut doc);
    set_accent(&mut doc, "#505050");
    let mut reloaded = store.document().unwrap();
    assert_eq!(accent(&reloaded), "#404040");
    assert_eq!(save(&store, &mut reloaded), None, "nothing is left to save");
}

#[test]
fn a_theme_file_imports_only_into_its_template() {
    let (_dir, path) = document();
    let (_store, mut doc) = open(&path);
    set_accent(&mut doc, "#606060");
    let other = json!({"template":"habit-heatmap","values":{"accent":"#000000"}}).to_string();
    assert!(palette(&mut doc, json!({"type":"importTheme","file":other})).is_err());
    let missing = r##"{"template":"checklist","values":{"missing":"#000000"}}"##;
    assert!(palette(&mut doc, json!({"type":"importTheme","file":missing})).is_err());
    assert_eq!(accent(&doc), "#606060");
    let file = doc.export_theme().unwrap();
    assert_eq!(serde_json::from_str::<Value>(&file).unwrap()["template"], "checklist", "named by the document's template");
    set_accent(&mut doc, "#707070");
    assert!(palette(&mut doc, json!({"type":"importTheme","file":file})).unwrap());
    assert_eq!(accent(&doc), "#606060");
}

// Failure: a read-only connection beside the writer in the same process (a copy of an
// open document, a snapshot render) intermittently made the platform SQLite fail the
// writer's locks with EBADF (SQLITE_IOERR_LOCK), and sometimes the reader's too.
#[test]
fn readers_beside_an_open_document_never_fail_its_saves() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let (_dir, path) = document();
    let (store, mut doc) = open(&path);
    let stop = Arc::new(AtomicBool::new(false));
    let reader = {
        let (path, stop) = (path.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut failures = vec![];
            while !stop.load(Ordering::Relaxed) {
                if let Err(e) = file::open(&path, true) {
                    failures.push(format!("open: {e}"));
                }
                if let Err(e) = Store::open(&path, Mode::Snapshot).and_then(|s| s.document().map(|_| ())) {
                    failures.push(format!("snapshot: {e}"));
                }
            }
            failures
        })
    };
    let mut failures = vec![];
    for i in 0..300 {
        set_title(&mut doc, &format!("Title {i}"));
        let job = store.job(&mut doc, false).unwrap().unwrap();
        if let Err(e) = store.write(&job) {
            failures.push(format!("save {i}: {e}"));
        }
    }
    stop.store(true, Ordering::Relaxed);
    failures.extend(reader.join().unwrap());
    assert!(failures.is_empty(), "{} failures, first: {:?}", failures.len(), failures.first());
}

// Crash outcomes at each kind of save, with real processes: what a save commits survives
// the death of the process that wrote it, and its lock dies with it. The rollback of a
// write interrupted mid-commit is SQLite's journal (`a_crash_mid_commit_is_recovered_by_the_next_writer`
// in `file.rs`); the timed helper kills in `tests/native/crash.native.test.ts` are stress coverage.
const SAVES: [&str; 3] = ["append", "checkpoint", "theme"];

/// Runs only as the child of `a_committed_save_survives_its_process_being_killed`: makes
/// one save of the kind `HITSLOP_CHILD_SAVE` names, then dies without closing.
#[test]
#[ignore]
fn save_then_die() {
    let (Ok(path), Ok(save)) = (std::env::var("HITSLOP_CHILD_DOCUMENT"), std::env::var("HITSLOP_CHILD_SAVE")) else { return };
    isolate_registry();
    let (store, mut doc) = open(Path::new(&path));
    let job = match save.as_str() {
        "theme" => {
            assert!(set_accent(&mut doc, "#123456"));
            store.job(&mut doc, false).unwrap().unwrap()
        }
        kind => {
            set_title(&mut doc, &format!("Saved by {kind}"));
            let job = store.job(&mut doc, kind == "checkpoint").unwrap().unwrap();
            assert_eq!(job.is_checkpoint(), kind == "checkpoint");
            job
        }
    };
    store.write(&job).unwrap();
    std::process::abort();
}

#[test]
fn a_committed_save_survives_its_process_being_killed() {
    for save in SAVES {
        let (_dir, path) = document();
        let (store, _) = open(&path);
        store.close().unwrap();
        let status = support::child("save_then_die", &[("HITSLOP_CHILD_DOCUMENT", path.to_str().unwrap()), ("HITSLOP_CHILD_SAVE", save)])
            .status()
            .unwrap();
        // A signal, not a failed assertion: the child reached its save.
        assert_eq!(std::os::unix::process::ExitStatusExt::signal(&status), Some(6), "{save}: {status:?}");
        // Opening as the writer proves the lock died with the child.
        let (store, doc) = open(&path);
        match save {
            "theme" => assert_eq!(accent(&doc), "#123456"),
            kind => assert_eq!(title(&doc), format!("Saved by {kind}")),
        }
        store.close().unwrap();
    }
}

/// Runs only as the child of `a_save_killed_before_its_commit_leaves_the_saved_state`:
/// starts a save while another connection holds the database, after marking that it did.
#[test]
#[ignore]
fn save_while_held() {
    let (Ok(path), Ok(marker)) = (std::env::var("HITSLOP_CHILD_DOCUMENT"), std::env::var("HITSLOP_CHILD_MARKER")) else { return };
    isolate_registry();
    let (store, mut doc) = open(Path::new(&path));
    set_title(&mut doc, "Never saved");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    std::fs::write(&marker, b"").unwrap();
    // Waits for the held database until the parent kills this process.
    let _ = store.write(&job);
}

#[test]
fn a_save_killed_before_its_commit_leaves_the_saved_state() {
    let (dir, path) = document();
    let (store, _) = open(&path);
    store.close().unwrap();
    let held = sql(&path);
    held.execute_batch("BEGIN IMMEDIATE").unwrap();
    let marker = dir.path().join("saving");
    let mut child = support::child("save_while_held", &[("HITSLOP_CHILD_DOCUMENT", path.to_str().unwrap()), ("HITSLOP_CHILD_MARKER", marker.to_str().unwrap())])
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !marker.exists() {
        assert!(std::time::Instant::now() < deadline && child.try_wait().unwrap().is_none(), "the child never started its save");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Its write now waits for the database (the busy timeout is 2 s).
    std::thread::sleep(std::time::Duration::from_millis(300));
    child.kill().unwrap();
    child.wait().unwrap();
    held.execute_batch("COMMIT").unwrap();
    let (store, doc) = open(&path);
    assert_eq!(title(&doc), "Saved");
    store.close().unwrap();
}

// Failure (the lost reply): a save that committed but whose acknowledgement was lost is
// saved again, because the saved version never advanced. Oracle: the counter after reopen
// counts the increment once, for an append and for a checkpoint.
#[test]
fn a_save_retried_after_a_lost_reply_counts_once() {
    let (_dir, path) = document_with(r#"{"kind":"object","properties":{"hits":{"kind":"counter"}}}"#, r#"{"hits":0}"#);
    let mut expected = 0;
    for checkpoint in [false, true] {
        let (store, mut doc) = open(&path);
        doc.apply_batch(r#"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#, Origin::Page).unwrap();
        expected += 3;
        let job = store.job(&mut doc, checkpoint).unwrap().unwrap();
        assert_eq!(job.is_checkpoint(), checkpoint);
        store.write(&job).unwrap();
        store.write(&job).unwrap();
        if !checkpoint {
            assert_eq!(stored(&path).rows, 2, "the retry saved the same updates again");
        }
        store.close().unwrap();
        let (store, doc) = open(&path);
        let value: Value = serde_json::from_str(&doc.value().unwrap()).unwrap();
        assert_eq!(value["hits"], expected, "checkpoint: {checkpoint}");
        store.close().unwrap();
    }
}
