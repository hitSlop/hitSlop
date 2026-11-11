//! Sync through a relay (plans/sync-simplification.md §7). Replicas run the real session
//! and every frame crosses the binary codec; the relay (`support::relay`) keeps the
//! Durable Object's rules over memory. The relay codec and its golden frames are tested in pop-contracts.
use crate::support;
use loro::{ExportMode, LoroDoc, VersionVector};
use pop_contracts::relay::frame::{Ack, Frame, Permission, Record, Span};
use pop_core::sync::{Base, BaseRead, Session, Status};
use pop_core::{Document, Origin};
use serde_json::json;
use std::collections::VecDeque;
use support::generate::intent;
use support::relay::Relay;
use support::{ApplyJson, View, app, fixture, next, workload};

const APP: pop_contracts::AppDigest = pop_contracts::AppDigest::from_bytes([7; 32]);

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
    fn open(spec: &pop_core::AppSpec, saved: Vec<u8>) -> Self {
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
fn base_of(session: &Session, doc: &Document) -> Option<Base> {
    match session.maintenance_base(doc).unwrap() {
        BaseRead::Pending => None,
        BaseRead::Ready(base) => Some(base),
        BaseRead::TooLarge => panic!("base is too large"),
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
    for seed in 1..=workload("POP_MERGE_SEEDS", 40) as u64 {
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
        for _ in 0..workload("POP_MERGE_STEPS", 300) {
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
                96 => {
                    if let Some(base) = base_of(&r.session, &r.doc) {
                        relay.compact(base);
                    }
                }
                _ => {
                    let saved = r.saved.clone();
                    *r = Replica::open(&spec, saved);
                    joined[i] = false;
                }
            }
            for (j, replica) in replicas.iter().enumerate() {
                if let Status::Paused { reason, .. } = replica.session.status() {
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
    let Status::Paused { reason, .. } = session.status() else { panic!("{:?}", session.status()) };
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
    let Status::Paused { reason, .. } = session.status() else { panic!("{:?}", session.status()) };
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
        let Some(loro::ValueOrContainer::Container(loro::Container::Text(title))) = other.get_map("data").get("title")
        else {
            panic!("title text")
        };
        title.insert(0, "Remote ").unwrap();
    });
    let before = support::value(&doc);
    let broadcast = session.receive(&mut doc, &Frame::Records(vec![record.clone()]).encode());
    assert_eq!(session.status(), &Status::Syncing, "a broadcast before Welcome is covered by backfill");
    assert!(broadcast.applied.is_none() && broadcast.send.is_empty());
    assert_eq!(support::value(&doc), before);

    let mut relay =
        Relay { app: APP, records: vec![record], heads: remote.iter().map(|(p, c)| (*p, *c as u32)).collect() };
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

// A full relay must allow its transport to make room without discarding local edits.
#[test]
fn quota_waits_for_compaction() {
    let origin = Document::create(&app(SCHEMA), r#"{"title":"Kept","done":false}"#).unwrap();
    let (mut doc, mut session) = admitted(&origin.checkpoint().unwrap(), &version(&origin));
    session.receive(&mut doc, &Frame::CaughtUp.encode());
    let mut relay =
        Relay { app: APP, records: vec![], heads: version(&origin).iter().map(|(&p, &n)| (p, n as u32)).collect() };
    doc.apply_json(r#"{"intents":[{"type":"set","path":["title"],"value":"Stored history"}]}"#, Origin::Page).unwrap();
    let (acks, _) = relay.reply(&session.push(&doc).unwrap());
    for ack in acks {
        session.receive(&mut doc, &ack.encode());
    }
    doc.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
    let Frame::Push { id, .. } = Frame::decode(&session.push(&doc).unwrap()).unwrap() else { panic!() };
    session.receive(&mut doc, &Frame::Ack { id, status: Ack::Quota }.encode());
    assert!(
        matches!(session.status(), Status::Paused { kind: pop_contracts::relay::PauseKind::RoomFull, .. }),
        "{:?}",
        session.status()
    );
    assert_eq!(support::value(&doc)["done"], true);
    relay.compact(base_of(&session, &doc).unwrap());
    let (replies, _) = relay.reply(&session.connect(&doc));
    for reply in replies {
        for push in session.receive(&mut doc, &reply.encode()).send {
            let (acks, _) = relay.reply(&push);
            for ack in acks {
                session.receive(&mut doc, &ack.encode());
            }
        }
    }
    assert_eq!(session.status(), &Status::Synced);
    assert_eq!(relay.heads, version(&doc).iter().map(|(&p, &n)| (p, n as u32)).collect());
}

/// Measurements use the same owner commits and push encoding as interactive edits.
/// Run with POP_NIGHTLY=1 bun run verify rust sync::room_compaction_measurements.
#[test]
fn room_compaction_measurements() {
    if std::env::var("POP_NIGHTLY").as_deref() != Ok("1") {
        return;
    }
    let f = fixture("checklist");
    for workload in ["typing", "checklist", "replacement"] {
        let mut doc = Document::create(&app(f["schema"].to_string()), &f["initial"].to_string()).unwrap();
        let seed = version(&doc);
        let mut session = Session::new(APP);
        session.connect(&doc);
        session.receive(
            &mut doc,
            &Frame::Welcome {
                permission: Permission::Write,
                version: seed.iter().map(|(&p, &n)| (p, n as u32)).collect(),
            }
            .encode(),
        );
        session.receive(&mut doc, &Frame::CaughtUp.encode());
        let mut total = 0;
        let steps = if workload == "replacement" { 1000 } else { 4000 };
        for i in 0..steps {
            let intents = match workload {
                "typing" => json!([{"type":"set","path":["title"],"value":"abcde".repeat(i + 1)}]),
                "checklist" => {
                    let id = format!("{:032x}", i % 2 + 1);
                    json!([{"type":"set","path":["rows",{"id":id},"done"],"value":(i/2)%2==0},
                        {"type":"move","path":["rows"],"id":id}])
                }
                _ => {
                    json!([{"type":"set","path":["title"],"value":format!("{i:04}{}", char::from(b'a' + (i % 26) as u8).to_string().repeat(20_000))}])
                }
            };
            doc.apply_json(&json!({"intents":intents}).to_string(), Origin::Page).unwrap();
            let _saved = doc.checkpoint().unwrap();
            let Frame::Push { id, records } = Frame::decode(&session.push(&doc).unwrap()).unwrap() else { panic!() };
            total += records.iter().map(|r| r.bytes.len()).sum::<usize>();
            session.receive(&mut doc, &Frame::Ack { id, status: Ack::Stored }.encode());
        }
        let raw = LoroDoc::new();
        raw.import(&doc.checkpoint().unwrap()).unwrap();
        let base = raw.export(ExportMode::updates(&doc.base_start())).unwrap();
        if base.len() > pop_contracts::relay::limits::BASE_BYTES {
            assert_eq!(doc.base(&version(&doc)).unwrap_err().code, pop_contracts::wire::Code::TooLarge);
        } else {
            assert_eq!(doc.base(&version(&doc)).unwrap(), base);
        }
        eprintln!(
            "COMPACTION {workload}: pushes={steps} records={total} base={} ratio={:.2}",
            base.len(),
            total as f64 / base.len() as f64
        );
    }
}

#[test]
fn base_is_updates_and_excludes_unsent_edits() {
    let original = Document::create(&app(SCHEMA), r#"{"title":"Seed","done":false}"#).unwrap();
    let seed = original.checkpoint().unwrap();
    let (mut doc, mut session) = admitted(&seed, &version(&original));
    assert!(base_of(&session, &doc).is_none(), "not caught up");
    session.receive(&mut doc, &Frame::CaughtUp.encode());
    let frontier = version(&doc);
    let base = base_of(&session, &doc).unwrap();
    let meta = LoroDoc::decode_import_blob_meta(&base.bytes, false).unwrap();
    assert_eq!(meta.mode, loro::EncodedBlobMode::Updates);
    assert_eq!(meta.partial_end_vv, frontier);
    doc.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
    assert!(base_of(&session, &doc).is_none(), "local edits unpushed");
    let Frame::Push { id, .. } = Frame::decode(&session.push(&doc).unwrap()).unwrap() else { panic!() };
    assert!(base_of(&session, &doc).is_none(), "push in flight");
    session.receive(&mut doc, &Frame::Ack { id, status: Ack::Quota }.encode());
    assert_eq!(base_of(&session, &doc).unwrap().frontier, frontier);
    assert_eq!(
        LoroDoc::decode_import_blob_meta(&doc.base(&frontier).unwrap(), false).unwrap().partial_end_vv,
        frontier
    );
}

#[test]
fn shallow_seed_base_is_applied_by_a_joiner() {
    let raw = LoroDoc::new();
    raw.import(&Document::create(&app(SCHEMA), r#"{"title":"Seed","done":false}"#).unwrap().checkpoint().unwrap())
        .unwrap();
    let trimmed = raw.export(ExportMode::shallow_snapshot(&raw.oplog_frontiers())).unwrap();
    let mut writer = Document::open(&app(SCHEMA), &trimmed, &[]).unwrap();
    writer.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
    let base = writer.base(&version(&writer)).unwrap();
    assert_eq!(LoroDoc::decode_import_blob_meta(&base, false).unwrap().mode, loro::EncodedBlobMode::Updates);
    let mut joiner = Document::open(&app(SCHEMA), &trimmed, &[]).unwrap();
    joiner.import_remote(&[&base]).unwrap();
    assert_eq!(support::value(&joiner), support::value(&writer));
}

#[test]
fn relay_error_codes_choose_recovery_actions() {
    use pop_contracts::relay::PauseKind;
    use pop_contracts::relay::frame::ErrorCode;
    let origin = Document::create(&app(SCHEMA), r#"{"title":"Seed","done":false}"#).unwrap();
    for (code, kind) in [
        (ErrorCode::Protocol.into(), PauseKind::NeedsUpdate),
        (ErrorCode::App.into(), PauseKind::Refused),
        (ErrorCode::Malformed.into(), PauseKind::Refused),
        (ErrorCode::Ended.into(), PauseKind::Ended),
        (255, PauseKind::Refused),
    ] {
        let (mut doc, mut session) = admitted(&origin.checkpoint().unwrap(), &version(&origin));
        session.receive(&mut doc, &Frame::Error { code, message: "reason".into() }.encode());
        assert_eq!(session.status(), &Status::Paused { kind, reason: "reason".into() });
    }
}

#[test]
fn compacted_backfill_merges_with_unsent_local_edits_and_later_records() {
    let origin = Document::create(&app(SCHEMA), r#"{"title":"Seed","done":false}"#).unwrap();
    let seed = origin.checkpoint().unwrap();
    let mut writer = Document::open(&app(SCHEMA), &seed, &[]).unwrap();
    let (record, heads) = foreign(&seed, |raw| {
        raw.get_map("data").insert("done", true).unwrap();
    });
    writer.import_remote(&[&record.bytes]).unwrap();
    let mut relay =
        Relay { app: APP, records: vec![record], heads: heads.iter().map(|(&p, &n)| (p, n as u32)).collect() };
    relay.compact(pop_core::sync::Base {
        frontier: heads,
        start: writer.base_start(),
        bytes: writer.base(&version(&writer)).unwrap(),
    });
    let (later, _) = foreign(&writer.checkpoint().unwrap(), |raw| {
        let Some(loro::ValueOrContainer::Container(loro::Container::Text(title))) = raw.get_map("data").get("title")
        else {
            panic!()
        };
        title.insert(0, "Later ").unwrap();
    });
    relay.reply(&Frame::Push { id: 1, records: vec![later] }.encode());
    let mut joiner = Replica::open(&app(SCHEMA), seed);
    joiner
        .doc
        .apply_json(r#"{"intents":[{"type":"set","path":["title"],"value":"Local Seed"}]}"#, Origin::Page)
        .unwrap();
    joiner.view = View::of(&joiner.doc);
    joiner.connect();
    let mut replicas = [joiner];
    let mut joined = [false];
    while !replicas[0].up.is_empty() || !replicas[0].down.is_empty() {
        deliver(&mut relay, &mut replicas, &mut joined, 0);
        replicas[0].receive();
    }
    let mut expected = Document::open(&app(SCHEMA), &writer.checkpoint().unwrap(), &[]).unwrap();
    for record in &relay.records {
        expected.import_remote(&[&record.bytes]).unwrap();
    }
    assert_eq!(support::value(&replicas[0].doc), support::value(&expected));
    assert_eq!(replicas[0].session.status(), &Status::Synced);
    assert!(support::value(&expected)["title"].as_str().unwrap().contains("Local"));
}

#[test]
fn read_only_offline_and_terminally_paused_sessions_cannot_compact() {
    let mut doc = Document::create(&app(SCHEMA), r#"{"title":"Seed","done":false}"#).unwrap();
    let mut session = Session::new(APP);
    assert!(base_of(&session, &doc).is_none());
    session.connect(&doc);
    let vv = version(&doc).iter().map(|(&p, &n)| (p, n as u32)).collect();
    session.receive(&mut doc, &Frame::Welcome { permission: Permission::Read, version: vv }.encode());
    session.receive(&mut doc, &Frame::CaughtUp.encode());
    assert!(base_of(&session, &doc).is_none());
    session.pause(pop_contracts::relay::PauseKind::Refused, "refused".into());
    assert!(base_of(&session, &doc).is_none());
}

// Hosts route on the phase and the pause, never on the status's debug text.
#[test]
fn status_gives_hosts_a_phase_and_a_pause() {
    use pop_contracts::relay::PauseKind;
    use pop_contracts::relay::SyncPhase;
    use pop_core::sync::Pause;
    let phases = [
        (Status::Offline, SyncPhase::Offline),
        (Status::Syncing, SyncPhase::Syncing),
        (Status::Synced, SyncPhase::Synced),
    ];
    for (status, phase) in phases {
        let state = status.state();
        assert_eq!((state.phase, state.pause), (phase, None));
    }
    let paused = Status::Paused { kind: PauseKind::NeedsUpdate, reason: "Update pop".into() }.state();
    assert_eq!(paused.phase, SyncPhase::Paused);
    assert_eq!(paused.pause, Some(Pause { kind: PauseKind::NeedsUpdate, reason: "Update pop".into() }));
    assert_eq!(
        serde_json::to_value(&paused).unwrap(),
        json!({"phase":"paused","pause":{"kind":"needsUpdate","reason":"Update pop"}})
    );
    assert_eq!(serde_json::to_value(Status::Synced.state()).unwrap(), json!({"phase":"synced"}));
}

#[test]
fn offline_backlog_uses_bounded_causally_complete_updates() {
    let schema = app(r#"{"kind":"object","properties":{"title":{"kind":"string"},"done":{"kind":"boolean"}}}"#);
    let origin = Document::create(&schema, r#"{"title":"initial","done":false}"#).unwrap();
    let seed = origin.checkpoint().unwrap();
    let mut source = Document::open(&schema, &seed, &[]).unwrap();
    let mut target = Document::open(&schema, &seed, &[]).unwrap();
    let mut random = 91_u64;
    for index in 0..12 {
        let value: String = (0..65_536).map(|_| char::from(b' ' + (next(&mut random) % 95) as u8)).collect();
        source
            .apply_json(&json!({"intents":[{"type":"set","path":["title"],"value":value}]}).to_string(), Origin::Page)
            .unwrap();
        if index == 5 {
            let before = version(&source);
            let mut other = Document::open(&schema, &source.checkpoint().unwrap(), &[]).unwrap();
            other.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
            let raw = LoroDoc::new();
            raw.import(&other.checkpoint().unwrap()).unwrap();
            let update = raw.export(ExportMode::updates(&before)).unwrap();
            source.import_remote(&[&update]).unwrap();
        }
    }
    let mut session = Session::new(APP);
    session.connect(&source);
    let welcome = Frame::Welcome {
        permission: Permission::Write,
        version: version(&origin).iter().map(|(&p, &n)| (p, n as u32)).collect(),
    };
    let mut queue: VecDeque<_> = session.receive(&mut source, &welcome.encode()).send.into();
    queue.extend(session.receive(&mut source, &Frame::CaughtUp.encode()).send);
    let mut frames = 0;
    while let Some(bytes) = queue.pop_front() {
        assert!(bytes.len() <= 256 * 1024, "offline backlog must be batched; got {} bytes", bytes.len());
        let Frame::Push { id, records } = Frame::decode(&bytes).unwrap() else { panic!("expected push") };
        let updates: Vec<_> = records.iter().map(|record| record.bytes.as_slice()).collect();
        target.import_remote(&updates).expect("each batch must be a valid causal document state");
        queue.extend(session.receive(&mut source, &Frame::Ack { id, status: Ack::Stored }.encode()).send);
        frames += 1;
        assert!(frames < 100, "the backlog must make bounded progress");
    }
    assert!(frames > 1);
    assert_eq!(support::value(&target), support::value(&source));
    assert_eq!(version(&target), version(&source));
    assert_eq!(session.status(), &Status::Synced);
}

/// Retained history a replica cannot accept (here, intermediate states the app never
/// validated) cannot be relayed or rebuilt: sharing pauses as outgrown. The copy keeps
/// editing locally and pushes nothing more.
#[test]
fn unrelayable_retained_history_pauses_outgrown_and_keeps_local_editing() {
    let schema = app(r#"{"kind":"object","properties":{"title":{"kind":"string"},"done":{"kind":"boolean"}}}"#);
    let origin = Document::create(&schema, r#"{"title":"initial","done":false}"#).unwrap();
    let seed = origin.checkpoint().unwrap();
    let raw = LoroDoc::new();
    raw.import(&seed).unwrap();
    raw.set_change_merge_interval(-1);
    // A saved checkpoint may have valid current state with older invalid prefixes.
    for value in 0..129 {
        raw.get_map("data").insert("title", value).unwrap();
        raw.commit();
    }
    raw.get_map("data").insert("title", "retained").unwrap();
    raw.commit();
    let checkpoint = raw.export(ExportMode::Snapshot).unwrap();
    let mut source = Document::open(&schema, &checkpoint, &[]).unwrap();
    let mut session = Session::new(APP);
    session.connect(&source);
    let welcome = Frame::Welcome {
        permission: Permission::Write,
        version: version(&origin).iter().map(|(&p, &n)| (p, n as u32)).collect(),
    };
    assert!(session.receive(&mut source, &welcome.encode()).send.is_empty());
    assert!(matches!(session.status(), Status::Paused { kind: pop_contracts::relay::PauseKind::Outgrown, .. }));
    source.apply_json(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#, Origin::Page).unwrap();
    assert_eq!(support::value(&source)["done"], true, "an outgrown copy keeps editing locally");
    assert!(session.push(&source).is_none(), "an outgrown copy pushes nothing");
}
