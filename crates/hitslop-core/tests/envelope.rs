#![cfg(all(feature = "storage", not(target_arch = "wasm32")))]
//! The core evaluates each envelope kind against its TypeBox-generated contract. What
//! each contract accepts is tested where it is written (`packages/schema/tests`); this
//! proves every kind is wired to its contract, bounded and parsed as JSON.
use hitslop_core::envelope::{is_valid, Envelope};
use serde_json::{json, Value};

fn check(kind: Envelope, value: &Value) -> bool {
    is_valid(kind, value.to_string().as_bytes())
}

#[test]
fn each_envelope_kind_is_checked_against_its_contract() {
    let cases = [
        (
            Envelope::SocketRequest,
            json!({"documentPath":"/tmp/a.slop","protocol":1,"method":"batch","ops":"{}"}),
            json!({"documentPath":"/tmp/a.slop","method":"batch","ops":"{}"}),
        ),
        (Envelope::SocketReply, json!({"ok":true,"method":"batch","sequence":3,"ids":["a"]}), json!({"ok":false,"code":"bogus"})),
        (
            Envelope::SocketDiscovery,
            json!({"socket":"/tmp/s","documentPath":"/tmp/a.slop"}),
            json!({"socket":"/tmp/s","documentPath":"/x","extra":1}),
        ),
        (Envelope::PageRequest, json!({"method":"apply","batch":"{}"}), json!({"method":"apply","batch":{}})),
    ];
    for (kind, accepted, refused) in cases {
        assert!(check(kind, &accepted), "{accepted}");
        assert!(!check(kind, &refused), "{refused}");
    }
}

#[test]
fn envelopes_are_bounded_and_must_be_json() {
    assert!(!is_valid(Envelope::SocketDiscovery, b"{"));
    assert!(!is_valid(Envelope::SocketDiscovery, b""));
    let oversized = vec![b' '; 48 * 1024 * 1024 + 1];
    assert!(!is_valid(Envelope::SocketDiscovery, &oversized));
}
