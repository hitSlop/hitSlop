//! The helper is a renderer, not another document-edit boundary.
use hitslop_core::native::{NativeReply, NativeRequest};
use hitslop_core::{Code, OutcomeCode};
use serde_json::json;

fn decode(value: serde_json::Value) -> Result<NativeRequest, NativeReply> {
    NativeRequest::decode(value.to_string().as_bytes())
}
fn invalid(bytes: &[u8], reason: Code) {
    let NativeReply::Failure { code, reason: actual, .. } = NativeRequest::decode(bytes).unwrap_err() else {
        panic!("expected a refusal")
    };
    assert_eq!(code, OutcomeCode::Rejected);
    assert_eq!(actual, Some(reason));
}

#[test]
fn only_complete_native_requests_are_accepted() {
    for request in [
        json!({"method":"open","documentPath":"-A document.slop"}),
        json!({"method":"export","documentPath":"x.slop","format":"pdf","output":"x.pdf"}),
        json!({"method":"screenshot","documentPath":"x.slop","target":"icon","output":"x.png","ifPresent":true}),
    ] {
        assert!(decode(request.clone()).is_ok());
        for field in request.as_object().unwrap().keys() {
            let mut missing = request.clone();
            missing.as_object_mut().unwrap().remove(field);
            invalid(missing.to_string().as_bytes(), Code::InvalidRequest);
            let mut null = request.clone();
            null[field] = serde_json::Value::Null;
            invalid(null.to_string().as_bytes(), Code::InvalidRequest);
        }
        let mut extra = request.clone();
        extra["unexpected"] = json!(true);
        invalid(extra.to_string().as_bytes(), Code::InvalidRequest);
    }
    for input in [
        r#"{"method":"get","documentPath":"x.slop"}"#,
        r#"{"method":"open","method":"open","documentPath":"x.slop"}"#,
        r#"{"method":"open","documentPath":"x.slop","documentPath":"y.slop"}"#,
        r#"{"method":"export","documentPath":"x.slop","output":"x.jpg","format":"jpg"}"#,
        r#"{"method":"screenshot","documentPath":"x.slop","output":"x.png","target":"unknown","ifPresent":true}"#,
        r#"{"method":"screenshot","documentPath":"x.slop","output":"x.png","target":"icon","ifPresent":1}"#,
        "{}",
        "[]",
        "null",
        "not JSON",
    ] {
        invalid(input.as_bytes(), Code::InvalidRequest);
    }
}

#[test]
fn native_paths_and_input_are_bounded_before_host_work() {
    for size in [1, 4096] {
        assert!(decode(json!({"method":"open","documentPath":"🦀".repeat(size)})).is_ok());
    }
    for size in [0, 4097] {
        for request in [
            json!({"method":"open","documentPath":"x".repeat(size)}),
            json!({"method":"export","documentPath":"x.slop","output":"x".repeat(size),"format":"png"}),
            json!({"method":"screenshot","documentPath":"x.slop","output":"x".repeat(size),"target":"preview","ifPresent":false}),
        ] {
            invalid(request.to_string().as_bytes(), Code::InvalidRequest);
        }
    }
    let mut request = br#"{"method":"open","documentPath":"x.slop"}"#.to_vec();
    request.resize(1024 * 1024, b' ');
    assert!(NativeRequest::decode(&request).is_ok());
    request.push(b' ');
    invalid(&request, Code::TooLarge);
    invalid(&[0xff], Code::InvalidRequest);
}

#[test]
fn native_replies_share_the_engine_shape_and_keep_nullable_output() {
    for (reply, expected) in [
        (
            NativeReply::Open { document_path: "x.slop".into() },
            json!({"ok":true,"method":"open","documentPath":"x.slop"}),
        ),
        (NativeReply::Screenshot { output: None }, json!({"ok":true,"method":"screenshot","output":null})),
        (NativeReply::Export { output: "x.pdf".into() }, json!({"ok":true,"method":"export","output":"x.pdf"})),
        (
            NativeReply::Failure {
                error: "update".into(),
                code: OutcomeCode::Rejected,
                reason: Some(Code::RequiresUpdate),
                op_index: None,
            },
            json!({"ok":false,"error":"update","code":"rejected","reason":"requires_update"}),
        ),
        (
            NativeReply::Failure {
                error: "bad op".into(),
                code: OutcomeCode::Rejected,
                reason: Some(Code::InvalidRequest),
                op_index: Some(2),
            },
            json!({"ok":false,"error":"bad op","code":"rejected","reason":"invalid_request","opIndex":2}),
        ),
    ] {
        assert_eq!(serde_json::from_str::<serde_json::Value>(&reply.to_json()).unwrap(), expected);
        let decoded: NativeReply = serde_json::from_value(expected).unwrap();
        assert_eq!(decoded.to_json(), reply.to_json());
    }
    for input in [
        json!({"ok":true,"method":"get","state":{}}),
        json!({"ok":true,"method":"pack"}),
        json!({"ok":true,"method":"screenshot"}),
        json!({"ok":true,"method":"open","documentPath":""}),
        json!({"ok":true,"method":"export","output":null}),
        json!({"ok":false,"code":"rejected"}),
        json!({"ok":false,"code":"guess","error":"unknown"}),
        json!({"ok":false,"code":"rejected","error":"bad","reason":null}),
    ] {
        assert!(serde_json::from_value::<NativeReply>(input.clone()).is_err(), "{input}");
    }
}
