//! Durable storage: identity, limits, crash outcomes and the writer lock, over one
//! document file.
use hitslop_core::file;
use hitslop_core::registry::Lease;
use hitslop_core::store::{Error, Mode, Phases, Store};
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

const MANIFEST: &str = r#"{"author":{"name":"Fixture"},"slug":"checklist","title":"Checklist","description":"A test document.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#;
/// A new document file, created from a template of `schema` and `initial`, alone in its
/// folder.
/// This process's registry lives in a temporary folder, so test runs never fill the
/// account's `~/.hitslop/live`.
fn isolate_registry() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let folder = std::env::temp_dir().join("hitslop-test-registry");
        hitslop_core::registry::use_folder(&folder).unwrap();
    });
}
fn package_with(schema: &str, initial: &str) -> (tempfile::TempDir, PathBuf) {
    isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let template = dir.path().join("Doc.template.slop");
    let app = file::App { package_format: 1, runtime_abi: 1, manifest: MANIFEST.into(), descriptor: schema.into(), initial: initial.into(), theme: THEME.into() };
    file::write_template(&template, &app, &[("app.js".into(), b"export default {}".to_vec())], &[]).unwrap();
    let root = dir.path().join("Doc.slop");
    file::create_document(&template, &root).unwrap();
    std::fs::remove_file(&template).unwrap();
    (dir, root)
}
fn package() -> (tempfile::TempDir, PathBuf) {
    package_with(SCHEMA, INITIAL)
}
/// The document is the database.
fn database(root: &Path) -> PathBuf {
    root.to_owned()
}
fn open(root: &Path) -> (Store, Document) {
    let store = Store::open(root, Mode::Document).unwrap();
    let doc = store.document().unwrap();
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
    assert!(matches!(result, Err(Error::Failed(ref message)) if message.contains("keep the file for recovery")), "{result:?}");
    assert_eq!(saved_updates(&root), before);
    let checkpoints: i64 = Connection::open(database(&root)).unwrap()
        .query_row("SELECT count(*) FROM checkpoint", [], |row| row.get(0)).unwrap();
    assert_eq!(checkpoints, 0, "initial state must not be written");
}

#[test]
fn writable_open_refuses_saved_updates_without_a_checkpoint() {
    missing_checkpoint_is_preserved(|root| {
        Store::open(root, Mode::Document)?.document().map(|_| ())
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
        let result = file::duplicate(root, &copy);
        assert!(!copy.exists(), "a refused source creates no file");
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
    assert!(matches!(result, Err(Error::Failed(ref message)) if message.contains("keep the file for recovery")), "{result:?}");
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
// change it ever saw. Oracle: a session that edited a large document closes with no
// history, a session that only reads trims nothing, and compaction keeps no history.
#[test]
fn closing_a_large_document_keeps_no_history_and_compaction_trims_to_now() {
    let (_dir, root) = package();
    let mut seed = 7;
    let first = session(&root, &mut seed, 100);
    let second = session(&root, &mut seed, 100);
    let (store, mut doc) = open(&root);
    assert!(stale(&doc, &first) && stale(&doc, &second), "the large document closed with no history");
    let before = store.metadata().unwrap();
    assert_eq!(close(store, &mut doc), None, "a session that only reads trims nothing");
    let (store, mut doc) = open(&root);
    assert_eq!(store.metadata().unwrap(), before);
    set_title(&mut doc, "Edited");
    let edited = doc.version();
    set_title(&mut doc, "Compacted");
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let latest = doc.version();
    store.close().unwrap();
    let doc = open(&root).1;
    assert!(stale(&doc, &edited), "compaction keeps no history");
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
    assert_eq!(title(&snapshot.document().unwrap()), "Before");
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
    assert_eq!(snapshot.document().unwrap().value().unwrap(), with_row);
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["rows",{"id":"row"},"text"],"value":"Edited after restoring"}]}).to_string(), Origin::Page).unwrap();
    let expected = doc.value().unwrap();
    close(store, &mut doc);
    assert_eq!(open(&root).1.value().unwrap(), expected);
}

// A redo restores a version from before the compaction; the live document still holds
// it, and the restore must save as updates the trimmed checkpoint can replay.
#[test]
fn redo_across_compaction_survives_reopen() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Before");
    set_title(&mut doc, "After");
    assert!(doc.undo().unwrap().publication.is_some());
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    assert!(doc.redo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "After");
    close(store, &mut doc);
    assert_eq!(title(&open(&root).1), "After");
}

// A compaction whose write fails leaves the saved state as it was; undo and the retried
// save must still reopen to what the window showed.
#[test]
fn undo_after_a_failed_compaction_saves_on_retry() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Before");
    save(&store, &mut doc);
    set_title(&mut doc, "After");
    store.set_phases(Some(Arc::new(Fail("checkpoint:uncommitted"))));
    let job = store.job(&mut doc, true).unwrap().unwrap();
    assert!(store.write(&job).is_err());
    store.set_phases(None);
    assert!(doc.undo().unwrap().publication.is_some());
    assert_eq!(title(&doc), "Before");
    save(&store, &mut doc);
    store.close().unwrap();
    assert_eq!(title(&open(&root).1), "Before");
}

