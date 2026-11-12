use pop_contracts::{
    hex,
    relay::frame::{Ack, Frame, Permission, Record, Span, Version},
};
use serde_json::{Value, json};

// --- The relay wire ---------------------------------------------------------------------

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}
/// A frame as the relay's tests read it: peers as decimal strings, bytes as hex.
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
            json!({"type": "hello", "protocol": protocol, "appDigest": hex(app_digest.as_bytes()), "version": version(v)})
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
/// One frame of every type, at the edges the codec must agree on with the relay's.
fn golden() -> Vec<(&'static str, Frame)> {
    let span = |peer, start, end| Span { peer, start, end };
    vec![
        (
            "hello",
            Frame::Hello {
                protocol: 1,
                app_digest: pop_contracts::AppDigest::from_bytes(std::array::from_fn(|i| i as u8)),
                version: [(1, 5), (300, 128), (u64::MAX, i32::MAX as u32)].into(),
            },
        ),
        ("welcome to write", Frame::Welcome { permission: Permission::Write, version: [(7, 1)].into() }),
        ("welcome to read", Frame::Welcome { permission: Permission::Read, version: Version::new() }),
        (
            "records",
            Frame::Records(vec![
                Record { spans: vec![span(1, 0, 3), span(u64::MAX, 127, 129)], bytes: vec![0x6c, 0x6f, 0x72, 0x6f] },
                Record { spans: vec![span(2, 16_384, 16_385)], bytes: vec![0; 130] },
            ]),
        ),
        ("caught up", Frame::CaughtUp),
        ("push", Frame::Push { id: 42, records: vec![Record { spans: vec![span(9, 0, 1)], bytes: vec![1, 2, 3] }] }),
        ("ack stored", Frame::Ack { id: 1, status: Ack::Stored }),
        ("ack gap", Frame::Ack { id: u64::MAX, status: Ack::Gap }),
        ("ack too large", Frame::Ack { id: 2, status: Ack::TooLarge }),
        ("ack denied", Frame::Ack { id: 3, status: Ack::Denied }),
        ("ack quota", Frame::Ack { id: 4, status: Ack::Quota }),
        ("error", Frame::Error { code: 1, message: "Different app ✓".into() }),
    ]
}
const FRAMES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sync-frames.json");

/// The golden frames are the wire's contract with the relay, which tests its codec against
/// the same file. Changing the wire changes them: `POP_WRITE_SYNC_FRAMES=1` rewrites the
/// file, and the relay's tests must pass against it.
#[test]
fn frames_encode_as_their_golden_bytes() {
    let expected: Vec<Value> = golden()
        .into_iter()
        .map(|(name, frame)| json!({"name": name, "hex": hex(&frame.encode()), "frame": frame_json(&frame)}))
        .collect();
    if std::env::var_os("POP_WRITE_SYNC_FRAMES").is_some() {
        std::fs::write(FRAMES, serde_json::to_string_pretty(&expected).unwrap() + "\n").unwrap();
    }
    let stored: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(FRAMES).unwrap()).unwrap();
    assert_eq!(stored, expected, "the wire changed; see this test's comment");
    for ((name, frame), entry) in golden().into_iter().zip(&stored) {
        let bytes = unhex(entry["hex"].as_str().unwrap());
        assert_eq!(Frame::decode(&bytes).unwrap(), frame, "{name}");
    }
}

#[test]
fn malformed_frames_are_refused() {
    let push = Frame::Push {
        id: 1,
        records: vec![Record { spans: vec![Span { peer: 1, start: 0, end: 2 }], bytes: vec![9] }],
    };
    let bytes = push.encode();
    let mut cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("an unknown type", vec![0x7f]),
        ("truncated", bytes[..bytes.len() - 1].to_vec()),
        ("trailing bytes", [bytes.clone(), vec![0]].concat()),
        ("an unknown acknowledgement", [&[6][..], &[0; 8], &[9]].concat()),
        ("an unknown permission", vec![2, 2, 0]),
        // A count claiming more items than the frame could hold.
        ("an impossible count", [&[3][..], &[0xff, 0xff, 0xff, 0x0f]].concat()),
        ("an overlong integer", [&[3][..], &[0x80; 11]].concat()),
        ("oversized", vec![4; pop_contracts::relay::frame::MAX_FRAME + 1]),
    ];
    let edit = |f: &dyn Fn(&mut Frame)| {
        let mut frame = push.clone();
        f(&mut frame);
        frame.encode()
    };
    cases.push((
        "an empty span",
        edit(&|f| {
            if let Frame::Push { records, .. } = f {
                records[0].spans[0].end = 0
            }
        }),
    ));
    cases.push((
        "a record without spans",
        edit(&|f| {
            if let Frame::Push { records, .. } = f {
                records[0].spans.clear()
            }
        }),
    ));
    // Version peers are sorted and unique.
    let mut unsorted = Frame::Welcome { permission: Permission::Read, version: [(1, 1), (2, 1)].into() }.encode();
    unsorted.swap(10, 19);
    cases.push(("unsorted peers", unsorted));
    for (case, bytes) in cases {
        assert!(Frame::decode(&bytes).is_err(), "{case} was accepted");
    }
}
