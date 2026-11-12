//! HTTP DTO and link examples (`tests/fixtures/v1`).
use crate::{document::*, http::*, resources::*};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

fn library_entry(n: usize, title: String) -> crate::library::AccountEntry {
    crate::library::AccountEntry {
        document: format!("00000000-0000-4000-8000-{n:012x}"),
        role: DocumentRole::Owner,
        title,
        template_id: "test".into(),
        sharing: false,
        revision: 1,
    }
}

#[test]
fn library_snapshot_pages_bound_escaped_bytes_and_preserve_every_entry() {
    use crate::library::{LibraryFrame, snapshot_pages};
    let entries: Vec<_> = (0..600).map(|n| library_entry(n, "\0".repeat(crate::limits::TITLE_BYTES))).collect();
    let pages = snapshot_pages(entries.clone()).unwrap();
    assert!(pages.len() > 3, "Escaped control characters must split before the entry-count limit");
    let mut received = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        assert!(page.len() <= crate::CONTROL_BYTES);
        let LibraryFrame::Snapshot { entries, last } = LibraryFrame::decode(page.as_bytes()).unwrap() else {
            panic!("Expected snapshot");
        };
        assert_eq!(last, i == pages.len() - 1);
        assert!(entries.len() <= crate::limits::ACCOUNT_SNAPSHOT_PAGE);
        received.extend(entries);
    }
    assert_eq!(received, entries);
}

#[test]
fn library_empty_snapshot_and_count_bounded_pages_complete() {
    use crate::library::{LibraryFrame, snapshot_pages};
    let pages = snapshot_pages([]).unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(
        LibraryFrame::decode(pages[0].as_bytes()).unwrap(),
        LibraryFrame::Snapshot { entries: vec![], last: true }
    );
    let pages = snapshot_pages((0..257).map(|n| library_entry(n, "Small".into()))).unwrap();
    assert_eq!(pages.len(), 2);
    let LibraryFrame::Snapshot { entries, last } = LibraryFrame::decode(pages[0].as_bytes()).unwrap() else {
        panic!("Expected snapshot");
    };
    assert_eq!(entries.len(), 256);
    assert!(!last);
}

#[test]
fn library_frames_refuse_invalid_identity_metadata_and_oversized_input() {
    use crate::library::LibraryFrame;
    let mut entry = library_entry(1, "Valid".into());
    entry.revision = 0;
    assert!(LibraryFrame::Upsert { entry: entry.clone() }.encode().is_err());
    entry.revision = 1;
    entry.title = "é".repeat(crate::limits::TITLE_BYTES);
    assert!(LibraryFrame::Upsert { entry }.encode().is_err());
    assert!(LibraryFrame::Remove { document: "other-account".into() }.encode().is_err());
    assert_eq!(
        LibraryFrame::decode(&vec![b' '; crate::CONTROL_BYTES + 1]).unwrap_err().code,
        pop_contracts::Code::TooLarge
    );
    assert!(LibraryFrame::decode(br#"{"type":"upsert","entry":{}}"#).is_err());
}

fn decoded<T: DeserializeOwned + Serialize>(input: &Value) -> Result<Value, serde_json::Error> {
    Ok(serde_json::to_value(serde_json::from_value::<T>(input.clone())?).unwrap())
}
/// Decodes `input` as the fixture type `kind` and encodes it again.
fn decode_as(kind: &str, input: &Value) -> Result<Value, serde_json::Error> {
    match kind {
        "EmptyReply" => decoded::<()>(input),
        "CreateRoom" => decoded::<CreateRoom>(input),
        "OpenRoom" => decoded::<OpenRoom>(input),
        "JoinDocument" => decoded::<JoinDocument>(input),
        "UpdateDocument" => decoded::<UpdateDocument>(input),
        "RoomInfo" => decoded::<RoomInfo>(input),
        "RoomPhase" => decoded::<RoomPhase>(input),
        "AccessState" => decoded::<AccessState>(input),
        "DocumentRole" => decoded::<DocumentRole>(input),
        "JoinedDocument" => decoded::<JoinedDocument>(input),
        "ApiFailure" => decoded::<ApiFailure>(input),
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
    }
}
/// Keep HTTP wire coverage independent from documentation generators.
#[test]
fn every_http_dto_has_a_fixture() {
    let cases = http_cases();
    let covered: std::collections::HashSet<_> = cases.iter().map(|case| case["type"].as_str().unwrap()).collect();
    for name in [
        "CreateRoom",
        "OpenRoom",
        "RoomInfo",
        "RoomPhase",
        "ApiFailure",
        "DocumentRole",
        "AccessState",
        "JoinDocument",
        "UpdateDocument",
        "JoinedDocument",
        "Resource",
    ] {
        assert!(covered.contains(name), "HTTP DTO {name} has no fixture in http.json");
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
fn unknown_phases_fail_closed_and_required_fields_have_no_defaults() {
    let cases: Value = serde_json::from_str(include_str!("../tests/fixtures/v1/unknown.json")).unwrap();
    assert!(serde_json::from_value::<RoomPhase>(cases["phase"].clone()).is_err());
    // Required fields have no defaults: a value without one is refused, not completed.
    for case in cases["incomplete"].as_array().unwrap() {
        assert!(decode_as(case["type"].as_str().unwrap(), &case["input"]).is_err(), "{}", case["name"]);
    }
}
