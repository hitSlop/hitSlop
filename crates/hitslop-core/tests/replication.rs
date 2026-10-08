//! Opt-in authority update installation, separate from transport and owner admission.
use hitslop_core::{
    AppSpec, Batch, Code, Document, Origin, file,
    store::{Mode, Store},
};
use loro::{ExportMode, LoroDoc};
use serde_json::{Value, json};
mod support;

const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"text"},"hits":{"kind":"counter"}}}"#;
const INITIAL: &str = r#"{"title":"Hello","hits":0}"#;

fn app() -> AppSpec {
    AppSpec::new(SCHEMA, "checklist", support::THEME).unwrap()
}
fn edit(doc: &mut Document, intents: Value) {
    doc.apply_batch(Batch::decode(&json!({"intents":intents}).to_string()).unwrap(), Origin::Agent).unwrap();
}
fn raw(doc: &Document) -> LoroDoc {
    let raw = LoroDoc::new();
    raw.import(&doc.checkpoint().unwrap()).unwrap();
    raw
}
fn value(doc: &Document) -> Value {
    serde_json::from_str(&doc.value()).unwrap()
}

#[test]
fn duplicate_updates_and_history_only_changes_have_correct_versions() {
    let mut authority = Document::create(&app(), INITIAL).unwrap();
    let mut replica = Document::open(&app(), &authority.checkpoint().unwrap(), &[]).unwrap();
    edit(
        &mut authority,
        json!([
            {"type":"increment","path":["hits"],"by":3},
            {"type":"setTheme","values":{"accent":"#abcdef"}}
        ]),
    );
    let updates = authority.updates_since(&replica.version_vector()).unwrap();
    let applied = replica.import_accepted(&updates).unwrap();
    assert!(applied.theme_changed);
    assert_eq!(replica.reading().unwrap().to_json(), authority.reading().unwrap().to_json());
    assert_eq!(replica.version_vector(), authority.version_vector());
    assert!(replica.import_accepted(&updates).unwrap().publication.is_none());
    assert_eq!(replica.sequence(), 1);

    let raw = raw(&authority);
    let base = raw.oplog_vv();
    raw.get_map("meta").insert("proof-only", "history").unwrap();
    raw.commit();
    let extra = raw.export(ExportMode::updates(&base)).unwrap();
    let result = replica.import_accepted(&extra).unwrap();
    let publication: Value = serde_json::from_str(result.publication.as_ref().unwrap()).unwrap();
    assert_eq!(publication["ops"], json!([]));
    assert_eq!(publication["previous"], 1);
    assert_eq!(publication["sequence"], 2);
    assert_eq!(publication["version"], replica.version());
    let reopened = Document::open(&app(), &replica.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(reopened.version_vector(), replica.version_vector());
}

#[test]
fn missing_dependencies_never_remain_latent_after_rejection() {
    let mut replica = Document::create(&app(), INITIAL).unwrap();
    let before = replica.state().unwrap();
    let raw = raw(&replica);
    let base = raw.oplog_vv();
    raw.get_map("data").insert("hits", 1).unwrap();
    raw.commit();
    let prerequisite = raw.export(ExportMode::updates(&base)).unwrap();
    let after_first = raw.oplog_vv();
    raw.get_map("data").insert("hits", 2).unwrap();
    raw.commit();
    let dependent = raw.export(ExportMode::updates(&after_first)).unwrap();
    assert_eq!(replica.import_accepted(&dependent).unwrap_err().code, Code::MissingDependencies);
    assert_eq!(replica.state().unwrap(), before);
    replica.import_accepted(&prerequisite).unwrap();
    assert_eq!(value(&replica)["hits"], 1, "refused operation must not activate later");
    assert_eq!(loro::VersionVector::decode(&replica.version_vector()).unwrap(), after_first);
    replica.import_accepted(&dependent).unwrap();
    assert_eq!(value(&replica)["hits"], 2);
}

#[test]
fn invalid_state_theme_and_layout_imports_leave_no_partial_history() {
    let mut replica = Document::create(&app(), INITIAL).unwrap();
    for (root, key, invalid) in
        [("data", "hits", json!(1.5)), ("theme", "accent", json!("bad-color")), ("meta", "layout", json!(999))]
    {
        let before = replica.state().unwrap();
        let version = replica.version_vector();
        let raw = raw(&replica);
        let base = raw.oplog_vv();
        raw.get_map("data").insert("hits", 9).unwrap();
        match invalid {
            Value::Number(n) if n.is_i64() => {
                raw.get_map(root).insert(key, n.as_i64().unwrap()).unwrap();
            }
            Value::Number(n) => {
                raw.get_map(root).insert(key, n.as_f64().unwrap()).unwrap();
            }
            Value::String(s) => {
                raw.get_map(root).insert(key, s).unwrap();
            }
            _ => unreachable!(),
        }
        raw.commit();
        assert!(replica.import_accepted(&raw.export(ExportMode::updates(&base)).unwrap()).is_err());
        assert_eq!(replica.state().unwrap(), before);
        assert_eq!(replica.version_vector(), version);
        edit(&mut replica, json!([{"type":"increment","path":["hits"],"by":1}]));
    }
}

fn stored_document() -> (tempfile::TempDir, Store, Document) {
    support::isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/ui.js"), "export default {}").unwrap();
    support::write_app(&stage, support::App::new(SCHEMA, INITIAL));
    let template = dir.path().join("Template.slop");
    support::pack(&stage, &template).unwrap();
    let path = dir.path().join("Authority.slop");
    file::create_document(&template, &path).unwrap();
    let store = Store::open(&path, Mode::Document).unwrap();
    let doc = store.document().unwrap();
    (dir, store, doc)
}

#[test]
fn durable_bootstrap_and_same_version_snapshot_preserve_identity_and_app() {
    let (dir, store, mut authority) = stored_document();
    edit(&mut authority, json!([{"type":"increment","path":["hits"],"by":4}]));
    store.write(&store.job(&mut authority, false).unwrap().unwrap()).unwrap();
    let identity = store.app_digest().unwrap();
    let path = dir.path().join("Replica.slop");
    store.backup(&path).unwrap();
    let replica_store = Store::open(&path, Mode::Document).unwrap();
    let replica = replica_store.document().unwrap();
    assert_eq!(replica_store.app_digest().unwrap(), identity);
    assert_eq!(replica.version_vector(), authority.version_vector());
    let snapshot = support::trimmed(&authority.checkpoint().unwrap());
    let candidate = replica.snapshot_candidate(&snapshot).unwrap();
    assert_eq!(candidate.sequence(), replica.sequence() + 1);
    let reset: Value = serde_json::from_str(&candidate.reset_publication(replica.sequence()).unwrap()).unwrap();
    assert_eq!(reset["ops"][0]["value"], value(&candidate));
    assert_eq!(reset["theme"]["accent"], "#335577");
    let job = replica_store.replacement(&candidate).unwrap();
    assert!(job.is_checkpoint());
    replica_store.write(&job).unwrap();
    // The first commit may have reached disk even if its reply was lost. Retrying the
    // identical replacement is idempotent, including opened/saved history accounting.
    replica_store.write(&job).unwrap();
    let mut still_same = replica.snapshot_candidate(&snapshot).unwrap();
    assert!(replica_store.job(&mut still_same, false).unwrap().is_none());
    replica_store.close().unwrap();
    let reopened_store = Store::open(&path, Mode::Document).unwrap();
    let reopened = reopened_store.document().unwrap();
    assert_eq!(reopened_store.app_digest().unwrap(), identity);
    assert_eq!(reopened.version_vector(), candidate.version_vector());
    assert!(raw(&reopened).is_shallow());
}

#[test]
fn snapshot_refuses_divergent_history_and_invalid_state() {
    let mut replica = Document::create(&app(), INITIAL).unwrap();
    let seed = replica.checkpoint().unwrap();
    edit(&mut replica, json!([{"type":"increment","path":["hits"],"by":1}]));
    assert_eq!(replica.snapshot_candidate(&seed).err().unwrap().code, Code::StaleBase);
    let invalid = raw(&replica);
    invalid.get_map("data").insert("hits", "not a number").unwrap();
    invalid.commit();
    assert!(replica.snapshot_candidate(&invalid.export(ExportMode::Snapshot).unwrap()).is_err());
    assert_eq!(value(&replica)["hits"], 1);
    let newer = replica.version_vector();
    let older = Document::open(&app(), &seed, &[]).unwrap();
    assert_eq!(older.updates_since(&newer).unwrap_err().code, Code::StaleBase);
}

#[test]
fn failed_snapshot_write_keeps_durable_account_and_live_replica_unchanged() {
    let (dir, store, replica) = stored_document();
    let original = replica.state().unwrap();
    let mut authority = Document::open(&app(), &replica.checkpoint().unwrap(), &[]).unwrap();
    edit(
        &mut authority,
        json!([
            {"type":"increment","path":["hits"],"by":7},
            {"type":"setTheme","values":{"accent":"#abcdef"}}
        ]),
    );
    let mut candidate = replica.snapshot_candidate(&authority.transfer_snapshot().unwrap()).unwrap();
    let job = store.replacement(&candidate).unwrap();
    let blocker = rusqlite::Connection::open(dir.path().join("Authority.slop")).unwrap();
    blocker.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert!(store.write(&job).is_err());
    assert_eq!(replica.state().unwrap(), original);
    assert!(store.job(&mut candidate, false).unwrap().is_some(), "failed save cannot advance the durable version");
    blocker.execute_batch("ROLLBACK").unwrap();
    let durable = Store::open(&dir.path().join("Authority.slop"), Mode::Snapshot).unwrap().document().unwrap();
    assert_eq!(durable.state().unwrap(), original);
    store.write(&job).unwrap();
    assert_eq!(store.document().unwrap().reading().unwrap().to_json(), authority.reading().unwrap().to_json());
}

#[test]
fn history_only_import_is_saved_and_available_after_reopen() {
    let (_dir, store, mut replica) = stored_document();
    let authority = raw(&replica);
    let base = authority.oplog_vv();
    authority.get_map("meta").insert("proof-only", true).unwrap();
    authority.commit();
    let applied = replica.import_accepted(&authority.export(ExportMode::updates(&base)).unwrap()).unwrap();
    assert!(applied.publication.is_some());
    let job = store.job(&mut replica, false).unwrap().expect("history alone is a save");
    store.write(&job).unwrap();
    assert_eq!(store.document().unwrap().version_vector(), replica.version_vector());
}

#[test]
fn checkpoint_packet_cannot_trim_the_live_replica_before_validation() {
    let mut replica = Document::create(&app(), INITIAL).unwrap();
    let before = replica.state().unwrap();
    let mut authority = Document::open(&app(), &replica.checkpoint().unwrap(), &[]).unwrap();
    edit(&mut authority, json!([{"type":"increment","path":["hits"],"by":8}]));
    let shallow = support::trimmed(&authority.checkpoint().unwrap());
    assert_eq!(replica.import_accepted(&shallow).unwrap_err().code, Code::InvalidBytes);
    assert_eq!(replica.state().unwrap(), before);
    replica.import_accepted(&authority.updates_since(&replica.version_vector()).unwrap()).unwrap();
    assert_eq!(value(&replica)["hits"], 8);
}
