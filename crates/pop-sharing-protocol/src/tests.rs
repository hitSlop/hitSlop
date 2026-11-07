//! The 2.0.0 baseline DTO, link and Frame examples (`tests/fixtures/v1`).
use crate::{document::*, envelope, frame::*, hex, http::*, resources::*};
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

fn decoded<T: DeserializeOwned + Serialize>(input: &Value) -> Result<Value, serde_json::Error> {
    Ok(serde_json::to_value(serde_json::from_value::<T>(input.clone())?).unwrap())
}
/// Decodes `input` as the fixture type `kind` and encodes it again.
fn decode_as(kind: &str, input: &Value) -> Result<Value, serde_json::Error> {
    match kind {
        "EmptyReply" => decoded::<()>(input),
        "CreateRoom" => decoded::<CreateRoom>(input),
        "JoinDocument" => decoded::<JoinDocument>(input),
        "UpdateDocument" => decoded::<UpdateDocument>(input),
        "RoomInfo" => decoded::<RoomInfo>(input),
        "RoomPhase" => decoded::<RoomPhase>(input),
        "AccessState" => decoded::<AccessState>(input),
        "DocumentRole" => decoded::<DocumentRole>(input),
        "DocumentMetadata" => decoded::<DocumentMetadata>(input),
        "DocumentEntry" => decoded::<DocumentEntry>(input),
        "RegisteredDocument" => decoded::<RegisteredDocument>(input),
        "RegisteredDocuments" => decoded::<RegisteredDocuments>(input),
        "ConnectionPlan" => decoded::<ConnectionPlan>(input),
        "SharingControl" => decoded::<SharingControl>(input),
        "SharingEvent" => decoded::<SharingEvent>(input),
        "ResourceGrant" => decoded::<ResourceGrant>(input),
        "ApiFailure" => decoded::<ApiFailure>(input),
        "Manifest" => decoded::<Manifest>(input),
        "Resource" => decoded::<Resource>(input),
        "ResourceList" => decoded::<Vec<Resource>>(input),
        other => panic!("Uncovered DTO {other}"),
    }
}
fn http_cases() -> Vec<Value> {
    serde_json::from_str(include_str!("../tests/fixtures/v1/http.json")).unwrap()
}
#[test]
fn baseline_http_dtos_and_extra_reply_fields() {
    for case in http_cases() {
        let output = decode_as(case["type"].as_str().unwrap(), &case["input"])
            .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
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
}
/// Every schema the service publishes has an example a reader must decode.
#[cfg(feature = "openapi")]
#[test]
fn every_published_schema_has_a_fixture() {
    let covered: std::collections::HashSet<String> =
        http_cases().iter().map(|case| case["type"].as_str().unwrap().to_owned()).collect();
    let description = serde_json::to_value(crate::http::openapi()).unwrap();
    let schemas = description["components"]["schemas"].as_object().unwrap();
    assert!(!schemas.is_empty());
    for name in schemas.keys() {
        assert!(covered.contains(name), "OpenAPI component {name} has no fixture in http.json");
    }
}
#[test]
fn baseline_invitation_round_trips_exactly() {
    let link = include_str!("../tests/fixtures/v1/invitation.txt").trim();
    let invitation = Invitation::parse(link).unwrap();
    assert_eq!(invitation.room, "00000000-0000-4000-8000-000000000001");
    assert_eq!(invitation.capability, "b".repeat(64));
    assert_eq!(invitation.link().unwrap(), link);
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
    // Required fields have no defaults: a value without one is refused, not completed.
    for case in cases["incomplete"].as_array().unwrap() {
        assert!(decode_as(case["type"].as_str().unwrap(), &case["input"]).is_err(), "{}", case["name"]);
    }
}

#[test]
fn an_account_listing_beyond_one_control_reply_is_many_pages_far_below_it() {
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
    assert_eq!(last, json!({"documents": []}), "the last page has no cursor");
}

#[test]
fn limits_allow_a_full_base_in_one_frame() {
    let frame = Frame::Records(vec![Record {
        spans: vec![Span { peer: 1, start: 0, end: 1 }],
        bytes: vec![0; crate::BASE_BYTES],
    }]);
    assert!(frame.encode().len() < MAX_FRAME);
    assert!(Frame::decode(&frame.encode()).is_ok());
}
