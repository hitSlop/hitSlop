#![cfg(all(feature = "schema-validation", not(target_arch = "wasm32")))]
use hitslop_core::envelope::{is_valid, Envelope};
use serde_json::{json, Value};

fn check(kind: Envelope, value: &Value) -> bool {
    is_valid(kind, value.to_string().as_bytes())
}
fn request(extra: Value) -> Value {
    let mut base = json!({"documentPath":"/tmp/a.slop","epoch":"e"});
    base.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    base
}

#[test]
fn socket_requests_match_the_contract() {
    let accepted = [
        json!({"documentPath":"/tmp/a.slop","method":"hello"}),
        json!({"documentPath":"/tmp/a.slop","method":"attachments.list"}),
        // Operations are JSON text that only the document core parses.
        request(json!({"method":"batch","ops":r#"{"kind":"anything"}"#})),
        request(json!({"method":"batch","ops":"[{}, {\"x\":1}]"})),
        request(json!({"method":"theme.set","values":{"accent":"#ffffff","overlay":"#0a0c10d9"}})),
        request(json!({"method":"theme.reset"})),
        request(json!({"method":"theme.reset","token":"accent"})),
        json!({"documentPath":"/tmp/a.slop","method":"theme.export"}),
        // A theme file is text that only the core parses.
        request(json!({"method":"theme.import","file":"{\"template\":\"x\"}"})),
        request(json!({"method":"export","format":"png","output":"/tmp/out.png"})),
    ];
    for value in accepted {
        assert!(check(Envelope::SocketRequest, &value), "{value}");
    }
    let rejected = [
        json!({"documentPath":"/tmp/a.slop","method":"unknown"}),
        // A mutation needs the owner epoch.
        json!({"documentPath":"/tmp/a.slop","method":"batch","ops":"{}"}),
        request(json!({"method":"batch","ops":{"kind":"anything"}})),
        // Unknown fields are refused, never tolerated.
        json!({"documentPath":"/tmp/a.slop","method":"hello","extra":true}),
        request(json!({"method":"export","format":"gif","output":"/tmp/out.gif"})),
        request(json!({"method":"attachments.read","attachmentID":"not-a-digest"})),
        // A palette's names and colors are enforced in full: lowercase hex, one spelling.
        request(json!({"method":"theme.set","values":{"1bad":"#ffffff"}})),
        request(json!({"method":"theme.set","values":{"accent":""}})),
        request(json!({"method":"theme.set","values":{"accent":"#fff"}})),
        request(json!({"method":"theme.set","values":{"accent":"#FFFFFF"}})),
        request(json!({"method":"theme.set","values":{"accent":"#ffffffff"}})),
        request(json!({"method":"theme.set","values":{"accent":"red"}})),
        request(json!({"method":"theme.reset","token":"not a token"})),
        request(json!({"method":"theme.import"})),
        json!({"documentPath":"/tmp/a.slop","method":"theme.import","file":"{}"}),
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
fn page_requests_match_the_contract() {
    let digest = "a".repeat(64);
    for value in [
        json!({"method":"config"}),
        json!({"method":"ready"}),
        json!({"method":"apply","batch":"{}"}),
        json!({"method":"text","request":"{}"}),
        json!({"method":"undo"}),
        json!({"method":"attachments.read","attachmentID":digest}),
        json!({"method":"window.resize","width":320,"height":240}),
        json!({"method":"pageError","kind":"application","error":"boom"}),
    ] {
        assert!(check(Envelope::PageRequest, &value), "{value}");
    }
    for value in [
        json!({"method":"config","extra":1}),
        json!({"method":"open","view":"retired"}),
        json!({"method":"apply","batch":{}}),
        json!({"method":"theme.save","values":{"accent":"red"}}),
        json!({"method":"window.resize","width":239,"height":240}),
        json!({"method":"window.resize","width":320,"height":4097}),
        json!({"method":"window.resize","width":320.5,"height":240}),
        json!({"method":"pageError","kind":"other","error":"x"}),
        json!({"method":"attachments.read","attachmentID":"short"}),
        json!({}),
    ] {
        assert!(!check(Envelope::PageRequest, &value), "{value}");
    }
}

#[test]
fn envelopes_are_bounded_and_must_be_json() {
    assert!(!is_valid(Envelope::SocketDiscovery, b"{"));
    assert!(!is_valid(Envelope::SocketDiscovery, b""));
    let oversized = vec![b' '; 48 * 1024 * 1024 + 1];
    assert!(!is_valid(Envelope::SocketDiscovery, &oversized));
}
