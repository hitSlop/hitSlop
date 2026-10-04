use hitslop_core::Document;
use serde_json::json;

#[test]
fn explicit_null_bounds_are_not_silently_treated_as_absent() {
    for node in [json!({"kind":"string","maxLength":null}), json!({"kind":"number","min":null}), json!({"kind":"integer","max":null})] {
        let schema = json!({"kind":"object","properties":{"value":node}});
        assert!(hitslop_core::canonical_descriptor(&schema.to_string()).is_err(), "{schema}");
    }
}

#[test]
fn schema_identity_normalizes_key_order_and_number_spelling() {
    let a = r#"{"kind":"object","properties":{"value":{"kind":"number","min":1.0,"max":2e0}}}"#;
    let b = r#"{"properties":{"value":{"max":2,"min":1,"kind":"number"}},"kind":"object"}"#;
    let key = hitslop_core::canonical_descriptor(a).unwrap();
    assert_eq!(key, hitslop_core::canonical_descriptor(b).unwrap());
    assert_eq!(key, hitslop_core::canonical_descriptor(&key).unwrap());
}

#[test]
fn descriptor_rules_protect_public_sdk_handles() {
    for key in ["path", "node", "with-hyphen", "0first"] {
        let schema = json!({"kind":"object","properties":{key:{"kind":"string"}}});
        assert_eq!(Document::create(&schema.to_string(), &json!({key:""}).to_string()).err().unwrap().code.as_str(), "invalid_schema", "{key}");
    }
    for key in ["set", "clear"] {
        let schema = json!({"kind":"object","properties":{
            "value":{"kind":"optional","inner":{"kind":"object","properties":{key:{"kind":"string"}}}}
        }});
        assert_eq!(Document::create(&schema.to_string(), "{}").err().unwrap().code.as_str(), "invalid_schema", "{key}");
    }
}

#[test]
fn descriptor_depth_is_bounded_before_initial_values_are_checked() {
    let mut node = json!({"kind":"string"});
    for _ in 0..18 { node = json!({"kind":"object","properties":{"child":node}}); }
    let schema = json!(node);
    assert_eq!(Document::create(&schema.to_string(), "{}").err().unwrap().code.as_str(), "too_large");
}

// Failure: serde ignores unknown fields on unit variants of an internally tagged enum,
// so options on text, boolean and counter descriptors were silently accepted.
#[test]
fn unknown_options_are_refused_on_every_kind() {
    for node in [
        json!({"kind":"text","maxLength":5}),
        json!({"kind":"boolean","default":true}),
        json!({"kind":"counter","min":0}),
        json!({"kind":"string","max":3}),
    ] {
        let schema = json!({"kind":"object","properties":{"value":node}});
        assert!(hitslop_core::canonical_descriptor(&schema.to_string()).is_err(), "{schema}");
    }
}

// Boundaries of the descriptor rules: the limits themselves are accepted.
#[test]
fn descriptor_limits_accept_their_boundaries() {
    let mut node = json!({"kind":"string"});
    for _ in 0..16 { node = json!({"kind":"object","properties":{"child":node}}); }
    assert!(hitslop_core::canonical_descriptor(&node.to_string()).is_ok(), "depth 16");
    let wide = |n: usize| json!({"kind":"object","properties":(0..n).map(|i| (format!("f{i}"), json!({"kind":"boolean"}))).collect::<serde_json::Map<_, _>>()});
    assert!(hitslop_core::canonical_descriptor(&wide(1024).to_string()).is_ok(), "1024 fields");
    assert!(hitslop_core::canonical_descriptor(&wide(1025).to_string()).is_err(), "1025 fields");
    for key in ["constructor", "prototype"] {
        let schema = json!({"kind":"object","properties":{key:{"kind":"string"}}});
        assert!(hitslop_core::canonical_descriptor(&schema.to_string()).is_err(), "{key}");
    }
}

// Every document records the layout it was written in. A build refuses bytes with no
// layout before interpreting them, and a document from a newer layout asks for an update,
// whether the newer layout arrives in the checkpoint or in a saved update.
#[test]
fn documents_record_their_layout_and_a_newer_one_asks_for_an_update() {
    use hitslop_core::{Code, LAYOUT};
    use loro::{ExportMode, LoroDoc};
    let schema = r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#;
    let checkpoint = Document::create(schema, r#"{"title":"A"}"#).unwrap().checkpoint().unwrap();
    let inspected = LoroDoc::new();
    inspected.import(&checkpoint).unwrap();
    assert_eq!(serde_json::to_value(inspected.get_map("meta").get_deep_value()).unwrap(), json!({"layout": LAYOUT}));
    assert!(Document::open(schema, &checkpoint, &[]).is_ok());

    let since = inspected.oplog_vv();
    inspected.get_map("meta").insert("layout", LAYOUT + 1).unwrap();
    inspected.commit();
    let newer = inspected.export(ExportMode::Snapshot).unwrap();
    let newer_update = inspected.export(ExportMode::updates(&since)).unwrap();
    assert_eq!(Document::open(schema, &newer, &[]).err().map(|e| e.code), Some(Code::RequiresUpdate));
    assert_eq!(Document::open(schema, &checkpoint, &[newer_update]).err().map(|e| e.code), Some(Code::RequiresUpdate));

    let bare = LoroDoc::new();
    bare.get_map("data").insert("title", "A").unwrap();
    bare.commit();
    let bare = bare.export(ExportMode::Snapshot).unwrap();
    assert_eq!(Document::open(schema, &bare, &[]).err().map(|e| e.code), Some(Code::InvalidBytes));
}
