#![cfg(all(feature = "schema-validation", not(target_arch = "wasm32")))]
use hitslop_core::envelope::{is_valid, Envelope};
use serde_json::{json, Value};

fn check(kind: Envelope, value: &Value) -> bool {
    is_valid(kind, value.to_string().as_bytes())
}
fn request(extra: Value) -> Value {
    let mut base = json!({"id":"1","documentPath":"/tmp/a.slop","epoch":"e"});
    base.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    base
}

#[test]
fn socket_requests_match_the_contract() {
    let accepted = [
        json!({"id":"1","documentPath":"/tmp/a.slop","method":"hello"}),
        json!({"id":"1","documentPath":"/tmp/a.slop","method":"attachments.list"}),
        // Operations are JSON text that only the document core parses.
        request(json!({"method":"apply","op":r#"{"kind":"anything"}"#})),
        request(json!({"method":"batch","ops":"[{}, {\"x\":1}]"})),
        request(json!({"method":"theme.set","values":{"accent":"#fff"}})),
        request(json!({"method":"theme.reset"})),
        request(json!({"method":"export","format":"png","output":"/tmp/out.png"})),
    ];
    for value in accepted {
        assert!(check(Envelope::SocketRequest, &value), "{value}");
    }
    let rejected = [
        json!({"id":"1","documentPath":"/tmp/a.slop","method":"unknown"}),
        // A mutation needs the owner epoch.
        json!({"id":"1","documentPath":"/tmp/a.slop","method":"apply","op":"{}"}),
        request(json!({"method":"apply","op":{"kind":"anything"}})),
        // Unknown fields are refused, never tolerated.
        json!({"id":"1","documentPath":"/tmp/a.slop","method":"hello","extra":true}),
        request(json!({"method":"export","format":"gif","output":"/tmp/out.gif"})),
        request(json!({"method":"attachments.read","attachmentID":"not-a-digest"})),
        // The record's key pattern and value bounds are enforced in full.
        request(json!({"method":"theme.set","values":{"1bad":"red"}})),
        request(json!({"method":"theme.set","values":{"accent":""}})),
        json!("hello"),
    ];
    for value in rejected {
        assert!(!check(Envelope::SocketRequest, &value), "{value}");
    }
}

#[test]
fn socket_replies_and_discovery_match_the_contract() {
    assert!(check(Envelope::SocketReply, &json!({"ok":true,"sequence":3,"ids":["a"]})));
    assert!(check(Envelope::SocketReply, &json!({"ok":false,"error":"no","code":"rejected"})));
    for value in [
        json!({}),
        json!({"ok":true,"sequence":-1}),
        json!({"ok":false,"code":"bogus"}),
        json!({"ok":true,"surprise":1}),
    ] {
        assert!(!check(Envelope::SocketReply, &value), "{value}");
    }
    assert!(check(Envelope::SocketDiscovery, &json!({"socket":"/tmp/s","documentPath":"/tmp/a.slop"})));
    assert!(!check(Envelope::SocketDiscovery, &json!({"socket":"/tmp/s"})));
    assert!(!check(Envelope::SocketDiscovery, &json!({"socket":"/tmp/s","documentPath":"/x","extra":1})));
}

#[test]
fn bridge_requests_match_the_contract() {
    let digest = "a".repeat(64);
    for value in [
        json!({"method":"config"}),
        json!({"method":"ready"}),
        json!({"method":"attachments.read","attachmentID":digest}),
        json!({"method":"window.resize","width":320,"height":240}),
        json!({"method":"pageError","kind":"application","error":"boom"}),
    ] {
        assert!(check(Envelope::BridgeRequest, &value), "{value}");
    }
    for value in [
        json!({"method":"config","extra":1}),
        json!({"method":"theme.save","values":{"accent":"red"}}),
        json!({"method":"window.resize","width":239,"height":240}),
        json!({"method":"window.resize","width":320,"height":4097}),
        json!({"method":"window.resize","width":320.5,"height":240}),
        json!({"method":"pageError","kind":"other","error":"x"}),
        json!({"method":"attachments.read","attachmentID":"short"}),
        json!({}),
    ] {
        assert!(!check(Envelope::BridgeRequest, &value), "{value}");
    }
}

#[test]
fn envelopes_are_bounded_and_must_be_json() {
    assert!(!is_valid(Envelope::SocketDiscovery, b"{"));
    assert!(!is_valid(Envelope::SocketDiscovery, b""));
    let oversized = vec![b' '; 48 * 1024 * 1024 + 1];
    assert!(!is_valid(Envelope::SocketDiscovery, &oversized));
}
