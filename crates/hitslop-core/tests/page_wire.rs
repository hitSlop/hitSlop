//! The Rust boundary owns request shapes and limits; Swift carries only typed host actions.
use hitslop_core::page_wire::{CaptureMode, HostCaptureResult, HostReply, HostRequest, PageRequest};
use serde_json::json;

#[test]
fn page_requests_and_nested_batches_are_strict() {
    for value in [
        json!({"method":"open"}),
        json!({"method":"config"}),
        json!({"method":"flush"}),
        json!({"method":"apply", "batch":{"intents":[]}}),
        json!({"method":"commands.run", "name":"addTask", "args":{"text":"task"}}),
        json!({"method":"window.resize", "width":240, "height":180}),
        json!({"method":"window.resize", "width":4096, "height":4096}),
        json!({"method":"pageError", "kind":"application", "error":""}),
        json!({"method":"failed", "error":"😀".repeat(2048)}),
    ] {
        assert!(PageRequest::decode(&value.to_string()).is_ok(), "{value}");
    }
    for value in [
        json!({"method":"open", "extra":true}),
        json!({"method":"config", "extra":true}),
        json!({"method":"attachments.list"}),
        json!({"method":"apply", "batch":{}}),
        json!({"method":"apply", "batch":""}),
        json!({"method":"apply", "batch":null}),
        json!({"method":"attachments.read", "attachmentID":"a".repeat(64)}),
        json!({"method":"commands.run", "name":"addTask"}),
        json!({"method":"commands.run", "name":"", "args":{}}),
        json!({"method":"window.resize", "width":239, "height":180}),
        json!({"method":"window.resize", "width":240, "height":179}),
        json!({"method":"window.resize", "width":4097, "height":180}),
        json!({"method":"window.resize", "width":240, "height":4097}),
        json!({"method":"window.resize", "width":240.5, "height":180}),
        json!({"method":"pageError", "kind":"other", "error":"x"}),
        json!({"method":"failed", "error":"😀".repeat(2049)}),
        json!({"method":"pageError", "kind":"operation", "error":"x".repeat(4097)}),
    ] {
        assert!(PageRequest::decode(&value.to_string()).is_err(), "{value}");
    }
    assert!(PageRequest::decode("{").is_err());
    assert!(PageRequest::decode("").is_err());
}

#[test]
fn requests_are_bounded_before_their_payloads_are_interpreted() {
    let batch = |value| {
        serde_json::from_value::<hitslop_core::Batch>(
            json!({"intents":[{"type":"set","path":["title"],"value":value}]}),
        )
        .unwrap()
    };
    let overhead = serde_json::to_string(&batch(String::new())).unwrap().len();
    assert!(PageRequest::Apply { batch: batch("x".repeat(4 * 1024 * 1024 - overhead)) }.check().is_ok());
    assert!(PageRequest::Apply { batch: batch("x".repeat(4 * 1024 * 1024 - overhead + 1)) }.check().is_err());
    let limit = (10_usize * 1024 * 1024).div_ceil(3) * 4;
    assert!(PageRequest::AttachmentsPut { bytes: "A".repeat(limit) }.check().is_ok());
    assert!(PageRequest::AttachmentsPut { bytes: "A".repeat(limit + 1) }.check().is_err());
    assert!(PageRequest::decode(&" ".repeat(16 * 1024 * 1024 + 1)).is_err());
}

#[test]
fn native_actions_use_the_same_serializer_and_capture_results_are_checked_in_rust() {
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&HostReply::Ready.to_json()).unwrap(),
        json!({"ok":true,"method":"ready"})
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &HostRequest::CaptureBegin { token: "one".into(), mode: CaptureMode::Export }.to_json()
        )
        .unwrap(),
        json!({"method":"capture.begin","token":"one","mode":"export"})
    );
    let refused: serde_json::Value =
        serde_json::from_str(&HostReply::WindowResize { width: f64::NAN, height: 3.0 }.to_json()).unwrap();
    assert_eq!(refused["ok"], false);
    let good = r#"{"x":-2,"y":0.5,"width":480,"height":360,"dedicated":true}"#;
    assert_eq!(HostCaptureResult::decode(good).unwrap().width, 480.0);
    for bad in [
        "{}",
        "null",
        r#"{"x":1e999,"y":0,"width":1,"height":1,"dedicated":false}"#,
        r#"{"x":0,"y":0,"width":1,"height":1,"dedicated":false,"extra":true}"#,
    ] {
        assert!(HostCaptureResult::decode(bad).is_err());
    }
    assert!(HostCaptureResult::decode(&" ".repeat(4097)).is_err());
}
