//! Palette edits use the document's history, sequence, publications and saved bytes.
use hitslop_core::{Document, Origin, theme::Change};
use serde_json::{Value, json};
mod support;
use support::{View, snapshot};

const SCHEMA: &str =
    r#"{"kind":"object","properties":{"title":{"kind":"string"},"hits":{"kind":"counter"}}}"#;
const INITIAL: &str = r#"{"title":"Initial","hits":0}"#;
const DEFAULTS: &str = r##"{"accent":"#335577","paper":"#ffffff"}"##;
fn doc() -> Document {
    Document::create_with_theme(SCHEMA, INITIAL, "palette-test", DEFAULTS).unwrap()
}
fn color(doc: &Document) -> Value {
    snapshot(doc)["theme"]["accent"].clone()
}
fn set(doc: &mut Document, color: &str) -> hitslop_core::ThemeApplied {
    doc.theme(Change::Set(&json!({"accent": color}).to_string()))
        .unwrap()
}

#[test]
fn theme_only_edits_publish_under_the_document_sequence_and_reopen() {
    let mut doc = doc();
    let mut view = View::of(&doc);
    let before = snapshot(&doc);
    let change = set(&mut doc, "#123456");
    assert_eq!(change.result.sequence, 1);
    let publication = change.result.publication.unwrap();
    let payload: Value = serde_json::from_str(&publication).unwrap();
    assert_eq!(payload["previous"], 0);
    assert_eq!(payload["ops"], json!([]));
    assert_eq!(payload["theme"]["accent"], "#123456");
    view.publish(&publication);
    view.check(&doc, "theme publication");
    assert_eq!(snapshot(&doc)["value"], before["value"]);
    assert_eq!(
        snapshot(&doc),
        serde_json::from_str::<Value>(&doc.state().unwrap()).unwrap()
    );
    let reopened = Document::open_with_theme(
        SCHEMA,
        &doc.checkpoint().unwrap(),
        &[],
        "palette-test",
        DEFAULTS,
    )
    .unwrap();
    assert_eq!(snapshot(&reopened)["theme"], snapshot(&doc)["theme"]);
    assert_eq!(reopened.version(), doc.version());
    assert!(
        !reopened.can_undo(),
        "history is scoped to the open session"
    );
}

#[test]
fn theme_and_data_share_undo_without_replacement_touching_theme() {
    let mut doc = doc();
    set(&mut doc, "#123456");
    doc.apply_batch(
        r#"{"intents":[{"type":"replace","path":[],"value":{"title":"Replaced","hits":8}}]}"#,
        Origin::Agent,
    )
    .unwrap();
    assert_eq!(color(&doc), "#123456");
    let mut view = View::of(&doc);
    view.publish(&doc.undo().unwrap().publication.unwrap());
    view.check(&doc, "undo data");
    assert_eq!(snapshot(&doc)["value"], json!({"title":"Initial","hits":0}));
    assert_eq!(color(&doc), "#123456");
    view.publish(&doc.undo().unwrap().publication.unwrap());
    view.check(&doc, "undo theme");
    assert_eq!(color(&doc), "#335577");
    view.publish(&doc.redo().unwrap().publication.unwrap());
    view.check(&doc, "redo theme");
    assert_eq!(color(&doc), "#123456");
    doc.redo().unwrap();
    assert_eq!(snapshot(&doc)["value"]["hits"], 8);
}

#[test]
fn palette_noops_and_refusals_preserve_sequence_redo_and_undo_group() {
    let mut doc = doc();
    assert!(
        doc.theme(Change::Reset(None))
            .unwrap()
            .result
            .publication
            .is_none()
    );
    assert!(set(&mut doc, "#335577").result.publication.is_none());
    assert!(!doc.can_undo());
    set(&mut doc, "#123456");
    doc.undo().unwrap();
    let before = snapshot(&doc);
    assert!(set(&mut doc, "#335577").result.publication.is_none());
    assert!(
        doc.theme(Change::Set(r##"{"accent":"#111111","missing":"#000000"}"##))
            .is_err()
    );
    assert_eq!(snapshot(&doc), before);
    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert_eq!(color(&doc), "#123456");
}

#[test]
fn panel_gestures_group_colors_and_cli_commands_remain_separate_steps() {
    let mut doc = doc();
    doc.begin_theme_gesture();
    set(&mut doc, "#111111");
    assert!(set(&mut doc, "#111111").result.publication.is_none());
    set(&mut doc, "#222222");
    doc.end_theme_gesture();
    set(&mut doc, "#333333");
    set(&mut doc, "#444444");
    for expected in ["#333333", "#222222", "#335577"] {
        doc.undo().unwrap();
        assert_eq!(color(&doc), expected);
    }
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(color(&doc), "#222222");
}

#[test]
fn data_between_color_updates_ends_the_theme_undo_run() {
    let mut doc = doc();
    doc.begin_theme_gesture();
    set(&mut doc, "#111111");
    doc.apply_batch(
        r#"{"intents":[{"type":"increment","path":["hits"],"by":2}]}"#,
        Origin::Agent,
    )
    .unwrap();
    set(&mut doc, "#222222");
    doc.end_theme_gesture();
    doc.undo().unwrap();
    assert_eq!(color(&doc), "#111111");
    assert_eq!(snapshot(&doc)["value"]["hits"], 2);
    doc.undo().unwrap();
    assert_eq!(snapshot(&doc)["value"]["hits"], 0);
    assert_eq!(color(&doc), "#111111");
}

#[test]
fn replica_import_merges_theme_tokens_and_counters_together() {
    let mut a = doc();
    let mut b = Document::open_with_theme(
        SCHEMA,
        &a.checkpoint().unwrap(),
        &[],
        "palette-test",
        DEFAULTS,
    )
    .unwrap();
    let base = a.version();
    set(&mut a, "#123456");
    b.theme(Change::Set(r##"{"paper":"#eeeeee"}"##)).unwrap();
    for (doc, amount) in [(&mut a, 2), (&mut b, 3)] {
        doc.apply_batch(
            &json!({"intents":[{"type":"increment","path":["hits"],"by":amount}]}).to_string(),
            Origin::Page,
        )
        .unwrap();
    }
    let mut views = [View::of(&a), View::of(&b)];
    let (left, right) = (
        a.export_since(&base).unwrap(),
        b.export_since(&base).unwrap(),
    );
    views[0].publish(&a.import(&right).unwrap().unwrap());
    views[1].publish(&b.import(&left).unwrap().unwrap());
    views[0].check(&a, "merged theme");
    views[1].check(&b, "merged theme");
    assert_eq!(
        snapshot(&a)["theme"],
        json!({"accent":"#123456","paper":"#eeeeee"})
    );
    assert_eq!(snapshot(&a)["theme"], snapshot(&b)["theme"]);
    assert_eq!(snapshot(&a)["value"]["hits"], 5);
    assert_eq!(snapshot(&a)["value"], snapshot(&b)["value"]);
    assert!(
        !a.can_undo() && !b.can_undo(),
        "external changes form an undo boundary"
    );
}
