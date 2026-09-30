//! Durable storage: identity, limits, crash outcomes and the writer lock.
use hitslop_core::store::{self, Error, Mode, Phases, Store};
use hitslop_core::theme::Change;
use hitslop_core::{STORAGE_BYTES, STORAGE_ROWS};
use hitslop_core::Document;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"string"},"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"string"}}}}}}"#;
const INITIAL: &str = r#"{"title":"Saved","rows":[]}"#;

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
    let doc = store.document(&key(), INITIAL).unwrap();
    (store, doc)
}
fn title(doc: &Document) -> String {
    let value: Value = serde_json::from_str(&doc.value().unwrap()).unwrap();
    value["title"].as_str().unwrap().into()
}
fn set_title(doc: &mut Document, title: &str) {
    doc.apply_batch(&json!({"intents":[{"type":"set","path":["title"],"value":title}]}).to_string()).unwrap();
}
fn save(store: &Store, doc: &Document) -> Option<bool> {
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
        save(&store, &doc);
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
        Store::open(root, Mode::Document)?.document(&key(), INITIAL).map(|_| ())
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
    save(&store, &doc);
    set_title(&mut doc, "Pending edit");
    let job = store.job(&doc, false).unwrap().unwrap();
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
    assert!(store.job(&doc, false).unwrap().is_none(), "a clean document has nothing to save");
    set_title(&mut doc, "Edited");
    assert_eq!(save(&store, &doc), Some(false), "an edit appends");
    assert_eq!(store.metadata().unwrap().rows, 1);
    assert!(store.job(&doc, false).unwrap().is_none());
    store.close().unwrap();
    let (reopened, doc) = open(&root);
    assert_eq!(title(&doc), "Edited");
    assert_eq!(reopened.doc_id(), id);
}

#[test]
fn a_long_log_checkpoints_and_compaction_is_always_a_checkpoint() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    for i in 0..256 {
        set_title(&mut doc, &format!("Edit {i}"));
        assert_eq!(save(&store, &doc), Some(false));
    }
    set_title(&mut doc, "Last");
    assert_eq!(save(&store, &doc), Some(true), "256 rows checkpoint");
    assert_eq!(store.metadata().unwrap().rows, 0);
    let job = store.job(&doc, true).unwrap().expect("a requested checkpoint is written even when clean");
    assert!(job.is_checkpoint());
    store.write(&job).unwrap();
    store.close().unwrap();
    assert_eq!(title(&open(&root).1), "Last");
}

#[test]
fn the_writer_lock_admits_one_owner_and_snapshots_read_without_it() {
    let (_dir, root) = package();
    let (store, mut doc) = open(&root);
    assert!(matches!(Store::open(&root, Mode::Document), Err(Error::Locked)));
    assert!(matches!(store::WriterLock::acquire(&root), Err(Error::Locked)));
    set_title(&mut doc, "Durable");
    save(&store, &doc);
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    assert_eq!(title(&snapshot.document(&key(), INITIAL).unwrap()), "Durable");
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
    let mut doc = snapshot.document(&key(), INITIAL).unwrap();
    set_title(&mut doc, "In memory");
    save(&snapshot, &doc);
    assert!(!root.join("state").exists(), "a render of an unsaved package writes nothing");
    assert!(matches!(snapshot.check(true), Err(Error::Closed)), "snapshots own nothing");

    let (store, _) = open(&root);
    store.close().unwrap();
    let before = std::fs::read(database(&root)).unwrap();
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    let mut doc = snapshot.document(&key(), INITIAL).unwrap();
    set_title(&mut doc, "In memory");
    save(&snapshot, &doc);
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
    let error = store.document(&key(), INITIAL).err().expect("refused");
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
    let job = store.job(&doc, false).unwrap().unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(store.write(&job), Err(Error::Busy)));
    assert!(started.elapsed() >= std::time::Duration::from_secs(1), "waits for the busy timeout");
    other.execute_batch("COMMIT").unwrap();
    store.write(&store.job(&doc, false).unwrap().unwrap()).unwrap();
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
        let job = store.job(&doc, phase.starts_with("checkpoint")).unwrap().unwrap();
        assert!(store.write(&job).is_err());
        store.set_phases(None);
        let committed = phase.ends_with(":committed");
        let rows = if phase == "append:committed" { 1 } else { 0 };
        assert_eq!(store.metadata().unwrap().rows, rows, "{phase}: sizes are re-read after a failure");
        let retry = store.job(&doc, false).unwrap().expect("the unacknowledged edit is saved again");
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
    let job = store.job(&doc, false).unwrap().unwrap();
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
    let job = store.job(&doc, false).unwrap().unwrap();
    let result = store.write(&job);
    assert!(matches!(result, Err(Error::Full)), "{result:?}");
    assert!(store.metadata().unwrap().update_bytes > 0, "saved state is intact");
    // Sizes were re-read, so the next save checkpoints, which fits.
    let retry = store.job(&doc, false).unwrap().unwrap();
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
        save(&store, &doc);
    }
    assert!(std::fs::metadata(database(&root)).unwrap().len() > 3 * 1024 * 1024);
    for _ in 0..48 {
        doc.apply_batch(&json!({"intents":[{"type":"insert","path":["rows"],"value":{"text":"row"}}]}).to_string()).unwrap();
        save(&store, &doc);
    }
    store.write(&store.job(&doc, true).unwrap().unwrap()).unwrap();
    let logical = store.metadata().unwrap().checkpoint_bytes as u64;
    let file = std::fs::metadata(database(&root)).unwrap().len();
    assert!(file < logical + 256 * 1024, "file {file} bytes for {logical} logical bytes");
}

