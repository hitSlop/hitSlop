#![cfg(feature = "storage")]
//! Routing checks live beside the serde types, not in a second JSON Schema decoder.
use hitslop_core::{
    Code, EngineReply,
    socket_wire::{Discovery, SocketRequest},
};
use serde_json::{Value, json};

fn decode(value: Value) -> Result<SocketRequest, hitslop_core::Error> {
    SocketRequest::decode(&value.to_string())
}
fn batch() -> Value {
    json!({"protocol":1,"method":"batch","documentPath":"/tmp/doc.slop","ops":"[]"})
}

#[test]
fn requests_check_routing_but_leave_intents_to_the_owner() {
    for method in ["get", "attachments.list", "theme.export"] {
        assert!(decode(json!({"protocol":1,"method":method,"documentPath":"/tmp/doc.slop"})).is_ok());
    }
    assert!(decode(json!({"protocol":1,"method":"export","documentPath":"/tmp/doc.slop","format":"pdf","output":"/tmp/doc.pdf"})).is_ok());
    let mut request = batch();
    request["ops"] = "[{\"type\":\"unknown-operation\"}]".into();
    assert!(decode(request).is_ok());
    for request in [
        json!({"protocol":1,"method":"unknown","documentPath":"/tmp/doc.slop"}),
        json!({"protocol":1,"method":"get","documentPath":"/tmp/doc.slop","output":"/tmp/doc.pdf"}),
        json!({"method":"batch","documentPath":"/tmp/doc.slop","ops":"[]"}),
        json!({"protocol":1,"method":"export","documentPath":"/tmp/doc.slop","format":"html","output":"/tmp/doc.html"}),
    ] {
        assert!(decode(request.clone()).is_err(), "{request}");
    }
}

#[test]
fn request_constraints_survive_the_schema_removal() {
    for (field, values) in [
        ("documentPath", vec![json!(""), json!("x".repeat(4097)), Value::Null]),
        ("ops", vec![json!(""), json!("x".repeat(1_048_577)), json!({}), Value::Null]),
        ("command", vec![json!(""), json!("Invalid"), json!("two-words"), json!("x".repeat(81)), Value::Null]),
        ("base", vec![Value::Null, json!(true)]),
        ("ifVersion", vec![Value::Null, json!(true)]),
        (
            "attachments",
            vec![Value::Null, json!([]), json!([false]), json!(["x".repeat(10_485_760_usize.div_ceil(3) * 4 + 1)])],
        ),
    ] {
        for value in values {
            let mut request = batch();
            request[field] = value;
            assert!(decode(request).is_err(), "{field}");
        }
    }
    for id in ["".into(), "a".repeat(63), "a".repeat(65), "A".repeat(64), "g".repeat(64)] {
        assert!(
            decode(json!({"protocol":1,"method":"attachments.read","documentPath":"/tmp/doc.slop","attachmentID":id}))
                .is_err()
        );
    }
    assert!(decode(json!({"protocol":1,"method":"attachments.read","documentPath":"/tmp/doc.slop","attachmentID":"a".repeat(64)})).is_ok());
    let mut request = batch();
    request["command"] = json!("rename2");
    request["attachments"] = json!(["YWJj"]);
    assert!(decode(request).is_ok());
    assert_eq!(SocketRequest::decode(&" ".repeat(16_777_217)).unwrap_err().code, Code::InvalidRequest);
}

#[test]
fn protocol_precedes_fields_and_discovery_ignores_future_fields() {
    for version in [0, 2, 9999] {
        let input = format!(r#"{{"protocol":{version},"method":{{"future":true}},"payload":1e999}}"#);
        assert_eq!(SocketRequest::decode(&input).unwrap_err().code, Code::RequiresUpdate);
    }
    let value = Discovery::decode(r#"{"socket":"/tmp/s","documentPath":"/tmp/a.slop","extra":1e999}"#).unwrap();
    assert_eq!(value.document_path, "/tmp/a.slop");
    assert_eq!(serde_json::to_string(&value).unwrap(), r#"{"documentPath":"/tmp/a.slop","socket":"/tmp/s"}"#);
    for input in [r#"{"documentPath":"/x"}"#, r#"{"documentPath":"/x","socket":""}"#, "{", ""] {
        assert!(Discovery::decode(input).is_err());
    }
    assert!(Discovery::decode(&" ".repeat(16_777_217)).is_err());
}

#[test]
fn socket_replies_share_the_engines_complete_results() {
    let replies = [
        json!({"ok":true,"method":"get","state":{"schema":{},"defaults":{"accent":"#335577"},"version":"v","value":{},"theme":{"accent":"#123456"}}}),
        json!({"ok":true,"method":"batch","ids":[]}),
        json!({"ok":true,"method":"export","output":"/tmp/doc.pdf"}),
        json!({"ok":true,"method":"theme.export","state":{"file":"{}"}}),
        json!({"ok":true,"method":"attachments.list","state":[{"id":"a".repeat(64),"byteLength":3,"mimeType":"application/octet-stream"}]}),
        json!({"ok":true,"method":"attachments.read","state":{"bytes":"YWJj"}}),
    ];
    for value in replies {
        assert!(serde_json::from_value::<EngineReply>(value.clone()).is_ok(), "{value}");
        for field in value.as_object().unwrap().keys() {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<EngineReply>(missing).is_err(), "{field} in {value}");
        }
        let mut extra = value.clone();
        extra["error"] = "contradictory success".into();
        assert!(serde_json::from_value::<EngineReply>(extra).is_err());
    }
    for code in ["save_failed", "unknown_outcome"] {
        assert!(serde_json::from_value::<EngineReply>(json!({"ok":false,"code":code,"error":"Disconnected"})).is_ok());
    }
    for reply in [json!({"ok":false,"error":"Disconnected"}), json!({"ok":true,"method":"batch","ids":[],"sequence":3})]
    {
        assert!(serde_json::from_value::<EngineReply>(reply).is_err());
    }
}
