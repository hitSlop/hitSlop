//! Durable storage: identity, limits, crash outcomes and the writer lock.
use hitslop_core::store::{self, Error, Mode, Phases, Store};
use hitslop_core::theme::Change;
use hitslop_core::{STORAGE_BYTES, STORAGE_ROWS};
use hitslop_core::Document;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use hitslop_core::Origin;

const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"string"},"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"string"}}}}}}"#;
const INITIAL: &str = r#"{"title":"Saved","rows":[]}"#;
const THEME: &str = r##"{"accent":"#335577"}"##;

fn key() -> String {
    hitslop_core::validate(SCHEMA, INITIAL).unwrap()
}
fn package() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Doc.slop");
    std::fs::create_dir(&root).unwrap();
    (dir, root)
}
fn database(root: &Path) -> PathBuf {
    root.join("state/document.sqlite")
}
fn open(root: &Path) -> (Store, Document) {
    let store = Store::open(root, Mode::Document).unwrap();
    let doc = store.document(&key(), INITIAL, THEME).unwrap();
    (store, doc)
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
struct Fail(&'static str);
impl Phases for Fail {
    fn reached(&self, phase: &str) -> Result<(), Error> {
        if phase == self.0 { Err(Error::Failed(format!("injected at {phase}"))) } else { Ok(()) }
    }
}

fn saved_updates(root: &Path) -> Vec<(i64, Vec<u8>)> {
    let conn = Connection::open(database(root)).unwrap();
    let mut statement = conn.prepare("SELECT seq,bytes FROM updates ORDER BY seq").unwrap();
    statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap().collect::<rusqlite::Result<_>>().unwrap()
}

// Failure: an absent checkpoint was treated as initial state even with saved updates,
// and opening then deleted those updates. Every reader must preserve the damaged file.
fn missing_checkpoint_is_preserved(access: impl FnOnce(&Path) -> Result<(), Error>) {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    for title in ["First", "Second", "Third"] {
        set_title(&mut doc, title);
        save(&store, &mut doc);
    }
    store.close().unwrap();
    Connection::open(database(&root)).unwrap().execute("DELETE FROM checkpoint", []).unwrap();
    let before = saved_updates(&root);
    assert_eq!(before.len(), 3);
    let result = access(&root);
    assert!(matches!(result, Err(Error::Failed(ref message)) if message.contains("preserve the package for recovery")), "{result:?}");
    assert_eq!(saved_updates(&root), before);
    let checkpoints: i64 = Connection::open(database(&root)).unwrap()
        .query_row("SELECT count(*) FROM checkpoint", [], |row| row.get(0)).unwrap();
    assert_eq!(checkpoints, 0, "initial state must not be written");
}

#[test]
fn writable_open_refuses_saved_updates_without_a_checkpoint() {
    missing_checkpoint_is_preserved(|root| {
        Store::open(root, Mode::Document)?.document(&key(), INITIAL, THEME).map(|_| ())
    });
}

#[test]
fn snapshot_refuses_saved_updates_without_a_checkpoint() {
    missing_checkpoint_is_preserved(|root| Store::open(root, Mode::Snapshot).map(|_| ()));
}

#[test]
fn duplicate_refuses_saved_updates_without_a_checkpoint() {
    missing_checkpoint_is_preserved(|root| {
        let copy = root.with_file_name("Copy.slop");
        std::fs::create_dir(&copy).unwrap();
        let result = store::duplicate(root, &copy);
        assert!(!database(&copy).exists(), "a refused source must not create a database");
        result
    });
}

#[test]
fn append_refuses_saved_updates_without_a_checkpoint() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Saved edit");
    save(&store, &mut doc);
    set_title(&mut doc, "Pending edit");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    assert!(!job.is_checkpoint());
    Connection::open(database(&root)).unwrap().execute("DELETE FROM checkpoint", []).unwrap();
    let before = saved_updates(&root);
    let result = store.write(&job);
    assert!(matches!(result, Err(Error::Failed(ref message)) if message.contains("preserve the package for recovery")), "{result:?}");
    assert_eq!(saved_updates(&root), before);
    assert_eq!(title(&doc), "Pending edit");
}

#[test]
fn a_new_package_saves_its_first_checkpoint_and_reopens_with_its_edits() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    let id = store.doc_id().to_owned();
    assert_eq!(store.metadata().unwrap().rows, 0);
    assert!(store.metadata().unwrap().checkpoint_bytes > 0);
    assert!(store.job(&mut doc, false).unwrap().is_none(), "a clean document has nothing to save");
    set_title(&mut doc, "Edited");
    assert_eq!(save(&store, &mut doc), Some(false), "an edit appends");
    assert_eq!(store.metadata().unwrap().rows, 1);
    assert!(store.job(&mut doc, false).unwrap().is_none());
    store.close().unwrap();
    let (reopened, doc) = open(&root);
    assert_eq!(title(&doc), "Edited");
    assert_eq!(reopened.doc_id(), id);
}

