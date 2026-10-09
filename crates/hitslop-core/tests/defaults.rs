mod support;
use hitslop_core::{AppSpec, Code, Document, Origin};
use serde_json::{Value, json};
use support::ApplyJson;

fn schema() -> Value {
    json!({"kind":"object","properties":{
        "title":{"kind":"text"},
        "count":{"kind":"counter"},
        "tags":{"kind":"record","value":{"kind":"string"}},
        "settings":{"kind":"object","properties":{"dark":{"kind":"boolean","default":true}}},
        "tasks":{"kind":"list","item":{"kind":"object","properties":{
            "text":{"kind":"text"},
            "done":{"kind":"boolean","default":false},
            "priority":{"kind":"enum","values":["low","high"],"default":"low"},
            "size":{"kind":"integer","min":1,"default":1},
            "note":{"kind":"optional","inner":{"kind":"string"}}
        }}},
        "notes":{"kind":"record","value":{"kind":"object","properties":{
            "body":{"kind":"text"},"pinned":{"kind":"boolean","default":false}
        }}},
        "extra":{"kind":"optional","inner":{"kind":"object","properties":{
            "label":{"kind":"string","default":"Untitled"}
        }}}
    }})
}
fn value(document: &Document) -> Value {
    serde_json::from_str::<Value>(&document.state().unwrap()).unwrap()["value"].clone()
}

#[test]
fn omitted_fields_take_their_defaults_when_created() {
    let app = AppSpec::data(&schema().to_string()).unwrap();
    let initial = json!({"tasks":[{"text":"Walk"}]}).to_string();
    hitslop_core::validate(&schema().to_string(), &initial).unwrap();
    let document = Document::create(&app, &initial).unwrap();
    let state = value(&document);
    assert_eq!(state["title"], "");
    assert_eq!(state["count"], 0);
    assert_eq!(state["tags"], json!({}));
    assert_eq!(state["settings"], json!({"dark":true}));
    assert_eq!(state["tasks"][0]["done"], false);
    assert_eq!(state["tasks"][0]["priority"], "low");
    assert_eq!(state["tasks"][0]["size"], 1);
    assert!(state["tasks"][0].get("note").is_none());
    assert!(state.get("extra").is_none());
}

#[test]
fn inserts_and_absent_puts_and_sets_fill_omitted_fields() {
    let app = AppSpec::data(&schema().to_string()).unwrap();
    let mut document = Document::create(&app, "{}").unwrap();
    document
        .apply_json(
            &json!({"intents":[
                {"type":"insert","path":["tasks"],"id":"a","value":{"done":true}},
                {"type":"set","path":["notes","n"],"value":{}},
                {"type":"set","path":["extra"],"value":{}}
            ]})
            .to_string(),
            Origin::Page,
        )
        .unwrap();
    let state = value(&document);
    assert_eq!(state["tasks"][0], json!({"$id":"a","text":"","done":true,"priority":"low","size":1}));
    assert_eq!(state["notes"]["n"], json!({"body":"","pinned":false}));
    assert_eq!(state["extra"], json!({"label":"Untitled"}));
}

#[test]
fn replacement_never_fills_creation_defaults() {
    let app = AppSpec::data(&schema().to_string()).unwrap();
    let initial = json!({"title":"Keep", "tasks":[{"$id":"a","text":"Keep task","done":true}],
        "notes":{"n":{"body":"Keep note","pinned":true}}, "extra":{"label":"Keep label"}});
    for (operation, path, supplied, pointer) in [
        ("replace", json!([]), json!({}), "/count"),
        ("replace", json!(["tasks",{"id":"a"}]), json!({"text":"Again"}), "/done"),
        ("replace", json!(["settings"]), json!({}), "/dark"),
        ("replace", json!(["tasks"]), json!([{"text":"New"}]), "/0/done"),
        ("set", json!(["extra"]), json!({}), "/label"),
        ("set", json!(["notes", "n"]), json!({"pinned":false}), "/body"),
    ] {
        let mut document = Document::create(&app, &initial.to_string()).unwrap();
        let before = document.state().unwrap();
        let error = document
            .apply_json(&json!({"intents":[{"type":operation,"path":path,"value":supplied}]}).to_string(), Origin::Page)
            .expect_err("an incomplete replacement must not reset existing fields");
        assert_eq!((error.code, error.pointer()), (Code::TypeMismatch, pointer.into()));
        assert_eq!(document.state().unwrap(), before, "refusal preserves state and version");
        assert!(!document.can_undo());
    }
}

#[test]
fn fields_without_defaults_are_still_required() {
    let schema = json!({"kind":"object","properties":{
        "rows":{"kind":"list","item":{"kind":"object","properties":{"name":{"kind":"string"}}}},
        "box":{"kind":"object","properties":{"size":{"kind":"number"}}}
    }});
    let error = hitslop_core::validate(&schema.to_string(), r#"{"rows":[]}"#).unwrap_err();
    assert_eq!((error.code, error.pointer()), (Code::TypeMismatch, "/box".into()));
    let app = AppSpec::data(&schema.to_string()).unwrap();
    let mut document = Document::create(&app, r#"{"box":{"size":1}}"#).unwrap();
    let error =
        document.apply_json(r#"{"intents":[{"type":"insert","path":["rows"],"value":{}}]}"#, Origin::Page).unwrap_err();
    assert_eq!((error.code, error.pointer()), (Code::TypeMismatch, "/name".into()));
}

#[test]
fn explicit_invalid_values_are_never_replaced_by_creation_defaults() {
    let app = AppSpec::data(&schema().to_string()).unwrap();
    for supplied in [json!({"done":null}), json!({"done":"yes"}), json!({"size":0})] {
        assert!(Document::create(&app, &json!({"tasks":[supplied]}).to_string()).is_err());
        let mut document = Document::create(&app, "{}").unwrap();
        let before = document.state().unwrap();
        assert!(
            document
                .apply_json(
                    &json!({"intents":[{"type":"insert","path":["tasks"],"value":supplied}]}).to_string(),
                    Origin::Page,
                )
                .is_err()
        );
        assert_eq!(document.state().unwrap(), before);
    }
}

#[test]
fn defaults_must_fit_their_field_and_belong_to_object_fields() {
    for (node, reason) in [
        (json!({"kind":"integer","max":3,"default":5}), "out of bounds"),
        (json!({"kind":"boolean","default":"yes"}), "wrong kind"),
        (json!({"kind":"enum","values":["a"],"default":"b"}), "not a value"),
        (json!({"kind":"string","default":null}), "explicit null"),
        (json!({"kind":"optional","inner":{"kind":"string","default":"x"}}), "optional"),
        (json!({"kind":"list","item":{"kind":"boolean","default":true}}), "list element"),
        (json!({"kind":"record","value":{"kind":"integer","default":0}}), "record entry"),
    ] {
        let schema = json!({"kind":"object","properties":{"value":node}});
        assert!(AppSpec::data(&schema.to_string()).is_err(), "{reason}: {schema}");
    }
}
