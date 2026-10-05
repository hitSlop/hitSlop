use hitslop_core::Document;
use serde_json::{json, Value};

/// Whether authoring accepts the descriptor, with initial values that fit it, so only the
/// descriptor can refuse.
fn accepts(schema: &Value) -> bool {
    fn fitting(node: &Value) -> Value {
        match node["kind"].as_str().unwrap() {
            "object" => Value::Object(node["properties"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), fitting(v))).collect()),
            "string" | "text" => json!(""),
            "boolean" => json!(false),
            _ => json!(0),
        }
    }
    hitslop_core::validate(&schema.to_string(), &fitting(schema).to_string()).is_ok()
}

#[test]
fn explicit_null_bounds_are_not_silently_treated_as_absent() {
    for node in [json!({"kind":"string","maxLength":null}), json!({"kind":"number","min":null}), json!({"kind":"integer","max":null})] {
        let schema = json!({"kind":"object","properties":{"value":node}});
        assert!(!accepts(&schema), "{schema}");
    }
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
        assert!(!accepts(&schema), "{schema}");
    }
}

// Boundaries of the descriptor rules: the limits themselves are accepted.
#[test]
fn descriptor_limits_accept_their_boundaries() {
    let mut node = json!({"kind":"string"});
    for _ in 0..16 { node = json!({"kind":"object","properties":{"child":node}}); }
    assert!(accepts(&node), "depth 16");
    let wide = |n: usize| json!({"kind":"object","properties":(0..n).map(|i| (format!("f{i}"), json!({"kind":"boolean"}))).collect::<serde_json::Map<_, _>>()});
    assert!(accepts(&wide(1024)), "1024 fields");
    assert!(!accepts(&wide(1025)), "1025 fields");
    for key in ["constructor", "prototype"] {
        let schema = json!({"kind":"object","properties":{key:{"kind":"string"}}});
        assert!(!accepts(&schema), "{key}");
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