#[test]
fn a_long_log_checkpoints_and_compaction_is_always_a_checkpoint() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    let first = doc.version();
    for i in 0..256 {
        set_title(&mut doc, &format!("Edit {i}"));
        assert_eq!(save(&store, &mut doc), Some(false));
    }
    set_title(&mut doc, "Last");
    assert_eq!(save(&store, &mut doc), Some(true), "256 rows checkpoint");
    assert_eq!(store.metadata().unwrap().rows, 0);
    store.close().unwrap();
    let (store, mut doc) = open(&root);
    assert!(!stale(&doc, &first), "a small checkpoint keeps its whole history");
    let job = store.job(&mut doc, true).unwrap().expect("a requested checkpoint is written even when clean");
    assert!(job.is_checkpoint());
    store.write(&job).unwrap();
    store.close().unwrap();
    assert_eq!(title(&open(&root).1), "Last");
}

fn stale(doc: &Document, version: &str) -> bool {
    match doc.export_since(version) {
        Ok(_) => false,
        Err(e) => e.code.as_str() == "stale_base",
    }
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
fn session(root: &Path, seed: &mut u64, count: usize) -> String {
    let (store, mut doc) = open(root);
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
// change it ever saw. Oracle: after a session closes, versions from before it are stale
// and its own still name the document's history; a session that only reads keeps the
// previous session's history.
#[test]
fn closing_keeps_the_last_session_and_compaction_trims_to_now() {
    let (_dir, root) = package();
    let mut seed = 7;
    let first = session(&root, &mut seed, 100);
    let second = session(&root, &mut seed, 100);
    let (store, mut doc) = open(&root);
    assert!(stale(&doc, &first), "history before the last session is trimmed");
    assert!(!stale(&doc, &second), "the last session's history is kept");
    let before = store.metadata().unwrap();
    assert_eq!(close(store, &mut doc), None, "a session that only reads trims nothing");
    let (store, mut doc) = open(&root);
    assert_eq!(store.metadata().unwrap(), before);
    assert!(!stale(&doc, &second));
    set_title(&mut doc, "Compacted");
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let latest = doc.version();
    store.close().unwrap();
    let doc = open(&root).1;
    assert!(stale(&doc, &second), "compaction keeps no history");
    assert!(!stale(&doc, &latest));
    assert_eq!(title(&doc), "Compacted");
}

// Failure: a cut at the session's start kept every row deleted before it, so a document
// that deletes a lot never shrank. Oracle: it closes to its live value with no history.
#[test]
fn a_session_too_large_to_keep_closes_with_no_history() {
    let (_dir, root) = package();
    let mut seed = 3;
    let mut opened = String::new();
    for _ in 0..2 {
        let (store, mut doc) = open(&root);
        opened = doc.version();
        for _ in 0..160 {
            let applied = doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"value":{"text":noise(&mut seed, 32 * 1024)}}]}).to_string(), Origin::Page).unwrap();
            doc.apply_batch(&json!({"intents":[{"type":"remove","path":["rows"],"id":applied.ids[0]}]}).to_string(), Origin::Page).unwrap();
            save(&store, &mut doc);
        }
        close(store, &mut doc);
    }
    let (store, doc) = open(&root);
    let meta = store.metadata().unwrap();
    assert!(meta.checkpoint_bytes + meta.update_bytes < 64 * 1024, "{meta:?}");
    assert!(stale(&doc, &opened), "no history is kept");
    assert_eq!(title(&doc), "Saved");
}

