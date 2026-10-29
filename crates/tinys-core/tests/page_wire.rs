#![cfg(feature = "storage")]
//! The Rust boundary owns request shapes and limits; Swift carries only typed host actions.
use serde_json::json;
use tinys_core::page_wire::{HostReply, HostRequest, PageRequest};

#[test]
fn close_recovery_preserves_full_text_and_paths_without_putting_values_in_decode_errors() {
    use tinys_core::page_wire::ClosePreparation;
    let text = "A long draft 🐈\n".repeat(1000);
    let wire = json!({"unresolvedDrafts": [
        {"path": ["rows", {"id":"a"}, "text"], "text": text},
        {"path": ["title"], "text": "第二份草稿"}
    ], "futureHint": true});
    let reply = ClosePreparation::decode(&wire.to_string()).unwrap();
    assert_eq!(reply.unresolved_drafts.len(), 2);
    assert_eq!(reply.unresolved_drafts[0].text, text);
    assert_eq!(reply.unresolved_drafts[0].field_label(), r#"["rows",{"id":"a"},"text"]"#);
    assert_eq!(reply.unresolved_drafts[1].text, "第二份草稿");
    assert!(ClosePreparation::decode(r#"{"unresolvedDrafts":[]}"#).unwrap().unresolved_drafts.is_empty());
    for invalid in ["true", "{}", r#"{"unresolvedDrafts":[{"path":[true],"text":"private text"}]}"#] {
        let error = ClosePreparation::decode(invalid).unwrap_err();
        assert_eq!(error.message, "Invalid close preparation reply");
    }
}

#[test]
fn each_page_request_classifies_its_required_capability() {
    use tinys_core::{app::Capability, page_wire::PageErrorKind};
    for (request, required) in [
        (PageRequest::Open {}, None),
        (PageRequest::Apply { batch: serde_json::from_value(json!({"intents":[]})).unwrap() }, None),
        (PageRequest::Flush {}, None),
        (PageRequest::Undo {}, None),
        (PageRequest::Redo {}, None),
        (PageRequest::Config {}, None),
        (PageRequest::HostEnvironment {}, None),
        (PageRequest::HapticsPlay { feedback: tinys_core::page_wire::HapticFeedback::Selection {} }, None),
        (PageRequest::AttachmentsPut { bytes: String::new() }, None),
        (PageRequest::WindowResize { width: 320, height: 240 }, None),
        (PageRequest::SpeechTranscribe { attachment: "a".repeat(64), locale: None }, Some(Capability::Speech)),
        (PageRequest::Ready {}, None),
        (PageRequest::PageRecovered {}, None),
        (PageRequest::Failed { error: String::new() }, None),
        (PageRequest::PageError { kind: PageErrorKind::Operation, error: String::new() }, None),
    ] {
        assert_eq!(request.requires(), required, "{request:?}");
        request.check().unwrap();
    }
}

#[test]
fn page_requests_and_nested_batches_are_strict() {
    for value in [
        json!({"method":"open"}),
        json!({"method":"config"}),
        json!({"method":"host.environment"}),
        json!({"method":"haptics.play","feedback":{"kind":"selection"}}),
        json!({"method":"haptics.play","feedback":{"kind":"impact","style":"soft"}}),
        json!({"method":"haptics.play","feedback":{"kind":"notification","notification":"success"}}),
        json!({"method":"flush"}),
        json!({"method":"apply", "batch":{"intents":[]}}),
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
        json!({"method":"host.environment", "platform":"macos"}),
        json!({"method":"haptics.play","feedback":{"kind":"impact","style":"unknown"}}),
        json!({"method":"haptics.play","feedback":{"kind":"selection","intensity":1}}),
        json!({"method":"haptics.play","feedback":{"kind":"notification"}}),
        json!({"method":"attachments.list"}),
        json!({"method":"apply", "batch":{}}),
        json!({"method":"apply", "batch":""}),
        json!({"method":"apply", "batch":null}),
        json!({"method":"attachments.read", "attachmentID":"a".repeat(64)}),
        json!({"method":"commands.run", "name":"addTask", "args":{"text":"task"}}),
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
        serde_json::from_value::<tinys_core::Batch>(json!({"intents":[{"type":"set","path":["title"],"value":value}]}))
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
fn native_actions_and_host_requests_use_the_same_serializer() {
    use tinys_core::page_wire::{HostDevice, HostEnvironment, HostPlatform};
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &HostReply::Environment {
                environment: HostEnvironment { platform: HostPlatform::Macos, device: HostDevice::Desktop },
            }
            .to_json()
        )
        .unwrap(),
        json!({"ok":true,"method":"host.environment","environment":{"platform":"macos","device":"desktop"}})
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&HostReply::Ready.to_json()).unwrap(),
        json!({"ok":true,"method":"ready"})
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&HostRequest::Publish { payload: "{}".into() }.to_json()).unwrap(),
        json!({"method":"publish","payload":"{}"})
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&HostReply::HapticsPlay.to_json()).unwrap(),
        json!({"ok":true,"method":"haptics.play"})
    );
    let refused: serde_json::Value =
        serde_json::from_str(&HostReply::WindowResize { width: f64::NAN, height: 3.0 }.to_json()).unwrap();
    assert_eq!(refused["ok"], false);
}

