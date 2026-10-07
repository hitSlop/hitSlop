//! History-trimmed documents. Failure: a trimmed checkpoint opened with another value or
//! version, a refused batch rolled a trimmed document back to nothing, or a saved update
//! that is itself trimmed was accepted. Oracle: literal values and the full document.
mod support;
use hitslop_core::Origin;
use hitslop_core::{Code, Document};
use loro::{ExportMode, LoroDoc};
use serde_json::json;
use support::ApplyJson;
use support::{Edit, View, app, trimmed, value};

/// A document's full checkpoint and the same document trimmed to its latest version.
fn trimmed_document() -> (String, Vec<u8>, Vec<u8>) {
    let schema = json!({"kind":"object","properties":{"title":{"kind":"text"}}}).to_string();
    let mut source = Document::create(&app(&schema), r#"{"title":"initial"}"#).unwrap();
    for i in 0..20 {
        source
            .apply_json(
                &json!({"intents":[{"type":"set","path":["title"],"value":format!("edit {i}")}]}).to_string(),
                Origin::Page,
            )
            .unwrap();
    }
    let full = source.checkpoint().unwrap();
    let shallow = trimmed(&full);
    let inspected = LoroDoc::new();
    inspected.import(&shallow).unwrap();
    assert!(inspected.is_shallow(), "fixture must actually trim history");
    (schema, full, shallow)
}

#[test]
fn a_shallow_checkpoint_opens_with_its_value_and_version() {
    let (schema, full, shallow) = trimmed_document();
    let (full, trimmed) =
        (Document::open(&app(&schema), &full, &[]).unwrap(), Document::open(&app(&schema), &shallow, &[]).unwrap());
    assert_eq!(value(&trimmed), value(&full));
    assert_eq!(trimmed.version(), full.version());
}

// Failure: rollback replayed the document from its first operation, which a trimmed
// document no longer has; a rejected two-intent batch replaced the value with {}.
#[test]
fn a_refused_batch_on_a_trimmed_document_keeps_its_value() {
    let (schema, _, shallow) = trimmed_document();
    let mut owner = Document::open(&app(&schema), &shallow, &[]).unwrap();
    let mut view = View::of(&owner);
    let before = owner.state().unwrap();
    let refused = owner.apply(
        &json!({"intents":[
            {"type":"set","path":["title"],"value":"mutated"},
            {"type":"set","path":["missing"],"value":"refused"},
        ]})
        .to_string(),
    );
    assert_eq!(refused.err().map(|e| e.op_index), Some(Some(1)));
    assert_eq!(owner.state().unwrap(), before, "value, version and sequence stay unchanged");
    view.publish(
        &owner.apply(&json!({"intents":[{"type":"set","path":["title"],"value":"after"}]}).to_string()).unwrap(),
    );
    view.check(&owner, "after a refused batch");
    let reopened = Document::open(&app(&schema), &owner.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(value(&reopened), json!({"title":"after"}));
}

// Only the checkpoint may start history late.
#[test]
fn shallow_saved_update_is_refused() {
    let (schema, _, shallow) = trimmed_document();
    let empty = LoroDoc::new().export(ExportMode::Snapshot).unwrap();
    assert_eq!(Document::open(&app(&schema), &empty, &[shallow]).err().map(|e| e.code), Some(Code::InvalidBytes));
}