#[test]
fn a_small_document_keeps_its_history_when_closed() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    let first = doc.version();
    for i in 0..300 {
        set_title(&mut doc, &format!("Edit {i}"));
        save(&store, &mut doc);
    }
    assert_eq!(close(store, &mut doc), None);
    assert!(!stale(&open(&root).1, &first));
}

// Failure: a session that never closes (a window left open for weeks) grew without bound.
// Oracle: past the session limit the checkpoint trims while open, and edits continue.
#[test]
fn a_session_past_its_limit_trims_while_open() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    let mut seed = 5;
    set_title(&mut doc, &noise(&mut seed, 1024 * 1024));
    let early = doc.version();
    for _ in 0..24 {
        set_title(&mut doc, &noise(&mut seed, 1024 * 1024));
        save(&store, &mut doc);
        let meta = store.metadata().unwrap();
        assert!(meta.checkpoint_bytes + meta.update_bytes <= 16 * 1024 * 1024 + 4 * 1024 * 1024 + 2 * 1024 * 1024);
    }
    store.close().unwrap();
    assert!(stale(&open(&root).1, &early), "the trimmed history stays trimmed");
}

// Kept history decides how far undo reaches into an agent's closed edits: a closing
// session that is itself too large keeps no history, so only later commands remain.
#[test]
fn undo_of_closed_agent_edits_reaches_as_far_as_history_is_kept() {
    let (_dir, root) = package();
    let mut seed = 9;
    let agent = |doc: &mut Document, intents: Value| doc.apply_batch(&json!({ "intents": intents }).to_string(), Origin::Agent).unwrap();
    let (store, mut doc) = open(&root);
    for _ in 0..160 {
        let applied = agent(&mut doc, json!([{"type":"insert","path":["rows"],"value":{"text":noise(&mut seed, 32 * 1024)}}]));
        agent(&mut doc, json!([{"type":"remove","path":["rows"],"id":applied.ids[0]}]));
        save(&store, &mut doc);
    }
    agent(&mut doc, json!([{"type":"set","path":["title"],"value":"First"}]));
    close(store, &mut doc);
    let (store, mut doc) = open(&root);
    agent(&mut doc, json!([{"type":"set","path":["title"],"value":"Second"}]));
    close(store, &mut doc);
    let (_store, mut doc) = open(&root);
    assert!(doc.can_undo());
    doc.undo().unwrap();
    assert_eq!(title(&doc), "First", "the first command's history was not kept");
    assert!(!doc.can_undo());
}

#[test]
fn undo_survives_compaction() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Before");
    set_title(&mut doc, "After");
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    assert!(doc.undo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "Before");
    save(&store, &mut doc);
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    assert_eq!(title(&snapshot.document(&key(), INITIAL, THEME).unwrap()), "Before");
    assert!(doc.redo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "After");
    close(store, &mut doc);
    assert_eq!(title(&open(&root).1), "After");
}

// Restoring a row deleted before a checkpoint must produce self-contained updates,
// not references to the history that compaction removed from the saved document.
#[test]
fn restoring_a_deleted_row_after_compaction_survives_reopen() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"id":"row","value":{"text":"Saved row"}}]}).to_string(), Origin::Page).unwrap();
    save(&store, &mut doc);
    let with_row = doc.value().unwrap();
    doc.apply_batch(&json!({"intents":[{"type":"remove","path":["rows"],"id":"row"}]}).to_string(), Origin::Page).unwrap();
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    assert!(doc.undo().unwrap().publication.is_some());
    save(&store, &mut doc);
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    assert_eq!(snapshot.document(&key(), INITIAL, THEME).unwrap().value().unwrap(), with_row);
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["rows",{"id":"row"},"text"],"value":"Edited after restoring"}]}).to_string(), Origin::Page).unwrap();
    let expected = doc.value().unwrap();
    close(store, &mut doc);
    assert_eq!(open(&root).1.value().unwrap(), expected);
}