#[test]
fn a_duplicate_has_the_same_history_and_a_new_identity() {
    let (dir, root) = package();
    let (store, mut doc) = open(&root);
    set_title(&mut doc, "Copied");
    save(&store, &doc);
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

const DEFAULTS: &str = r##"{"accent":"#335577"}"##;
fn accent(store: &Store) -> String {
    let theme = store.theme(DEFAULTS, Change::Get).unwrap();
    serde_json::from_str::<Value>(&theme.effective).unwrap()["accent"].as_str().unwrap().into()
}

#[test]
fn theme_overrides_persist_and_snapshots_keep_what_they_read() {
    let (_dir, root) = package();
    let (store, _) = open(&root);
    assert_eq!(accent(&store), "#335577");
    store.theme(DEFAULTS, Change::Set(r##"{"accent":"#111111"}"##)).unwrap();
    let snapshot = Store::open(&root, Mode::Snapshot).unwrap();
    store.theme(DEFAULTS, Change::Set(r##"{"accent":"#222222"}"##)).unwrap();
    // A render sees the theme saved with the document it read.
    assert_eq!(accent(&snapshot), "#111111");
    assert!(matches!(snapshot.theme(DEFAULTS, Change::Reset(None)), Err(Error::Closed)));
    assert!(matches!(store.theme(DEFAULTS, Change::Set(r#"{"unknown":"red"}"#)), Err(Error::Rejected(_))));
    store.close().unwrap();
    assert_eq!(accent(&open(&root).0), "#222222");
    let entries: Vec<_> = std::fs::read_dir(root.join("state")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(entries.iter().all(|name| !name.to_string_lossy().starts_with(".theme-")), "no staged file remains");
}

// Failure: theme overrides kept in `state/theme.json` could be redirected through a
// swapped `state/` link, block on a FIFO, or leave a staging file that failed package
// validation. They now live in the document's database.
#[test]
fn theme_overrides_live_in_the_database() {
    let (_dir, root) = package();
    let (store, _) = open(&root);
    store.theme(DEFAULTS, Change::Set(r##"{"accent":"#101010"}"##)).unwrap();
    assert!(!root.join("state/theme.json").exists());
    // Whatever sits at the old path is never opened.
    let fifo = std::ffi::CString::new(root.join("state/theme.json").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(accent(&store), "#101010");
    store.theme(DEFAULTS, Change::Set(r##"{"accent":"#202020"}"##)).unwrap();
    assert_eq!(accent(&store), "#202020");
}

#[test]
fn a_busy_database_fails_a_theme_write_and_keeps_the_old_theme() {
    let (_dir, root) = package();
    let (store, _) = open(&root);
    let other = Connection::open(database(&root)).unwrap();
    other.busy_timeout(std::time::Duration::ZERO).unwrap();
    other.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert!(matches!(store.theme(DEFAULTS, Change::Set(r##"{"accent":"#303030"}"##)), Err(Error::Busy)));
    other.execute_batch("COMMIT").unwrap();
    assert_eq!(accent(&store), "#335577");
}

// Failure: a theme command that changed nothing still wrote the document row, so it
// could fail on a busy database (and rewrote saved state for nothing).
#[test]
fn an_unchanged_theme_is_not_written() {
    let (_dir, root) = package();
    let (store, _) = open(&root);
    // Another writer holds the write lock; readers may continue.
    let other = Connection::open(database(&root)).unwrap();
    other.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(store.theme(DEFAULTS, Change::Reset(None)).unwrap().overrides, "{}");
    other.execute_batch("COMMIT").unwrap();
}

#[test]
fn a_duplicate_carries_theme_overrides() {
    let (dir, root) = package();
    let (store, _) = open(&root);
    store.theme(DEFAULTS, Change::Set(r##"{"accent":"#123456"}"##)).unwrap();
    let copy = dir.path().join("Copy.slop");
    std::fs::create_dir(&copy).unwrap();
    store::duplicate(&root, &copy).unwrap();
    assert_eq!(accent(&open(&copy).0), "#123456");
}
