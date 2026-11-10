//! The frame codec against `tests/fixtures/relay`.
use super::{envelope, frame::*, limits::BASE_BYTES};
use crate::hex;
use serde_json::{Value, json};

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

#[test]
fn frozen_frame_codec_is_also_the_current_transport_codec() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../../tests/fixtures/relay/frames.json")).unwrap();
    for case in cases {
        let bytes = unhex(case["hex"].as_str().unwrap());
        let frame = Frame::decode(&bytes).unwrap();
        assert_eq!(frame_json(&frame), case["frame"], "{}", case["name"]);
        if let Frame::Hello { protocol, .. } = &frame {
            assert_eq!(*protocol, PROTOCOL, "The relay must still accept shipped Hello frames");
        }
        assert_eq!(frame.encode(), bytes);
        let decoded = envelope::decode(&bytes).unwrap();
        assert_eq!(decoded, frame);
        assert_eq!(envelope::encode(decoded).unwrap(), bytes);
    }
}
#[test]
fn malformed_varint_overflow_is_refused() {
    let mut bytes = vec![7, 1]; // Error frame, code 1, then an overflowing message length
    bytes.extend([0x80; 9]);
    bytes.extend([0x02]);
    assert!(Frame::decode(&bytes).is_err());
}
#[test]
fn unknown_variants_fail_closed_but_error_codes_remain_opaque() {
    let cases: Value = serde_json::from_str(include_str!("../../tests/fixtures/relay/unknown.json")).unwrap();
    for bytes in cases["frames"].as_array().unwrap() {
        assert!(Frame::decode(&unhex(bytes.as_str().unwrap())).is_err());
    }
    let bytes = unhex(cases["unknownErrorFrame"].as_str().unwrap());
    let frame = Frame::decode(&bytes).unwrap();
    assert_eq!(frame, Frame::Error { code: 255, message: String::new() });
    assert_eq!(frame.encode(), bytes);
    let frame = envelope::decode(&bytes).unwrap();
    assert_eq!(frame, Frame::Error { code: 255, message: String::new() });
    assert_eq!(envelope::encode(frame).unwrap(), bytes);
}
#[test]
fn error_codes_keep_their_wire_bytes() {
    for (code, byte) in
        [(ErrorCode::Protocol, 1), (ErrorCode::App, 2), (ErrorCode::Ended, 3), (ErrorCode::Malformed, 5)]
    {
        assert_eq!(u8::from(code), byte);
        assert_eq!(ErrorCode::try_from(byte), Ok(code));
    }
    assert_eq!(ErrorCode::try_from(4), Err(4));
}
#[test]
fn limits_allow_a_full_base_in_one_frame() {
    let frame =
        Frame::Records(vec![Record { spans: vec![Span { peer: 1, start: 0, end: 1 }], bytes: vec![0; BASE_BYTES] }]);
    assert!(frame.encode().len() < MAX_FRAME);
    assert!(Frame::decode(&frame.encode()).is_ok());
}