// Failure: a concurrent text edit branched from a version a checkpoint had just trimmed,
// and saved an update that depends on it; the package could never be opened again.
#[test]
fn a_stale_text_base_cannot_make_the_package_unopenable() {
    let schema = r#"{"kind":"object","properties":{"title":{"kind":"text"}}}"#;
    let initial = r#"{"title":"abc"}"#;
    let key = hitslop_core::validate(schema, initial).unwrap();
    let (_dir, root) = package();
    let store = Store::open(&root, Mode::Document).unwrap();
    let mut doc = store.document(&key, initial, THEME).unwrap();
    let base = doc.version();
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["title"],"value":"Rabc"}]}).to_string(), Origin::Page).unwrap();
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let request = json!({"base":base,"path":["title"],"from":"abc","to":"abcX","selectionStart":4,"selectionEnd":4});
    let result = doc.edit_text(&request.to_string()).map(|_| ());
    save(&store, &mut doc);
    store.close().unwrap();
    let store = Store::open(&root, Mode::Document).unwrap();
    let reopened = store.document(&key, initial, THEME).expect("the package opens");
    assert_eq!(result.unwrap_err().code.as_str(), "stale_base");
    let value: Value = serde_json::from_str(&reopened.value().unwrap()).unwrap();
    assert_eq!(value["title"], "Rabc");
}

// Failure: a drawing app that saves large strokes and erases them reached the storage
// limit in weeks while its live value stayed tiny. Oracle: after each session closes,
// stored bytes stay within that session's history and the live state.
#[test]
fn doodle_like_use_stays_bounded() {
    let (_dir, root) = package();
    let mut seed = 11;
    let bound = 4 * 1024 * 1024 + 2 * 26 * 32 * 1024;
    for day in 0..8 {
        let (store, mut doc) = open(&root);
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
        let (store, doc) = open(&root);
        let meta = store.metadata().unwrap();
        let stored = meta.checkpoint_bytes + meta.update_bytes;
        assert!(stored <= bound, "day {day}: {stored} bytes stored");
        assert_eq!(doc.value().unwrap(), value);
        store.close().unwrap();
    }
}

#[test]
fn the_writer_lock_admits_one_owner_and_snapshots_read_without_it() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    assert!(matches!(Store::open(&root, Mode::Document), Err(Error::Locked)));
    assert!(matches!(store::WriterLock::acquire(&root), Err(Error::Locked)));
    set_title(&mut doc, "Durable");
    save(&store, &mut doc);
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    assert_eq!(title(&snapshot.document(&key(), INITIAL, THEME).unwrap()), "Durable");
    // A failed close keeps ownership; a retry releases it.
    store.set_phases(Some(Arc::new(Fail("close"))));
    assert!(store.close().is_err());
    assert!(matches!(Store::open(&root, Mode::Document), Err(Error::Locked)));
    store.set_phases(None);
    store.close().unwrap();
    drop(store::WriterLock::acquire(&root).unwrap());
}

#[test]
fn snapshots_never_create_lock_or_modify_package_files() {
    let (_dir, root) = package();
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    let mut doc = snapshot.document(&key(), INITIAL, THEME).unwrap();
    set_title(&mut doc, "In memory");
    save(&snapshot, &mut doc);
    assert!(!root.join("state").exists(), "a render of an unsaved package writes nothing");
    assert!(matches!(snapshot.check(true), Err(Error::Closed)), "snapshots own nothing");

    let (store, _) = open(&root);
    store.close().unwrap();
    let before = std::fs::read(database(&root)).unwrap();
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    let mut doc = snapshot.document(&key(), INITIAL, THEME).unwrap();
    set_title(&mut doc, "In memory");
    save(&snapshot, &mut doc);
    assert_eq!(std::fs::read(database(&root)).unwrap(), before);
    assert!(!root.join("state/writer.lock").exists() || store::WriterLock::acquire(&root).is_ok());
}

