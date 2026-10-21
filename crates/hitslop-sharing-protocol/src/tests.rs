//! Frozen v1 wire examples: add fixtures, never rewrite existing expectations.
use crate::{envelope, frame::*, http::*, resources::*};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}
fn frame_json(frame: &Frame) -> Value {
    let version = |v: &Version| json!(v.iter().map(|(peer, end)| json!([peer.to_string(), end])).collect::<Vec<_>>());
    let records = |records: &[Record]| {
        json!(
            records
                .iter()
                .map(|r| json!({
                    "spans": r.spans.iter().map(|s| json!([s.peer.to_string(), s.start, s.end])).collect::<Vec<_>>(),
                    "bytes": hex(&r.bytes),
                }))
                .collect::<Vec<_>>()
        )
    };
    match frame {
        Frame::Hello { protocol, app_digest, version: v } => {
            json!({"type": "hello", "protocol": protocol, "appDigest": hex(app_digest), "version": version(v)})
        }
        Frame::Welcome { permission, version: v } => json!({
            "type": "welcome",
            "permission": if *permission == Permission::Write { "write" } else { "read" },
            "version": version(v),
        }),
        Frame::Records(r) => json!({"type": "records", "records": records(r)}),
        Frame::CaughtUp => json!({"type": "caughtUp"}),
        Frame::Push { id, records: r } => json!({"type": "push", "id": id.to_string(), "records": records(r)}),
        Frame::Ack { id, status } => json!({"type": "ack", "id": id.to_string(), "status": format!("{status:?}")}),
        Frame::Error { code, message } => json!({"type": "error", "code": code, "message": message}),
    }
}

fn canonical<T: DeserializeOwned + Serialize>(input: &Value) -> Value {
    serde_json::to_value(serde_json::from_value::<T>(input.clone()).unwrap()).unwrap()
}
#[test]
fn frozen_http_dtos_and_extra_reply_fields() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../tests/fixtures/v1/http.json")).unwrap();
    for case in cases {
        let input = &case["input"];
        let output = match case["type"].as_str().unwrap() {
            "EmptyReply" => canonical::<()>(input),
            "CreateRoom" => canonical::<CreateRoom>(input),
            "CreatedRoom" => canonical::<CreatedRoom>(input),
            "AccountRoom" => canonical::<Option<AccountRoom>>(input),
            "RoomInit" => canonical::<RoomInit>(input),
            "RoomInfo" => canonical::<RoomInfo>(input),
            "RoomPhase" => canonical::<RoomPhase>(input),
            "ConnectionPlan" => {
                let plan: ConnectionPlan = serde_json::from_value(input.clone()).unwrap();
                assert_eq!(
                    plan.protocols,
                    [crate::SOCKET_PROTOCOL],
                    "Shipped clients must still negotiate the v1 socket"
                );
                serde_json::to_value(plan).unwrap()
            }
            "SharingControl" => canonical::<SharingControl>(input),
            "SharingEvent" => canonical::<SharingEvent>(input),
            "ResourceGrant" => canonical::<ResourceGrant>(input),
            "ApiFailure" => canonical::<ApiFailure>(input),
            "Manifest" => canonical::<Manifest>(input),
            "Resource" => canonical::<Resource>(input),
            "ResourceList" => canonical::<Vec<Resource>>(input),
            other => panic!("Uncovered DTO {other}"),
        };
        assert_eq!(output, case["canonical"], "{}", case["name"]);
        if let Some(statuses) = case["statuses"].as_array() {
            for status in statuses {
                assert_eq!(
                    serde_json::to_value(ApiFailure::refused(status.as_u64().unwrap() as u16)).unwrap(),
                    case["canonical"]
                );
            }
        }
    }
    let link = include_str!("../tests/fixtures/v1/invitation.txt").trim();
    let invitation = Invitation::parse(link).unwrap();
    assert_eq!(invitation.room, "a".repeat(32));
    assert_eq!(invitation.capability, "b".repeat(64));
    assert_eq!(invitation.link().unwrap(), link);
}
#[test]
fn frozen_binary_codecs() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../tests/fixtures/v1/frames.json")).unwrap();
    for case in cases {
        let bytes = unhex(case["hex"].as_str().unwrap());
        let frame = Frame::decode(&bytes).unwrap();
        assert_eq!(frame_json(&frame), case["frame"], "{}", case["name"]);
        if let Frame::Hello { protocol, .. } = &frame {
            assert_eq!(*protocol, PROTOCOL, "The relay must still accept shipped Hello frames");
        }
        assert_eq!(frame.encode(), bytes);
        let bytes = unhex(case["envelopeHex"].as_str().unwrap());
        let decoded = envelope::decode(&bytes).unwrap();
        assert_eq!(decoded, frame);
        assert_eq!(envelope::encode(decoded).unwrap(), bytes);
    }
}
#[test]
fn unknown_variants_fail_closed_but_error_codes_remain_opaque() {
    let cases: Value = serde_json::from_str(include_str!("../tests/fixtures/v1/unknown.json")).unwrap();
    assert!(serde_json::from_value::<RoomPhase>(cases["phase"].clone()).is_err());
    assert!(serde_json::from_value::<SharingControl>(cases["control"].clone()).is_err());
    assert!(serde_json::from_value::<SharingEvent>(cases["event"].clone()).is_err());
    for bytes in cases["frames"].as_array().unwrap() {
        assert!(Frame::decode(&unhex(bytes.as_str().unwrap())).is_err());
    }
    for bytes in cases["envelopes"].as_array().unwrap() {
        assert!(envelope::decode(&unhex(bytes.as_str().unwrap())).is_err());
    }
    let bytes = unhex(cases["unknownErrorFrame"].as_str().unwrap());
    let frame = Frame::decode(&bytes).unwrap();
    assert_eq!(frame, Frame::Error { code: 255, message: String::new() });
    assert_eq!(frame.encode(), bytes);
    let bytes = unhex(cases["unknownErrorEnvelope"].as_str().unwrap());
    let frame = envelope::decode(&bytes).unwrap();
    assert_eq!(frame, Frame::Error { code: 255, message: String::new() });
    assert_eq!(envelope::encode(frame).unwrap(), bytes);
}
