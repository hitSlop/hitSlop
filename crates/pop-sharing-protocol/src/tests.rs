//! Frozen DTO and Frame examples, retained unchanged.
use crate::{envelope, frame::*, hex, http::*, resources::*};
use serde::{Serialize, de::DeserializeOwned};
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
            "AccountRoom" => continue, // Retired temporary-room discovery is not an active contract.
            "RoomInit" => canonical::<RoomInit>(input),
            "RoomInfo" => canonical::<RoomInfo>(input),
            "RoomPhase" => canonical::<RoomPhase>(input),
            "ConnectionPlan" => {
                let plan: ConnectionPlan = serde_json::from_value(input.clone()).unwrap();
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
    assert!(Invitation::parse(link).is_err(), "temporary room links do not name persistent documents");
    let invitation = Invitation { room: "00000000-0000-4000-8000-000000000001".into(), capability: "b".repeat(64) };
    assert_eq!(Invitation::parse(&invitation.link().unwrap()).unwrap().room, invitation.room);
}
#[test]
fn frozen_frame_codec_is_also_the_current_transport_codec() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../tests/fixtures/v1/frames.json")).unwrap();
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
    let cases: Value = serde_json::from_str(include_str!("../tests/fixtures/v1/unknown.json")).unwrap();
    assert!(serde_json::from_value::<RoomPhase>(cases["phase"].clone()).is_err());
    assert!(serde_json::from_value::<SharingControl>(cases["control"].clone()).is_err());
    assert!(serde_json::from_value::<SharingEvent>(cases["event"].clone()).is_err());
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
fn an_account_listing_beyond_one_control_reply_is_many_pages_far_below_it() {
    use crate::document::{DocumentRole, RegisteredDocument, RegisteredDocuments};
    use crate::limits::ACCOUNT_DOCUMENTS_PAGE;
    let all: Vec<_> = (0..16_000u32)
        .map(|n| RegisteredDocument {
            document: format!("00000000-0000-4000-8000-{n:012x}"),
            role: DocumentRole::Owner,
        })
        .collect();
    let whole = serde_json::to_vec(&RegisteredDocuments { documents: all.clone(), next: None }).unwrap();
    assert!(whole.len() > crate::CONTROL_BYTES, "one reply would exceed what clients read: {}", whole.len());
    // Walk the cursor as the account handler's keyset query does.
    let (mut listed, mut after, mut pages) = (Vec::new(), String::new(), 0);
    loop {
        let rows = all.iter().filter(|row| row.document > after).take(ACCOUNT_DOCUMENTS_PAGE + 1).cloned().collect();
        let page = RegisteredDocuments::page(rows);
        let bytes = serde_json::to_vec(&page).unwrap();
        assert!(bytes.len() < crate::CONTROL_BYTES / 8, "page {pages} is {} bytes", bytes.len());
        pages += 1;
        listed.extend(page.documents);
        let Some(next) = page.next else { break };
        after = next;
    }
    assert_eq!(pages, 16);
    assert_eq!(listed, all);
    let last: Value = serde_json::to_value(RegisteredDocuments::page(vec![])).unwrap();
    assert_eq!(last, json!({"documents": []}), "the last page keeps the original shape");
}

#[test]
fn compaction_hint_defaults_and_limits_allow_a_base_frame() {
    let old = json!({"document":"fixture", "phase":"live", "seedHash":null,"seedSize":null,"app":null});
    let mut info: RoomInfo = serde_json::from_value(old.clone()).unwrap();
    assert!(!info.compact);
    assert_eq!(serde_json::to_value(&info).unwrap(), old);
    info.compact = true;
    assert_eq!(serde_json::to_value(info).unwrap()["compact"], true);
    let frame = Frame::Records(vec![Record {
        spans: vec![Span { peer: 1, start: 0, end: 1 }],
        bytes: vec![0; crate::BASE_BYTES],
    }]);
    assert!(frame.encode().len() < MAX_FRAME);
    assert!(Frame::decode(&frame.encode()).is_ok());
}