#[test]
fn foreign_databases_are_refused_and_left_unchanged() {
    // A foreign file, a newer storage version, and the earlier layouts.
    for sql in [
        "PRAGMA application_id=1; CREATE TABLE t(x);",
        "PRAGMA application_id=1213418576; PRAGMA user_version=4; CREATE TABLE t(x);",
        "PRAGMA application_id=1213418576; PRAGMA user_version=1; CREATE TABLE document(id INTEGER PRIMARY KEY, checkpoint BLOB, schema_key TEXT, doc_id TEXT NOT NULL); CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);",
        "PRAGMA application_id=1213418576; PRAGMA user_version=2; CREATE TABLE document(id INTEGER PRIMARY KEY, checkpoint BLOB, schema_key TEXT, doc_id TEXT NOT NULL, theme TEXT NOT NULL DEFAULT '{}'); CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);",
    ] {
        let (_dir, root) = package();
        std::fs::create_dir(root.join("state")).unwrap();
        Connection::open(database(&root)).unwrap().execute_batch(sql).unwrap();
        let before = std::fs::read(database(&root)).unwrap();
        for mode in [Mode::Document, Mode::Snapshot] {
            let error = Store::open(&root, mode).err().expect("refused");
            assert!(error.to_string().contains("Unsupported document storage"), "{error}");
        }
        assert_eq!(std::fs::read(database(&root)).unwrap(), before);
    }
}

#[test]
fn a_symlinked_state_or_database_is_refused() {
    let (dir, root) = package();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join("state")).unwrap();
    assert!(Store::open(&root, Mode::Document).is_err());
    assert!(Store::open(&root, Mode::Snapshot).is_err());

    let (dir, root) = package();
    let (store, _) = open(&root);
    store.close().unwrap();
    let real = dir.path().join("real.sqlite");
    std::fs::rename(database(&root), &real).unwrap();
    std::os::unix::fs::symlink(&real, database(&root)).unwrap();
    assert!(Store::open(&root, Mode::Document).is_err());
    let copy = dir.path().join("Copy.slop");
    std::fs::create_dir(&copy).unwrap();
    assert!(store::duplicate(&root, &copy).is_err(), "a duplicate never follows a replaced database");
}

#[test]
fn oversized_documents_are_refused_before_any_blob_is_read() {
    let (_dir, root) = package();
    let (store, _) = open(&root);
    store.close().unwrap();
    let conn = Connection::open(database(&root)).unwrap();
    conn.execute_batch(&format!("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<{}) INSERT INTO updates(bytes) SELECT x'00' FROM n;", STORAGE_ROWS + 1)).unwrap();
    drop(conn);
    let store = Store::open(&root, Mode::Document).unwrap();
    let error = store.document(&key(), INITIAL, THEME).err().expect("refused");
    assert!(error.to_string().contains("exceeds storage limits"), "{error}");
}

#[test]
fn a_busy_database_fails_the_save_and_a_retry_succeeds() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Retried");
    let other = Connection::open(database(&root)).unwrap();
    other.busy_timeout(std::time::Duration::ZERO).unwrap();
    other.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let job = store.job(&mut doc, false).unwrap().unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(store.write(&job), Err(Error::Busy)));
    assert!(started.elapsed() >= std::time::Duration::from_secs(1), "waits for the busy timeout");
    other.execute_batch("COMMIT").unwrap();
    store.write(&store.job(&mut doc, false).unwrap().unwrap()).unwrap();
    store.close().unwrap();
    assert_eq!(title(&open(&root).1), "Retried");
}