// Failure: a concurrent text edit branched from a version a checkpoint had just trimmed,
// and saved an update that depends on it; the package could never be opened again.
#[test]
fn a_stale_text_base_cannot_make_the_document_unopenable() {
    let schema = r#"{"kind":"object","properties":{"title":{"kind":"text"}}}"#;
    let initial = r#"{"title":"abc"}"#;
    let (_dir, root) = package_with(schema, initial);
    let store = Store::open(&root, Mode::Document).unwrap();
    let mut doc = store.document().unwrap();
    let base = doc.version();
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["title"],"value":"Rabc"}]}).to_string(), Origin::Page).unwrap();
    store.write(&store.job(&mut doc, true).unwrap().unwrap()).unwrap();
    let request = json!({"base":base,"path":["title"],"from":"abc","to":"abcX","selectionStart":4,"selectionEnd":4});
    let result = doc.edit_text(&request.to_string()).map(|_| ());
    save(&store, &mut doc);
    store.close().unwrap();
    let store = Store::open(&root, Mode::Document).unwrap();
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
    assert!(matches!(Lease::acquire(&root), Err(Error::Locked)));
    set_title(&mut doc, "Durable");
    save(&store, &mut doc);
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    assert_eq!(title(&snapshot.document().unwrap()), "Durable");
    // A failed close keeps ownership; a retry releases it.
    store.set_phases(Some(Arc::new(Fail("close"))));
    assert!(store.close().is_err());
    assert!(matches!(Store::open(&root, Mode::Document), Err(Error::Locked)));
    store.set_phases(None);
    store.close().unwrap();
    drop(Lease::acquire(&root).unwrap());
}

#[test]
fn snapshots_never_lock_or_modify_the_document() {
    let (_dir, root) = package();
    // Never opened by a writer: no saved state yet, so a snapshot starts from the app's
    // initial values in memory.
    let before = std::fs::read(&root).unwrap();
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    let mut doc = snapshot.document().unwrap();
    set_title(&mut doc, "In memory");
    save(&snapshot, &mut doc);
    assert_eq!(std::fs::read(&root).unwrap(), before, "a render of an unsaved document writes nothing");
    assert!(matches!(snapshot.check(true), Err(Error::Closed)), "snapshots own nothing");

    let (store, _) = open(&root);
    store.close().unwrap();
    let before = std::fs::read(&root).unwrap();
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    let mut doc = snapshot.document().unwrap();
    set_title(&mut doc, "In memory");
    save(&snapshot, &mut doc);
    assert_eq!(std::fs::read(&root).unwrap(), before);
    drop(Lease::acquire(&root).expect("a snapshot holds no lock"));
}

#[test]
fn foreign_databases_are_refused_and_left_unchanged() {
    let (_dir, root) = package();
    std::fs::remove_file(&root).unwrap();
    Connection::open(database(&root)).unwrap().execute_batch("PRAGMA application_id=1; CREATE TABLE t(x);").unwrap();
    let before = std::fs::read(database(&root)).unwrap();
    for mode in [Mode::Document, Mode::Snapshot] {
        let error = Store::open(&root, mode).err().expect("refused");
        assert!(error.to_string().contains("not a hitSlop document"), "{error}");
    }
    assert_eq!(std::fs::read(database(&root)).unwrap(), before);
}

