use hitslop_core::Document;
use serde_json::json;

#[test]
fn explicit_null_bounds_are_not_silently_treated_as_absent() {
    for node in [json!({"kind":"string","maxLength":null}), json!({"kind":"number","min":null}), json!({"kind":"integer","max":null})] {
        let schema = json!({"kind":"object","properties":{"value":node}});
        assert!(hitslop_core::schema_key(&schema.to_string()).is_err(), "{schema}");
    }
}

#[test]
fn schema_identity_normalizes_key_order_and_number_spelling() {
    let a = r#"{"kind":"object","properties":{"value":{"kind":"number","min":1.0,"max":2e0}}}"#;
    let b = r#"{"properties":{"value":{"max":2,"min":1,"kind":"number"}},"kind":"object"}"#;
    let key = hitslop_core::schema_key(a).unwrap();
    assert_eq!(key, hitslop_core::schema_key(b).unwrap());
    assert_eq!(key, hitslop_core::schema_key(&key).unwrap());
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
        assert!(hitslop_core::schema_key(&schema.to_string()).is_err(), "{schema}");
    }
}

// Boundaries of the descriptor rules: the limits themselves are accepted.
#[test]
fn descriptor_limits_accept_their_boundaries() {
    let mut node = json!({"kind":"string"});
    for _ in 0..16 { node = json!({"kind":"object","properties":{"child":node}}); }
    assert!(hitslop_core::schema_key(&node.to_string()).is_ok(), "depth 16");
    let wide = |n: usize| json!({"kind":"object","properties":(0..n).map(|i| (format!("f{i}"), json!({"kind":"boolean"}))).collect::<serde_json::Map<_, _>>()});
    assert!(hitslop_core::schema_key(&wide(1024).to_string()).is_ok(), "1024 fields");
    assert!(hitslop_core::schema_key(&wide(1025).to_string()).is_err(), "1025 fields");
    for key in ["constructor", "prototype"] {
        let schema = json!({"kind":"object","properties":{key:{"kind":"string"}}});
        assert!(hitslop_core::schema_key(&schema.to_string()).is_err(), "{key}");
    }
}