#[test]
fn a_lost_acknowledgement_never_advances_the_saved_version() {
    for phase in ["append:uncommitted", "append:committed", "checkpoint:uncommitted", "checkpoint:committed"] {
        let (_dir, root) = package();
        let (store, mut doc) = open(&root);
        set_title(&mut doc, "Crash");
        store.set_phases(Some(Arc::new(Fail(phase))));
        let job = store.job(&mut doc, phase.starts_with("checkpoint")).unwrap().unwrap();
        assert!(store.write(&job).is_err());
        store.set_phases(None);
        let committed = phase.ends_with(":committed");
        let rows = if phase == "append:committed" { 1 } else { 0 };
        assert_eq!(store.metadata().unwrap().rows, rows, "{phase}: sizes are re-read after a failure");
        let retry = store.job(&mut doc, false).unwrap().expect("the unacknowledged edit is saved again");
        store.write(&retry).unwrap();
        store.close().unwrap();
        assert_eq!(title(&open(&root).1), "Crash", "{phase} (committed: {committed})");
    }
}

#[test]
fn a_moved_package_refuses_writes() {
    let (dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Unsaved");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    std::fs::rename(&root, dir.path().join("Moved.slop")).unwrap();
    std::fs::create_dir(&root).unwrap();
    assert!(matches!(store.write(&job), Err(Error::Moved)));
    assert!(matches!(store.check(false), Err(Error::Moved)));
}

#[test]
fn a_full_document_refuses_appends_and_keeps_saved_state() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    // Pad the log past the byte limit from outside, as an old save would have.
    let conn = Connection::open(database(&root)).unwrap();
    let room = STORAGE_BYTES as i64 - store.metadata().unwrap().checkpoint_bytes;
    conn.execute("INSERT INTO updates(bytes) VALUES(zeroblob(?))", [room - 8]).unwrap();
    drop(conn);
    set_title(&mut doc, "Too much");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    let result = store.write(&job);
    assert!(matches!(result, Err(Error::Full)), "{result:?}");
    assert!(store.metadata().unwrap().update_bytes > 0, "saved state is intact");
    // Sizes were re-read, so the next save checkpoints, which fits.
    let retry = store.job(&mut doc, false).unwrap().unwrap();
    assert!(retry.is_checkpoint());
    store.write(&retry).unwrap();
}

#[test]
fn checkpoints_reclaim_free_pages() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    // Large appends grow the file; the checkpoint that replaces them frees their pages.
    let text = "x".repeat(64 * 1024);
    for i in 0..48 {
        set_title(&mut doc, &format!("{i}{text}"));
        save(&store, &mut doc);
    }
    assert!(std::fs::metadata(database(&root)).unwrap().len() > 3 * 1024 * 1024);
    for _ in 0..48 {
        doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"value":{"text":"row"}}]}).to_string(), Origin::Page).unwrap();
        save(&store, &mut doc);
    }
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let logical = store.metadata().unwrap().checkpoint_bytes as u64;
    let file = std::fs::metadata(database(&root)).unwrap().len();
    assert!(file < logical + 256 * 1024, "file {file} bytes for {logical} logical bytes");
}

