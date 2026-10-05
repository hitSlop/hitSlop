//! Palette edits use the document's history, sequence, publications and saved bytes.
use hitslop_core::{AppSpec, Applied, Document, Origin};
use serde_json::{Value, json};
mod support;
use support::{View, snapshot};

const SCHEMA: &str =
    r#"{"kind":"object","properties":{"title":{"kind":"string"},"hits":{"kind":"counter"}}}"#;
const INITIAL: &str = r#"{"title":"Initial","hits":0}"#;
const DEFAULTS: &str = r##"{"accent":"#335577","paper":"#ffffff"}"##;
fn app() -> AppSpec {
    AppSpec::new(SCHEMA, "palette-test", DEFAULTS).unwrap()
}
fn doc() -> Document {
    Document::create(&app(), INITIAL).unwrap()
}
fn color(doc: &Document) -> Value {
    snapshot(doc)["theme"]["accent"].clone()
}
fn palette(doc: &mut Document, values: Value, origin: Origin) -> Applied {
    let batch = json!({"intents":[{"type":"setTheme","values":values}]});
    doc.apply_batch(&batch.to_string(), origin).unwrap()
}
/// A color panel step: the window sets the accent.
fn set(doc: &mut Document, color: &str) -> Applied {
    palette(doc, json!({ "accent": color }), Origin::Window)
}

#[test]
fn theme_only_edits_publish_under_the_document_sequence_and_reopen() {
    let mut doc = doc();
    let mut view = View::of(&doc);
    let before = snapshot(&doc);
    let change = set(&mut doc, "#123456");
    assert_eq!(change.sequence, 1);
    assert!(change.theme_changed);
    let publication = change.publication.unwrap();
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
    let reopened = Document::open(&app(), &doc.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["theme"], snapshot(&doc)["theme"]);
    assert_eq!(reopened.version(), doc.version());
    assert!(
        !reopened.can_undo(),
        "history is scoped to the open session"
    );
}

#[test]
fn a_batch_reports_whether_it_changed_the_palette() {
    let mut doc = doc();
    let data = r#"{"intents":[{"type":"increment","path":["hits"],"by":1}]}"#;
    assert!(!doc.apply_batch(data, Origin::Agent).unwrap().theme_changed);
    let both = json!({"intents":[
        {"type":"set","path":["title"],"value":"Both"},
        {"type":"setTheme","values":{"paper":"#eeeeee"}},
    ]});
    let applied = doc.apply_batch(&both.to_string(), Origin::Agent).unwrap();
    assert!(applied.theme_changed, "data and palette change atomically in one batch");
    assert!(doc.undo().unwrap().theme_changed);
    assert!(!palette(&mut doc, json!({ "paper": "#ffffff" }), Origin::Window).theme_changed, "a no-op");
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
    let reset = r#"{"intents":[{"type":"setTheme","values":{},"replace":true}]}"#;
    assert!(doc.apply_batch(reset, Origin::Window).unwrap().publication.is_none());
    assert!(set(&mut doc, "#335577").publication.is_none());
    assert!(!doc.can_undo());
    set(&mut doc, "#123456");
    doc.undo().unwrap();
    let before = snapshot(&doc);
    assert!(set(&mut doc, "#335577").publication.is_none());
    let refused = json!({"intents":[{"type":"setTheme","values":{"accent":"#111111","missing":"#000000"}}]});
    assert!(doc.apply_batch(&refused.to_string(), Origin::Window).is_err());
    assert_eq!(snapshot(&doc), before);
    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert_eq!(color(&doc), "#123456");
}

// A color panel sends one change per step of a drag, so the window's consecutive changes
// to one color are one undo step; a reset, another color, an import or an agent's change
// is a step of its own.
#[test]
fn a_run_of_window_changes_to_one_color_is_one_undo_step() {
    let mut doc = doc();
    set(&mut doc, "#111111");
    assert!(set(&mut doc, "#111111").publication.is_none());
    set(&mut doc, "#222222");
    palette(&mut doc, json!({ "paper": "#eeeeee" }), Origin::Window);
    set(&mut doc, "#333333");
    palette(&mut doc, json!({ "accent": "#444444" }), Origin::Agent);
    palette(&mut doc, json!({ "accent": null }), Origin::Window);
    set(&mut doc, "#555555");
    let file = json!({"template":"palette-test","values":{"accent":"#666666"}}).to_string();
    doc.apply_batch(&json!({"intents":[{"type":"importTheme","file":file}]}).to_string(), Origin::Window).unwrap();
    for (accent, paper) in [
        ("#555555", "#eeeeee"),
        ("#335577", "#eeeeee"),
        ("#444444", "#eeeeee"),
        ("#333333", "#eeeeee"),
        ("#222222", "#eeeeee"),
        ("#222222", "#ffffff"),
        ("#335577", "#ffffff"),
    ] {
        doc.undo().unwrap();
        assert_eq!((color(&doc), snapshot(&doc)["theme"]["paper"].clone()), (json!(accent), json!(paper)));
    }
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(color(&doc), "#222222");
}

#[test]
fn data_between_color_updates_ends_the_theme_undo_run() {
    let mut doc = doc();
    set(&mut doc, "#111111");
    doc.apply_batch(
        r#"{"intents":[{"type":"increment","path":["hits"],"by":2}]}"#,
        Origin::Agent,
    )
    .unwrap();
    set(&mut doc, "#222222");
    doc.undo().unwrap();
    assert_eq!(color(&doc), "#111111");
    assert_eq!(snapshot(&doc)["value"]["hits"], 2);
    doc.undo().unwrap();
    assert_eq!(snapshot(&doc)["value"]["hits"], 0);
    assert_eq!(color(&doc), "#111111");
}
