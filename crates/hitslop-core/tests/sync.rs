//! Sync through a relay (plans/sync-simplification.md §7). Replicas run the real session
//! and every frame crosses the binary codec; the relay (`support::relay`) keeps the
//! Durable Object's rules over memory. The relay's own codec is tested against `sync-frames.json`.
mod support;
use hitslop_core::Document;
use hitslop_core::Origin;
use hitslop_core::sync::{Ack, Frame, Permission, Record, Session, Span, Status, Version};
use loro::{ExportMode, LoroDoc, VersionVector};
use serde_json::{Value, json};
use std::collections::VecDeque;
use support::generate::intent;
use support::relay::Relay;
use support::{ApplyJson, View, app, fixture, next, workload};

const APP: [u8; 32] = [7; 32];

// --- The relay wire ---------------------------------------------------------------------

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
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
/// One frame of every type, at the edges the codec must agree on with the relay's.
fn golden() -> Vec<(&'static str, Frame)> {
    let span = |peer, start, end| Span { peer, start, end };
    vec![
        (
            "hello",
            Frame::Hello {
                protocol: 1,
                app_digest: std::array::from_fn(|i| i as u8),
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
/// the same file. Changing the wire changes them: `HITSLOP_WRITE_SYNC_FRAMES=1` rewrites the
/// file, and the relay's tests must pass against it.
#[test]
fn frames_encode_as_their_golden_bytes() {
    let expected: Vec<Value> = golden()
        .into_iter()
        .map(|(name, frame)| json!({"name": name, "hex": hex(&frame.encode()), "frame": frame_json(&frame)}))
        .collect();
    if std::env::var_os("HITSLOP_WRITE_SYNC_FRAMES").is_some() {
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
        ("oversized", vec![4; hitslop_core::sync::MAX_FRAME + 1]),
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

// --- Replicas through the relay, with faults ---------------------------------------------

struct Replica {
    doc: Document,
    session: Session,
    /// The page's view: every publication, local and remote, applied in order.
    view: View,
    saved: Vec<u8>,
    connected: bool,
    up: VecDeque<Vec<u8>>,
    down: VecDeque<Vec<u8>>,
}
impl Replica {
    fn open(spec: &hitslop_core::AppSpec, saved: Vec<u8>) -> Self {
        // A new session: a new Loro peer, as every reopened owner has.
        let doc = Document::open(spec, &saved, &[]).unwrap();
        let view = View::of(&doc);
        Self {
            doc,
            session: Session::new(APP),
            view,
            saved,
            connected: false,
            up: VecDeque::new(),
            down: VecDeque::new(),
        }
    }
    fn connect(&mut self) {
        self.connected = true;
        self.up.push_back(self.session.connect(&self.doc));
    }
    fn disconnect(&mut self) {
        self.connected = false;
        self.up.clear();
        self.down.clear();
        self.session.disconnect();
    }
    /// After a local commit, as the owner does.
    fn push(&mut self) {
        if let Some(push) = self.session.push(&self.doc).filter(|_| self.connected) {
            self.up.push_back(push);
        }
    }
    fn receive(&mut self) {
        let Some(bytes) = self.down.pop_front() else { return };
        let step = self.session.receive(&mut self.doc, &bytes);
        if let Some(publication) = step.applied.and_then(|a| a.publication) {
            self.view.publish(&publication);
        }
        self.up.extend(step.send);
    }
}
fn version(doc: &Document) -> VersionVector {
    let loro = LoroDoc::new();
    loro.import(&doc.checkpoint().unwrap()).unwrap();
    loro.oplog_vv()
}
/// Delivers one frame from replica `i` to the relay, routing its replies and broadcasts.
fn deliver(relay: &mut Relay, replicas: &mut [Replica], joined: &mut [bool], i: usize) {
    let Some(bytes) = replicas[i].up.pop_front() else { return };
    let (replies, stored) = relay.reply(&bytes);
    if replies.iter().any(|f| matches!(f, Frame::Welcome { .. })) {
        joined[i] = true;
    }
    replicas[i].down.extend(replies.iter().map(Frame::encode));
    if !stored.is_empty() {
        let records = Frame::Records(stored).encode();
        for (j, replica) in replicas.iter_mut().enumerate() {
            if j != i && joined[j] && replica.connected {
                replica.down.push_back(records.clone());
            }
        }
    }
}

// Failure: an edit that was saved locally or held by the relay is lost, replicas that hold
// the same edits read differently, a page's view drifts from its document, or sync pauses
// although nothing was wrong. Faults: dropped connections (losing pushes and
// acknowledgements in flight), relay restarts, and replicas that crash and reopen from
// their last save as a new session.
#[test]
fn replicas_converge_through_the_relay_despite_faults() {
    let f = fixture("checklist");
    let spec = app(f["schema"].to_string());
    for seed in 1..=workload("HITSLOP_MERGE_SEEDS", 40) as u64 {
        let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let origin = Document::create(&spec, &f["initial"].to_string()).unwrap();
        let start = origin.checkpoint().unwrap();
        let heads = version(&origin).iter().map(|(p, c)| (*p, *c as u32)).collect();
        let mut relay = Relay { app: APP, records: vec![], heads };
        let mut replicas: Vec<Replica> = (0..3).map(|_| Replica::open(&spec, start.clone())).collect();
        let mut joined = [false; 3];
        let mut serials = [0usize; 3];
        // Every operation that was durable somewhere: a local save or the relay.
        let mut durable = version(&origin);
        for _ in 0..workload("HITSLOP_MERGE_STEPS", 300) {
            let i = (next(&mut rng) % 3) as usize;
            let r = &mut replicas[i];
            match next(&mut rng) % 100 {
                0..=29 => {
                    let current = support::value(&r.doc);
                    let op = intent(&mut rng, &mut serials[i], &f["schema"], &current);
                    if let Ok(applied) = r.doc.apply_json(&json!({"intents": [op]}).to_string(), Origin::Page) {
                        r.view.publish(applied.publication.as_deref().unwrap_or(r#"{"ops":[]}"#));
                    }
                    r.push();
                }
                30..=34 => {
                    if let Ok(applied) = r.doc.undo() {
                        r.view.publish(applied.publication.as_deref().unwrap_or(r#"{"ops":[]}"#));
                    }
                    r.push();
                }
                35..=39 => {
                    r.saved = r.doc.checkpoint().unwrap();
                    durable.merge(&version(&r.doc));
                }
                40..=64 => deliver(&mut relay, &mut replicas, &mut joined, i),
                65..=89 => r.receive(),
                90..=93 => {
                    if r.connected {
                        r.disconnect();
                        joined[i] = false;
                    } else {
                        r.connect();
                    }
                }
                94..=95 => {
                    for (replica, joined) in replicas.iter_mut().zip(joined.iter_mut()) {
                        replica.disconnect();
                        *joined = false;
                    }
                }
                _ => {
                    let saved = r.saved.clone();
                    *r = Replica::open(&spec, saved);
                    joined[i] = false;
                }
            }
            for (j, replica) in replicas.iter().enumerate() {
                if let Status::Paused { reason } = replica.session.status() {
                    panic!("seed {seed}: replica {j} paused: {reason}");
                }
            }
        }
        // Everyone connects, and frames flow until none are left.
        for (replica, joined) in replicas.iter_mut().zip(joined.iter_mut()) {
            replica.disconnect();
            *joined = false;
            replica.connect();
        }
        let mut rounds = 0;
        while replicas.iter().any(|r| !r.up.is_empty() || !r.down.is_empty()) {
            rounds += 1;
            assert!(rounds < 10_000, "seed {seed}: sync did not settle");
            for i in 0..3 {
                deliver(&mut relay, &mut replicas, &mut joined, i);
                replicas[i].receive();
            }
        }
        let value = support::value(&replicas[0].doc);
        for (j, replica) in replicas.iter().enumerate() {
            assert_eq!(replica.session.status(), &Status::Synced, "seed {seed}: replica {j}");
            assert_eq!(support::value(&replica.doc), value, "seed {seed}: replica {j} reads differently");
            replica.view.check(&replica.doc, &format!("seed {seed}: replica {j}"));
            let held = version(&replica.doc);
            assert!(
                durable.iter().all(|(p, c)| held.get(p).is_some_and(|h| h >= c)),
                "seed {seed}: a durable edit was lost"
            );
        }
    }
}

// --- What a replica refuses --------------------------------------------------------------

const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"text"},"done":{"kind":"boolean"}}}"#;

/// A replica of `seed` admitted by a relay holding `relay`.
fn admitted(seed: &[u8], relay: &VersionVector) -> (Document, Session) {
    let mut doc = Document::open(&app(SCHEMA), seed, &[]).unwrap();
    let mut session = Session::new(APP);
    session.connect(&doc);
    let welcome =
        Frame::Welcome { permission: Permission::Write, version: relay.iter().map(|(p, c)| (*p, *c as u32)).collect() };
    assert!(session.receive(&mut doc, &welcome.encode()).send.is_empty(), "nothing to push");
    (doc, session)
}
/// A raw Loro change on `seed` by another peer, as a record the relay would send.
fn foreign(seed: &[u8], change: impl Fn(&LoroDoc)) -> (Record, VersionVector) {
    let other = LoroDoc::new();
    other.import(seed).unwrap();
    let before = other.oplog_vv();
    change(&other);
    other.commit();
    let spans = other
        .oplog_vv()
        .iter()
        .filter(|(p, c)| before.get(p).is_none_or(|b| b < c))
        .map(|(p, c)| Span { peer: *p, start: before.get(p).copied().unwrap_or(0) as u32, end: *c as u32 })
        .collect();
    (Record { spans, bytes: other.export(ExportMode::updates(&before)).unwrap() }, other.oplog_vv())
}

/// A change this app cannot accept (another peer wrote a number where the descriptor has
/// text) pauses sync and leaves the replica as it was, still editable.
#[test]
fn a_change_the_app_refuses_pauses_sync_and_leaves_the_replica_as_it_was() {
    let doc = Document::create(&app(SCHEMA), r#"{"title":"Kept","done":false}"#).unwrap();
    let seed = doc.checkpoint().unwrap();
    let (mut doc, mut session) = admitted(&seed, &version(&doc));
    let before = support::value(&doc);
    let (record, _) = foreign(&seed, |other| {
        other.get_map("data").insert("title", 5).unwrap();
    });
    let step = session.receive(&mut doc, &Frame::Records(vec![record]).encode());
    assert!(step.applied.is_none() && step.send.is_empty());
    let Status::Paused { reason } = session.status() else { panic!("{:?}", session.status()) };
    assert!(reason.contains("refused"), "{reason}");
    assert_eq!(support::value(&doc), before);
    doc.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
    assert!(session.push(&doc).is_none(), "a paused session pushes nothing until it reconnects");
}

/// Records whose dependencies never arrive are held by Loro without changing anything; at
/// the end of the backfill they pause sync, since the relay lacks history they need.
#[test]
fn records_missing_their_history_pause_sync_once_caught_up() {
    let doc = Document::create(&app(SCHEMA), r#"{"title":"Kept","done":false}"#).unwrap();
    let seed = doc.checkpoint().unwrap();
    let (mut doc, mut session) = admitted(&seed, &version(&doc));
    // Two changes by one peer; the relay delivers only the second.
    let other = LoroDoc::new();
    other.import(&seed).unwrap();
    other.get_map("data").insert("done", true).unwrap();
    other.commit();
    let middle = other.oplog_vv();
    other.get_map("data").insert("done", false).unwrap();
    other.commit();
    let peer = other.peer_id();
    let record = Record {
        spans: vec![Span {
            peer,
            start: middle.get(&peer).copied().unwrap() as u32,
            end: other.oplog_vv().get(&peer).copied().unwrap() as u32,
        }],
        bytes: other.export(ExportMode::updates(&middle)).unwrap(),
    };
    let step = session.receive(&mut doc, &Frame::Records(vec![record]).encode());
    assert!(step.applied.is_none(), "nothing applied");
    assert_eq!(session.status(), &Status::Syncing);
    session.receive(&mut doc, &Frame::CaughtUp.encode());
    let Status::Paused { reason } = session.status() else { panic!("{:?}", session.status()) };
    assert!(reason.contains("missing history"), "{reason}");
}

/// A gap (the relay holds less than this connection believed) reconnects with a Hello
/// carrying the replica's version, and the relay's answer heals it. A broadcast already
/// in flight must not pause sync while that new Hello is waiting for its Welcome.
#[test]
fn a_gap_says_hello_again() {
    let doc = Document::create(&app(SCHEMA), r#"{"title":"Kept","done":false}"#).unwrap();
    let seed = doc.checkpoint().unwrap();
    let (mut doc, mut session) = admitted(&seed, &version(&doc));
    doc.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
    let push = session.push(&doc).expect("a push");
    let Frame::Push { id, .. } = Frame::decode(&push).unwrap() else { panic!() };
    let step = session.receive(&mut doc, &Frame::Ack { id, status: Ack::Gap }.encode());
    let [hello] = step.send.as_slice() else { panic!("one frame") };
    assert!(matches!(Frame::decode(hello).unwrap(), Frame::Hello { .. }));
    assert_eq!(session.status(), &Status::Syncing);

    let (record, remote) = foreign(&seed, |other| {
        other.get_map("data").ensure_mergeable_text("title").unwrap().insert(0, "Remote ").unwrap();
    });
    let before = support::value(&doc);
    let broadcast = session.receive(&mut doc, &Frame::Records(vec![record.clone()]).encode());
    assert_eq!(session.status(), &Status::Syncing, "a broadcast before Welcome is covered by backfill");
    assert!(broadcast.applied.is_none() && broadcast.send.is_empty());
    assert_eq!(support::value(&doc), before);

    let mut relay = Relay {
        app: APP,
        records: vec![record],
        heads: remote.iter().map(|(p, c)| (*p, *c as u32)).collect(),
    };
    let (backfill, _) = relay.reply(hello);
    let mut pushes = vec![];
    for frame in backfill {
        pushes.extend(session.receive(&mut doc, &frame.encode()).send);
    }
    assert_eq!(session.status(), &Status::Syncing, "the local edit still needs its acknowledgement");
    for push in pushes {
        let (replies, _) = relay.reply(&push);
        for reply in replies {
            assert!(session.receive(&mut doc, &reply.encode()).send.is_empty());
        }
    }
    assert_eq!(session.status(), &Status::Synced);
    assert_eq!(support::value(&doc), json!({"title":"Remote Kept","done":true}));
    assert_eq!(relay.heads, version(&doc).iter().map(|(p, c)| (*p, *c as u32)).collect());
}

// Failure: a session reported `Synced` while the relay had not acknowledged a local change,
// so a status line would claim others have an edit they may never receive.
#[test]
fn synced_means_caught_up_with_nothing_unacknowledged() {
    let doc = Document::create(&app(SCHEMA), r#"{"title":"Kept","done":false}"#).unwrap();
    let seed = doc.checkpoint().unwrap();
    let (mut doc, mut session) = admitted(&seed, &version(&doc));
    assert_ne!(session.status(), &Status::Synced, "not caught up yet");
    session.receive(&mut doc, &Frame::CaughtUp.encode());
    assert_eq!(session.status(), &Status::Synced);
    doc.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
    let push = session.push(&doc).expect("a push");
    assert_ne!(session.status(), &Status::Synced, "the relay has not acknowledged the edit");
    let Frame::Push { id, .. } = Frame::decode(&push).unwrap() else { panic!() };
    session.receive(&mut doc, &Frame::Ack { id, status: Ack::Stored }.encode());
    assert_eq!(session.status(), &Status::Synced);
}