// A document a newer build saved asks for an update, from every reader, and is never
// written: the newer build can still open it.
#[test]
fn a_newer_storage_version_asks_for_an_update_and_is_left_unchanged() {
    let (dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Newer");
    save(&store, &mut doc);
    store.close().unwrap();
    Connection::open(database(&root)).unwrap().execute_batch("PRAGMA user_version=2;").unwrap();
    let before = std::fs::read(database(&root)).unwrap();
    let copy = dir.path().join("Copy.slop");
    let refusals = [
        Store::open(&root, Mode::Document).err(),
        Store::open(&root, Mode::Snapshot).err(),
        file::duplicate(&root, &copy).err(),
    ];
    for refusal in refusals {
        assert!(matches!(refusal, Some(Error::Rejected(ref e)) if e.code == hitslop_core::Code::RequiresUpdate), "{refusal:?}");
    }
    assert!(!copy.exists());
    assert_eq!(std::fs::read(database(&root)).unwrap(), before);
}

// Failure: saved state recorded its descriptor as a string, and opening compared strings,
// so a build that spelled the same descriptor differently (field order, `1` for `1.0`)
// could no longer open any document. Saved state opens under any spelling of the same
// descriptor, and a different descriptor is still refused.
#[test]
fn saved_state_opens_under_any_spelling_of_its_descriptor() {
    const BOUNDED: &str = r#"{"kind":"object","properties":{"title":{"kind":"string","maxLength":40},"score":{"kind":"number","min":0,"max":10}}}"#;
    const BOUNDED_INITIAL: &str = r#"{"title":"Saved","score":1}"#;
    let (_dir, root) = package_with(BOUNDED, BOUNDED_INITIAL);
    let store = Store::open(&root, Mode::Document).unwrap();
    let mut doc = store.document().unwrap();
    doc.apply_batch(r#"{"intents":[{"type":"set","path":["score"],"value":7}]}"#, Origin::Page).unwrap();
    save(&store, &mut doc);
    store.close().unwrap();
    // Another build's spelling of the same descriptor, recorded with the saved state.
    let respelled = r#"{"properties":{"score":{"max":10.0,"min":0.0,"kind":"number"},"title":{"maxLength":40,"kind":"string"}},"kind":"object"}"#;
    Connection::open(database(&root)).unwrap().execute("UPDATE checkpoint SET schema_key=?", [respelled]).unwrap();
    for mode in [Mode::Document, Mode::Snapshot] {
        let store = Store::open(&root, mode).unwrap();
        let doc = store.document().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&doc.value().unwrap()).unwrap()["score"], 7);
        store.close().unwrap();
    }
    // An app whose descriptor means something else does not open the saved state.
    Connection::open(database(&root)).unwrap().execute("UPDATE app SET descriptor=?", [BOUNDED.replace(r#""max":10"#, r#""max":11"#)]).unwrap();
    for mode in [Mode::Document, Mode::Snapshot] {
        let error = Store::open(&root, mode).unwrap().document().err().expect("refused");
        assert!(error.to_string().contains("Document schema differs"), "{error}");
    }
}

#[test]
fn a_symlinked_document_is_refused() {
    let (dir, root) = package();
    let (store, _) = open(&root);
    store.close().unwrap();
    let real = dir.path().join("Real.slop");
    std::fs::rename(&root, &real).unwrap();
    std::os::unix::fs::symlink(&real, &root).unwrap();
    assert!(Store::open(&root, Mode::Document).is_err());
    assert!(Store::open(&root, Mode::Snapshot).is_err());
    let copy = dir.path().join("Copy.slop");
    assert!(file::duplicate(&root, &copy).is_err(), "a duplicate never follows a link");
    assert!(!copy.exists());
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
    let error = store.document().err().expect("refused");
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
fn a_moved_document_refuses_writes() {
    let (dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Unsaved");
    let job = store.job(&mut doc, false).unwrap().unwrap();
    std::fs::rename(&root, dir.path().join("Moved.slop")).unwrap();
    std::fs::write(&root, b"").unwrap();
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
    // The source stays open: the copy is an online backup.
    file::duplicate(&root, &copy).unwrap();
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
        std::fs::remove_file(root).unwrap();
        Connection::open(database(root)).unwrap().execute_batch("PRAGMA application_id=1; CREATE TABLE t(x);").unwrap();
    };
    let empty = |root: &Path| {
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
        assert!(file::duplicate(&root, &copy).is_err());
        assert!(!copy.exists(), "no file is created for a refused source");
    }
}

fn accent(store: &Store) -> String {
    let (theme, _) = store.theme(Change::Get).unwrap();
    serde_json::from_str::<Value>(&theme.effective).unwrap()["accent"].as_str().unwrap().into()
}
fn saved_accent(root: &Path) -> String {
    let snapshot = Store::open(root, Mode::Snapshot).unwrap();
    snapshot.document().unwrap();
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
    assert_eq!(title(&snapshot.document().unwrap()), "Both");
    assert!(matches!(snapshot.theme(Change::Reset(None)), Err(Error::Closed)));
    assert!(matches!(store.theme(Change::Set(r##"{"unknown":"#000000"}"##)), Err(Error::Rejected(_))));
    store.close().unwrap();
    assert_eq!(accent(&open(&root).0), "#222222");
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
    store.document().unwrap();
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
    file::duplicate(&root, &copy).unwrap();
    assert_eq!(accent(&open(&copy).0), "#123456");
}

// Failure: a read-only connection beside the writer in the same process (Duplicate of an
// open document, a snapshot render) intermittently made the platform SQLite fail the
// writer's locks with EBADF (SQLITE_IOERR_LOCK), and sometimes the reader's too.
#[test]
fn readers_beside_an_open_document_never_fail_its_saves() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let (dir, root) = package();
    let (store, mut doc) = open(&root);
    let stop = Arc::new(AtomicBool::new(false));
    let reader = {
        let (root, stop, scratch) = (root.clone(), stop.clone(), dir.path().to_owned());
        std::thread::spawn(move || {
            let mut failures = vec![];
            let mut round = 0;
            while !stop.load(Ordering::Relaxed) {
                let copy = scratch.join(format!("Copy{round}.slop"));
                if let Err(e) = file::duplicate(&root, &copy) {
                    failures.push(format!("duplicate: {e}"));
                }
                let _ = std::fs::remove_file(&copy);
                if let Err(e) = Store::open(&root, Mode::Snapshot).and_then(|s| s.document().map(|_| ())) {
                    failures.push(format!("snapshot: {e}"));
                }
                round += 1;
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