// A transcription names a stored recording by its attachment ID, never its bytes, and an
// optional locale identifier; both are checked before any host sees the request.
#[test]
fn speech_requests_name_a_stored_recording_and_a_bounded_locale() {
    let id = "0123456789abcdef".repeat(4);
    for value in [
        json!({"method":"speech.transcribe","attachment":id}),
        json!({"method":"speech.transcribe","attachment":id,"locale":null}),
        json!({"method":"speech.transcribe","attachment":id,"locale":"en-US"}),
        json!({"method":"speech.transcribe","attachment":id,"locale":"zh_Hant_TW"}),
        json!({"method":"speech.transcribe","attachment":id,"locale":"a".repeat(35)}),
    ] {
        assert!(PageRequest::decode(&value.to_string()).is_ok(), "{value}");
    }
    for value in [
        json!({"method":"speech.transcribe"}),
        json!({"method":"speech.transcribe","attachment":&id[1..]}),
        json!({"method":"speech.transcribe","attachment":id.to_uppercase()}),
        json!({"method":"speech.transcribe","attachment":id,"bytes":"AAAA"}),
        json!({"method":"speech.transcribe","attachment":id,"locale":""}),
        json!({"method":"speech.transcribe","attachment":id,"locale":"a".repeat(36)}),
        json!({"method":"speech.transcribe","attachment":id,"locale":"en US"}),
        json!({"method":"speech.transcribe","attachment":id,"locale":"-en"}),
        json!({"method":"speech.transcribe","attachment":id,"locale":"en-US\u{0}"}),
        json!({"method":"speech.transcribe","attachment":id,"locale":7}),
    ] {
        assert!(PageRequest::decode(&value.to_string()).is_err(), "{value}");
    }
}

// The host's outcome is serialized by Rust and bounded before the page reads it.
#[test]
fn speech_outcomes_are_bounded_before_they_reach_the_page() {
    use tinys_core::page_wire::{SpeechOutcome, SpeechUnavailable};
    let reply = |outcome| {
        serde_json::from_str::<serde_json::Value>(&HostReply::SpeechTranscribe { outcome }.to_json()).unwrap()
    };
    assert_eq!(
        reply(SpeechOutcome::Transcribed { text: "Hello there.".into(), locale: "en-US".into() }),
        json!({"ok":true,"method":"speech.transcribe","outcome":{"status":"transcribed","text":"Hello there.","locale":"en-US"}})
    );
    assert_eq!(
        reply(SpeechOutcome::Unavailable { reason: SpeechUnavailable::NotDeclared, message: "no".into() })["outcome"],
        json!({"status":"unavailable","reason":"notDeclared","message":"no"})
    );
    assert_eq!(reply(SpeechOutcome::unsupported_host())["outcome"]["reason"], "host");
    assert_eq!(
        reply(SpeechOutcome::Unavailable {
            reason: SpeechUnavailable::Format,
            message: "Unsupported audio/ogg".into()
        })["outcome"],
        json!({"status":"unavailable","reason":"format","message":"Unsupported audio/ogg"})
    );
    let limit = 1024 * 1024;
    let at_limit = reply(SpeechOutcome::Transcribed { text: "x".repeat(limit), locale: "en".into() });
    assert_eq!(at_limit["outcome"]["text"].as_str().unwrap().len(), limit);
    let over = reply(SpeechOutcome::Transcribed { text: "x".repeat(limit + 1), locale: "en".into() });
    assert_eq!(over["outcome"]["status"], "failed");
    let odd = reply(SpeechOutcome::Transcribed { text: "hi".into(), locale: "en US".into() });
    assert_eq!(odd["outcome"]["status"], "failed");
    // Diagnostic text is shortened, never refused, in the page's UTF-16 units.
    let long = reply(SpeechOutcome::Failed { message: "😀".repeat(3000) });
    assert_eq!(long["outcome"]["message"].as_str().unwrap().encode_utf16().count(), 4096);
    let cancelled = reply(SpeechOutcome::Cancelled { message: "x".repeat(5000) });
    assert_eq!(cancelled["outcome"]["message"].as_str().unwrap().len(), 4096);
}
