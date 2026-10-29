//! Engine acceptance belongs to Rust; no generated JSON Schema runs beside it.
use serde_json::{Value, json};
use tinys_core::{EngineReply, EngineRequest};

fn accepts(value: Value) -> bool {
    EngineRequest::parse(&value.to_string()).is_ok()
}

#[test]
fn engine_request_shapes_are_closed_and_optionals_are_not_nullable() {
    assert!(EngineRequest::parse(r#"{"method":"get","documentPath":"x","documentPath":"y"}"#).is_err());
    assert!(accepts(json!({"method":"get","documentPath":"-Document with spaces.tiny"})));
    for value in [
        json!({"method":"get","documentPath":"x","protocol":1}),
        json!({"method":"get"}),
        json!({"method":"future"}),
        json!({"method":"batch","documentPath":"x","batch":{"intents":[],"ifVersion":null}}),
        json!({"method":"batch","documentPath":"x","batch":{"intents":[]},"command":"rename"}),
        json!({"method":"batch","documentPath":"x","batch":{"intents":[]},"attachments":null}),
        json!({"method":"artwork.export","file":"x","output":"y","target":"other"}),
    ] {
        assert!(!accepts(value.clone()), "{value}");
    }
}

#[test]
fn paths_and_command_names_keep_their_bounds() {
    assert!(accepts(json!({"method":"get","documentPath":"😀".repeat(4096)})));
    for path in [String::new(), "x".repeat(4097)] {
        for value in [
            json!({"method":"get","documentPath":path}),
            json!({"method":"create","from":path,"output":"x"}),
            json!({"method":"pack","stage":"x","file":path}),
            json!({"method":"artwork.export","file":"x","output":path,"target":"icon"}),
        ] {
            assert!(!accepts(value.clone()), "{value}");
        }
    }
    for name in ["addTask".to_owned(), "a".repeat(80)] {
        assert!(accepts(json!({"method":"call","documentPath":"x","command":name,"args":{}})));
    }
    for name in ["", "Add", "a-b", "a_b", "aé", &"a".repeat(81)] {
        assert!(!accepts(json!({"method":"call","documentPath":"x","command":name,"args":{}})));
        assert!(!accepts(json!({"method":"batch","documentPath":"x","command":name,"batch":{"intents":[]}})));
    }
}

/// Only `pack` takes `renderViews`: a boolean, false when absent.
#[test]
fn only_pack_renders_views() {
    for (flag, expected) in [(None, false), (Some(json!(false)), false), (Some(json!(true)), true)] {
        let mut request = json!({"method":"pack","stage":"x","file":"y","app":{}});
        if let Some(flag) = flag {
            request["renderViews"] = flag;
        }
        let Ok(EngineRequest::Pack { render_views, .. }) = EngineRequest::parse(&request.to_string()) else {
            panic!("{request}")
        };
        assert_eq!(render_views, expected, "{request}");
    }
    for value in [
        json!({"method":"pack","stage":"x","file":"y","app":{},"renderArtwork":true}),
        json!({"method":"pack","stage":"x","file":"y","app":{},"renderViews":null}),
        json!({"method":"pack","stage":"x","file":"y","app":{},"renderViews":"true"}),
        json!({"method":"validateApp","stage":"x","app":{},"renderViews":true}),
        json!({"method":"inspect","file":"x","renderViews":true}),
    ] {
        assert!(!accepts(value.clone()), "{value}");
    }
}

#[test]
fn attachment_identity_and_payload_limits_are_explicit() {
    assert!(accepts(json!({"method":"attachments.read","documentPath":"x","attachmentID":"a".repeat(64)})));
    for id in ["a".repeat(63), "a".repeat(65), "A".repeat(64), "g".repeat(64)] {
        assert!(!accepts(json!({"method":"attachments.read","documentPath":"x","attachmentID":id})));
    }
    assert!(!accepts(json!({"method":"batch","documentPath":"x","batch":{"intents":[]},"attachments":[]})));
    let max = tinys_core::ATTACHMENT_FILE_BYTES.div_ceil(3) * 4;
    assert!(accepts(
        json!({"method":"batch","documentPath":"x","batch":{"intents":[]},"attachments":["a".repeat(max)]})
    ));
    assert!(!accepts(
        json!({"method":"batch","documentPath":"x","batch":{"intents":[]},"attachments":["a".repeat(max+1)]})
    ));
    for attachments in [None, Some(vec!["YWJj"])] {
        let request = |value: String| {
            json!({"method":"batch","documentPath":"x",
            "batch":{"intents":[{"type":"set","path":["title"],"value":value}]},
            "attachments": attachments})
        };
        let mut small = request("😀".repeat(100));
        let mut large = request("😀".repeat(262_144));
        if attachments.is_none() {
            small.as_object_mut().unwrap().remove("attachments");
            large.as_object_mut().unwrap().remove("attachments");
        }
        assert!(accepts(small));
        assert!(!accepts(large));
    }
    assert!(EngineRequest::parse(&" ".repeat(tinys_core::command::MAX_REQUEST_BYTES + 1)).is_err());
}

#[test]
fn future_app_payload_reaches_its_marker_check_unchanged() {
    let app =
        r#"{ "packageFormat":999,"runtimeABI":1,"manifest":{"future":1e999},"descriptor":{},"initial":{},"theme":{} }"#;
    let input = format!(r#"{{"method":"validateApp","stage":"/not-read","app":{app}}}"#);
    let EngineRequest::ValidateApp { app: raw, .. } = EngineRequest::parse(&input).unwrap() else { panic!() };
    assert_eq!(raw.get(), app);
    assert!(matches!(tinys_core::file::validate_app(raw.get(), std::path::Path::new("/not-read")),
        Err(tinys_core::store::Error::Rejected(error)) if error.code == tinys_core::Code::RequiresUpdate));
}

#[test]
fn future_app_markers_precede_current_required_and_unknown_fields() {
    for app in [
        r#"{"packageFormat":999,"runtimeABI":1}"#,
        r#"{"packageFormat":999,"runtimeABI":1,"futureField":1e999}"#,
        r#"{"packageFormat":1,"runtimeABI":999,"futureField":{"new":true}}"#,
    ] {
        let input = format!(r#"{{"method":"validateApp","stage":"/not-read","app":{app}}}"#);
        let EngineRequest::ValidateApp { app, stage } = EngineRequest::parse(&input).unwrap() else { panic!() };
        let error = tinys_core::file::validate_app(app.get(), std::path::Path::new(&stage)).unwrap_err();
        assert!(
            matches!(&error, tinys_core::store::Error::Rejected(inner)
            if inner.code == tinys_core::Code::RequiresUpdate),
            "{error}"
        );
    }
}

#[test]
fn successes_require_their_results_and_literal_discriminants() {
    for value in [
        json!({"ok":true,"method":"create"}),
        json!({"ok":true,"method":"templates","catalog":{}}),
        json!({"ok":true,"method":"describe","state":{}}),
        json!({"ok":true,"method":"artwork.export"}),
        json!({"ok":true,"method":"pack"}),
        json!({"ok":true,"method":"pack","artwork":{"preview":"rendered"}}),
        json!({"ok":true,"method":"pack","artwork":{"preview":"rendered","icon":"none"}}),
        json!({"ok":false,"method":"batch","ids":[]}),
        json!({"ok":true,"method":"batch","ids":[],"extra":1}),
        json!({"ok":true,"method":"call","ids":[]}),
        json!({"ok":false,"code":"rejected","error":"x","reason":null}),
        json!({"ok":false,"code":"rejected","error":"x","opIndex":null}),
    ] {
        assert!(serde_json::from_value::<EngineReply>(value.clone()).is_err(), "{value}");
    }
    for value in [
        json!({"ok":true,"method":"artwork.export","output":null}),
        json!({"ok":true,"method":"pack","artwork":{"preview":"rendered","icon":"absent"}}),
        json!({"ok":true,"method":"pack","artwork":{"preview":"supplied","icon":"supplied"}}),
        json!({"ok":true,"method":"call","result":null,"ids":[]}),
        json!({"ok":false,"code":"unknown_outcome","error":"Lost reply"}),
    ] {
        assert!(serde_json::from_value::<EngineReply>(value.clone()).is_ok(), "{value}");
    }
}

#[test]
fn the_permanent_protocol_refusal_keeps_its_exact_bytes() {
    let error = tinys_core::Error {
        code: tinys_core::Code::RequiresUpdate,
        message: "Update tinys".into(),
        op_index: None,
        path: vec![],
    }
    .into();
    assert_eq!(
        tinys_core::command::failure(error, false, false),
        r#"{"ok":false,"error":"Update tinys","code":"rejected","reason":"requires_update"}"#
    );
}