#[test]
fn a_duplicate_has_the_same_history_and_a_new_identity() {
    let (dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Copied");
    save(&store, &mut doc);
    let copy = dir.path().join("Copy.slop");
    std::fs::create_dir(&copy).unwrap();
    // The source stays open: the copy is an online backup.
    store::duplicate(&root, &copy).unwrap();
    let (duplicate, copied) = open(&copy);
    assert_eq!(title(&copied), "Copied");
    assert_ne!(duplicate.doc_id(), store.doc_id());
    assert_eq!(copied.version(), doc.version());
}

// Failure: a duplicate copied a database that opening refuses (a foreign, empty or
// oversized one), so Duplicate succeeded and the copy then failed to open.
#[test]
fn a_duplicate_refuses_a_source_that_would_not_open() {
    let foreign = |root: &Path| {
        std::fs::create_dir(root.join("state")).unwrap();
        Connection::open(database(root)).unwrap().execute_batch("PRAGMA application_id=1; CREATE TABLE t(x);").unwrap();
    };
    let empty = |root: &Path| {
        std::fs::create_dir(root.join("state")).unwrap();
        Connection::open(database(root)).unwrap().execute_batch("PRAGMA user_version=0;").unwrap();
    };
    let oversized = |root: &Path| {
        open(root).0.close().unwrap();
        Connection::open(database(root)).unwrap()
            .execute_batch(&format!("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<{}) INSERT INTO updates(bytes) SELECT x'00' FROM n;", STORAGE_ROWS + 1))
            .unwrap();
    };
    for prepare in [&foreign as &dyn Fn(&Path), &empty, &oversized] {
        let (dir, root) = package();
        prepare(&root);
        let copy = dir.path().join("Copy.slop");
        std::fs::create_dir(&copy).unwrap();
        assert!(store::duplicate(&root, &copy).is_err());
        assert!(!database(&copy).exists(), "no database is created for a refused source");
    }
}

fn accent(store: &Store) -> String {
    let (theme, _) = store.theme(Change::Get).unwrap();
    serde_json::from_str::<Value>(&theme.effective).unwrap()["accent"].as_str().unwrap().into()
}
fn saved_accent(root: &Path) -> String {
    let snapshot = Store::open(root, Mode::Snapshot).unwrap();
    snapshot.document(&key(), INITIAL, THEME).unwrap();
    accent(&snapshot)
}
fn set_accent(store: &Store, color: &str) -> bool {
    store.theme(Change::Set(&json!({ "accent": color }).to_string())).unwrap().1
}

#[test]
fn a_theme_change_is_saved_by_the_next_job() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    assert_eq!(accent(&store), "#335577");
    assert!(set_accent(&store, "#111111"));
    // Accepted in memory like an edit; durable once a job writes it.
    assert_eq!(accent(&store), "#111111");
    assert!(store.theme_unsaved());
    assert_eq!(saved_accent(&root), "#335577");
    let rows = store.metadata().unwrap();
    assert_eq!(save(&store, &mut doc), Some(false));
    assert!(!store.theme_unsaved());
    assert_eq!(store.metadata().unwrap(), rows, "a theme-only job writes no document bytes");
    assert_eq!(saved_accent(&root), "#111111");
    // A theme change and an edit are saved in one transaction.
    set_accent(&store, "#222222");
    set_title(&mut doc, "Both");
    store.set_phases(Some(Arc::new(Fail("append:uncommitted"))));
    assert!(store.write(&store.job(&mut doc, false).unwrap().unwrap()).is_err());
    store.set_phases(None);
    assert!(store.theme_unsaved());
    assert_eq!(saved_accent(&root), "#111111");
    save(&store, &mut doc);
    assert_eq!(saved_accent(&root), "#222222");
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    assert_eq!(title(&snapshot.document(&key(), INITIAL, THEME).unwrap()), "Both");
    assert!(matches!(snapshot.theme(Change::Reset(None)), Err(Error::Closed)));
    assert!(matches!(store.theme(Change::Set(r##"{"unknown":"#000000"}"##)), Err(Error::Rejected(_))));
    store.close().unwrap();
    assert_eq!(accent(&open(&root).0), "#222222");
}

// Failure: theme overrides kept in `state/theme.json` could be redirected through a
// swapped `state/` link, block on a FIFO, or leave a staging file that failed package
// validation. They now live in the document's database.
#[test]
fn theme_overrides_live_in_the_database() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_accent(&store, "#101010");
    save(&store, &mut doc);
    assert!(!root.join("state/theme.json").exists());
    // Whatever sits at the old path is never opened.
    let fifo = std::ffi::CString::new(root.join("state/theme.json").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(saved_accent(&root), "#101010");
}

/// Holds a write at `at` until released.
struct Pause {
    at: &'static str,
    reached: std::sync::Mutex<std::sync::mpsc::Sender<()>>,
    resume: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}
impl Phases for Pause {
    fn reached(&self, phase: &str) -> Result<(), Error> {
        if phase == self.at {
            self.reached.lock().unwrap().send(()).ok();
            self.resume.lock().unwrap().recv().ok();
        }
        Ok(())
    }
}

// Failure: a theme change checked ownership under the mutex a save holds for its whole
// transaction, so one slow save stalled the theme change and every edit queued behind it
// on the owner's edit queue.
#[test]
fn a_theme_change_never_waits_for_a_save_in_progress() {
    use std::sync::mpsc::channel;
    use std::time::Duration;
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    let store = Arc::new(store);
    set_title(&mut doc, "Slow");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    let (reached, paused) = channel();
    let (release, resume) = channel();
    store.set_phases(Some(Arc::new(Pause {
        at: "append:uncommitted",
        reached: std::sync::Mutex::new(reached),
        resume: std::sync::Mutex::new(resume),
    })));
    let writer = { let store = store.clone(); std::thread::spawn(move || store.write(&job)) };
    paused.recv_timeout(Duration::from_secs(5)).expect("the save reached its transaction");
    let (done, changed) = channel();
    let theme = {
        let store = store.clone();
        std::thread::spawn(move || done.send(store.theme(Change::Set(r##"{"accent":"#808080"}"##)).map(|(_, changed)| changed)))
    };
    let accepted = changed.recv_timeout(Duration::from_secs(1));
    release.send(()).unwrap();
    writer.join().unwrap().unwrap();
    theme.join().unwrap().ok();
    assert!(accepted.expect("the theme change waited for the save").unwrap());
    store.set_phases(None);
    assert_eq!(save(&store, &mut doc), Some(false));
    assert_eq!(saved_accent(&root), "#808080");
}

#[test]
fn a_failed_theme_save_keeps_the_change_for_a_retry() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_accent(&store, "#303030");
    let other = Connection::open(database(&root)).unwrap();
    other.busy_timeout(std::time::Duration::ZERO).unwrap();
    other.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let job = store.job(&mut doc, false).unwrap().expect("a theme-only job");
    assert!(matches!(store.write(&job), Err(Error::Busy)));
    other.execute_batch("COMMIT").unwrap();
    assert!(store.theme_unsaved());
    assert_eq!(accent(&store), "#303030");
    assert_eq!(save(&store, &mut doc), Some(false));
    assert_eq!(saved_accent(&root), "#303030");
}

// Failure: a theme command that changed nothing still wrote the document row, so it
// could fail on a busy database (and rewrote saved state for nothing).
#[test]
fn an_unchanged_theme_is_not_written() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    assert!(!store.theme(Change::Reset(None)).unwrap().1);
    assert!(!set_accent(&store, "#335577"), "setting the default changes nothing");
    assert!(!store.theme_unsaved());
    assert_eq!(save(&store, &mut doc), None);
}

#[test]
fn reloading_after_a_discard_drops_unsaved_theme_changes() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_accent(&store, "#404040");
    save(&store, &mut doc);
    set_accent(&store, "#505050");
    store.document(&key(), INITIAL, THEME).unwrap();
    assert_eq!(accent(&store), "#404040");
    assert!(!store.theme_unsaved());
}

#[test]
fn a_refused_import_leaves_the_palette_unchanged() {
    let (_dir, root) = package();
    let (store, _) = open(&root);
    set_accent(&store, "#606060");
    let other = json!({"template":"habit-heatmap","values":{"accent":"#000000"}}).to_string();
    let import = |file: &str| store.theme(Change::Import { template: "checklist", file });
    assert!(matches!(import(&other), Err(Error::Rejected(_))));
    assert!(matches!(import(r##"{"template":"checklist","values":{"missing":"#000000"}}"##), Err(Error::Rejected(_))));
    assert_eq!(accent(&store), "#606060");
    let file = store.export_theme("checklist").unwrap();
    set_accent(&store, "#707070");
    assert!(import(&file).unwrap().1);
    assert_eq!(accent(&store), "#606060");
}

#[test]
fn a_duplicate_carries_theme_overrides() {
    let (dir, root) = package();
    let (store, mut doc) = open(&root);
    set_accent(&store, "#123456");
    save(&store, &mut doc);
    let copy = dir.path().join("Copy.slop");
    std::fs::create_dir(&copy).unwrap();
    store::duplicate(&root, &copy).unwrap();
    assert_eq!(accent(&open(&copy).0), "#123456");
}
