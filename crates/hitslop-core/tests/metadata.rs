//! Commit attribution survives persistence, branches and rollback without becoming an audit log.
use hitslop_core::{AppSpec, Batch, Document, Origin};
use loro::LoroDoc;
use serde_json::{Value, json};

fn batch(value: Value) -> Batch {
    Batch::decode(&value.to_string()).unwrap()
}
fn latest(doc: &Document, message: &str) {
    let raw = LoroDoc::new();
    raw.import(&doc.checkpoint().unwrap()).unwrap();
    for id in raw.oplog_frontiers().iter() {
        let change = raw.get_change(id).unwrap();
        assert_eq!(change.message(), message);
        assert!(change.timestamp() > 0, "live edits record a timestamp");
    }
}

#[test]
fn every_live_origin_records_its_message_and_timestamp_after_reopen() {
    let app = AppSpec::new(
        r#"{"kind":"object","properties":{"title":{"kind":"text"},"hits":{"kind":"counter"}}}"#,
        "metadata",
        r##"{"accent":"#123456"}"##,
    )
    .unwrap();
    let mut doc = Document::create(&app, r#"{"title":"Hello","hits":0}"#).unwrap();
    latest(&doc, "create");
    for (origin, message) in [(Origin::Page, "page"), (Origin::Agent, "agent")] {
        doc.apply_batch(batch(json!({"intents":[{"type":"increment","path":["hits"],"by":1}]})), origin).unwrap();
        latest(&doc, message);
    }
    doc.apply_batch(batch(json!({"intents":[{"type":"setTheme","values":{"accent":"#abcdef"}}]})), Origin::Window)
        .unwrap();
    latest(&doc, "window");
    doc.apply_command(batch(json!({"intents":[{"type":"increment","path":["hits"],"by":1}]})), Origin::Page, "bump")
        .unwrap();
    latest(&doc, "command:bump");
    doc.undo().unwrap();
    latest(&doc, "undo");
    doc.redo().unwrap();
    latest(&doc, "redo");
    let mut reopened = Document::open(&app, &doc.checkpoint().unwrap(), &[]).unwrap();
    reopened
        .apply_batch(batch(json!({"intents":[{"type":"increment","path":["hits"],"by":1}]})), Origin::Page)
        .unwrap();
    latest(&reopened, "page");
}

#[test]
fn merged_text_and_post_rollback_edits_keep_live_metadata() {
    let app = AppSpec::data(r#"{"kind":"object","properties":{"title":{"kind":"text"}}}"#).unwrap();
    let mut doc = Document::create(&app, r#"{"title":"Hello"}"#).unwrap();
    doc.apply_batch(batch(json!({"intents":[{"type":"set","path":["title"],"value":"Hello A"}]})), Origin::Page)
        .unwrap();
    doc.apply_batch(
        batch(json!({"intents":[{"type":"set","path":["title"],"from":"Hello","value":"Hello B"}]})),
        Origin::Agent,
    )
    .unwrap();
    // The merged edit is the session's own commit, labeled by its origin.
    let raw = LoroDoc::new();
    raw.import(&doc.checkpoint().unwrap()).unwrap();
    assert!(raw.oplog_frontiers().iter().any(|id| raw.get_change(id).unwrap().message() == "agent"));
    for id in raw.oplog_frontiers().iter() {
        assert!(raw.get_change(id).unwrap().timestamp() > 0);
    }
    assert!(
        doc.apply_batch(
            batch(json!({"intents":[
                {"type":"set","path":["title"],"value":"Rolled back"},
                {"type":"set","path":["missing"],"value":"bad"}
            ]})),
            Origin::Agent
        )
        .is_err()
    );
    doc.apply_batch(batch(json!({"intents":[{"type":"set","path":["title"],"value":"After rollback"}]})), Origin::Page)
        .unwrap();
    latest(&doc, "page");
}
