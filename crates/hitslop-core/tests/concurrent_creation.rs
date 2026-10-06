//! Failure: two replicas that first created the same absent optional value or record entry
//! each made their own Loro container; the map showed one and hid the other's content,
//! although every replica held the same operations. Values that can be created lazily
//! now live in mergeable containers (deterministic identity), so this file also pins what
//! that identity must not break: cleared content stays cleared, rows recreated under one
//! list identity are addressed by their new IDs, and removed content leaves trimmed
//! documents. Oracles: a fresh replica opened from the seed plus every peer's updates, the
//! full snapshot against the page's publication-maintained view, and full and trimmed
//! reopens.
mod support;
use hitslop_core::{Code, Document, Error};
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};
use support::{Edit, View};

fn schema() -> String {
    let int = json!({"kind":"optional","inner":{"kind":"integer"}});
    json!({"kind":"object","properties":{
        "n": {"kind":"integer"},
        "profile": {"kind":"optional","inner":{"kind":"object","properties":{"a": int, "b": int, "c": int}}},
        "card": {"kind":"optional","inner":{"kind":"object","properties":{
            "title": {"kind":"string"}, "count": {"kind":"integer"}, "extra": int,
        }}},
        "note": {"kind":"optional","inner":{"kind":"text"}},
        "entries": {"kind":"record","value":{"kind":"object","properties":{"a": int, "b": int}}},
        "box": {"kind":"optional","inner":{"kind":"object","properties":{
            "title": {"kind":"text"},
            "rows": {"kind":"list","item":{"kind":"object","properties":{"name":{"kind":"string"}}}},
            "hits": {"kind":"counter"},
            "tags": {"kind":"list","item":{"kind":"string"}},
        }}},
        "rows": {"kind":"list","item":{"kind":"object","properties":{
            "body": {"kind":"text"},
            "note": {"kind":"optional","inner":{"kind":"text"}},
        }}},
    }}).to_string()
}
const INITIAL: &str = r#"{"n":0,"entries":{},"rows":[]}"#;
fn batch(intents: Value) -> String { json!({ "intents": intents }).to_string() }
/// What replicas holding the same operations must agree on; `sequence` is per owner.
fn state(doc: &Document) -> Value {
    let snapshot: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
    json!({"value": snapshot["value"], "issues": snapshot["issues"], "version": snapshot["version"]})
}
fn value(doc: &Document) -> Value { state(doc)["value"].clone() }
fn trimmed(checkpoint: &[u8]) -> Vec<u8> {
    let loro = LoroDoc::new();
    loro.import(checkpoint).unwrap();
    loro.export(ExportMode::shallow_snapshot(&loro.oplog_frontiers())).unwrap()
}
/// A full and a history-trimmed reopen read exactly as `doc` does.
fn reopens(doc: &Document) {
    let checkpoint = doc.checkpoint().unwrap();
    for (kind, bytes) in [("full", checkpoint.clone()), ("trimmed", trimmed(&checkpoint))] {
        let reopened = Document::open(&schema(), &bytes, &[]).unwrap();
        assert_eq!(state(&reopened)["value"], state(doc)["value"], "{kind} reopen: value");
        assert_eq!(state(&reopened)["issues"], state(doc)["issues"], "{kind} reopen: issues");
    }
}
fn sorted(text: &Value) -> Vec<char> {
    let mut chars: Vec<char> = text.as_str().unwrap().chars().collect();
    chars.sort();
    chars
}
fn ids(rows: &Value) -> Vec<&str> { rows.as_array().unwrap().iter().map(|r| r["$id"].as_str().unwrap()).collect() }

/// A replica with the page's view of it, checked against the snapshot after every change.
struct Peer {
    doc: Document,
    view: View,
}
impl Peer {
    fn open(checkpoint: &[u8]) -> Self {
        let doc = Document::open(&schema(), checkpoint, &[]).unwrap();
        Self { view: View::of(&doc), doc }
    }
    fn apply(&mut self, intents: Value) {
        let publication = self.doc.apply(&batch(intents)).unwrap();
        self.view.publish(&publication);
        self.view.check(&self.doc, "after a local batch");
    }
    fn refused(&mut self, intents: Value) -> Error {
        let before = self.doc.snapshot().unwrap();
        let error = self.doc.apply(&batch(intents)).unwrap_err();
        assert_eq!(self.doc.snapshot().unwrap(), before, "a refused batch changes nothing");
        error
    }
    fn merge(&mut self, bytes: &[u8]) {
        let publication = self.doc.merge(bytes).unwrap();
        self.view.publish(&publication);
        self.view.check(&self.doc, "after a merge");
    }
    fn undo(&mut self) {
        if let Some(publication) = self.doc.undo().unwrap().publication { self.view.publish(&publication); }
        self.view.check(&self.doc, "after undo");
    }
    fn redo(&mut self) {
        if let Some(publication) = self.doc.redo().unwrap().publication { self.view.publish(&publication); }
        self.view.check(&self.doc, "after redo");
    }
}

/// The seed document after `intents`, as a checkpoint and its version.
fn seed(intents: Value) -> (Vec<u8>, String) {
    let mut doc = Document::create(&schema(), INITIAL).unwrap();
    if !intents.as_array().unwrap().is_empty() {
        doc.apply(&batch(intents)).unwrap();
    }
    (doc.checkpoint().unwrap(), doc.version())
}
/// Exchanges every peer's updates since the seed. Each peer, a fresh replica of the seed
/// plus all updates, and full and trimmed reopens must agree; returns their state.
fn settle(peers: &mut [Peer], checkpoint: &[u8], base: &str) -> Value {
    let updates: Vec<_> = peers.iter().map(|p| p.doc.export_since(base).unwrap()).collect();
    for peer in peers.iter_mut() {
        for update in &updates { peer.merge(update); }
    }
    let fresh = state(&Document::open(&schema(), checkpoint, &updates).unwrap());
    for (index, peer) in peers.iter().enumerate() {
        assert_eq!(state(&peer.doc), fresh, "peer {index} diverged from a fresh merge");
    }
    reopens(&peers[0].doc);
    fresh
}
/// Two peers each apply their batch to the same seed, then exchange.
fn concurrently(seed_intents: Value, edits: [Value; 2]) -> Value {
    let (checkpoint, base) = seed(seed_intents);
    let mut peers: Vec<Peer> = edits.into_iter().map(|intents| {
        let mut peer = Peer::open(&checkpoint);
        peer.apply(intents);
        peer
    }).collect();
    settle(&mut peers, &checkpoint, &base)
}

// Fails on layout 1 as first shipped: one side's content was hidden.
#[test]
fn concurrently_created_optional_objects_merge() {
    let merged = concurrently(json!([]), [
        json!([{"type":"set","path":["profile"],"value":{"a":1}}]),
        json!([{"type":"set","path":["profile"],"value":{"b":2}}]),
    ]);
    assert_eq!(merged["value"]["profile"], json!({"a":1,"b":2}));
}

#[test]
fn concurrently_created_optional_text_merges() {
    let merged = concurrently(json!([]), [
        json!([{"type":"set","path":["note"],"value":"x"}]),
        json!([{"type":"set","path":["note"],"value":"y"}]),
    ]);
    assert_eq!(sorted(&merged["value"]["note"]), ['x', 'y']);
}

#[test]
fn concurrently_created_record_entries_merge() {
    let merged = concurrently(json!([]), [
        json!([{"type":"set","path":["entries","k"],"value":{"a":1}}]),
        json!([{"type":"set","path":["entries","k"],"value":{"b":2}}]),
    ]);
    assert_eq!(merged["value"]["entries"]["k"], json!({"a":1,"b":2}));
}

// The required collections inside a lazily created object are created by both peers too.
#[test]
fn concurrently_created_collections_keep_both_sides() {
    let merged = concurrently(json!([]), [
        json!([{"type":"set","path":["box"],"value":{"title":"T","rows":[{"$id":"r0","name":"zero"}],"hits":2,"tags":["x"]}}]),
        json!([{"type":"set","path":["box"],"value":{"title":"U","rows":[{"$id":"r1","name":"one"}],"hits":3,"tags":["y"]}}]),
    ]);
    let made = &merged["value"]["box"];
    assert_eq!(sorted(&made["title"]), ['T', 'U']);
    let mut rows = ids(&made["rows"]);
    rows.sort();
    assert_eq!(rows, ["r0", "r1"]);
    assert_eq!(made["hits"], 5, "initial counter values add like increments");
    let mut tags: Vec<&str> = made["tags"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
    tags.sort();
    assert_eq!(tags, ["x", "y"]);
}

// A row's optional text is lazily created; its identity comes from the row.
#[test]
fn concurrently_created_text_in_a_row_merges() {
    let merged = concurrently(json!([{"type":"insert","path":["rows"],"id":"r","value":{"body":"b"}}]), [
        json!([{"type":"set","path":["rows",{"id":"r"},"note"],"value":"x"}]),
        json!([{"type":"set","path":["rows",{"id":"r"},"note"],"value":"y"}]),
    ]);
    assert_eq!(sorted(&merged["value"]["rows"][0]["note"]), ['x', 'y']);
}

// Guards the identity: a mergeable child keeps its state after its key is removed, so
// a concurrent creation would show what the other peer cleared.
#[test]
fn a_concurrent_creation_does_not_show_cleared_content() {
    let (checkpoint, base) = seed(json!([]));
    let mut clearer = Peer::open(&checkpoint);
    clearer.apply(json!([{"type":"set","path":["profile"],"value":{"a":1,"c":1}}]));
    clearer.apply(json!([{"type":"clear","path":["profile"]}]));
    let mut creator = Peer::open(&checkpoint);
    // Enough history that the creation is the later write.
    for n in 1..=10 { creator.apply(json!([{"type":"set","path":["n"],"value":n}])); }
    creator.apply(json!([{"type":"set","path":["profile"],"value":{"a":2}}]));
    let merged = settle(&mut [clearer, creator], &checkpoint, &base);
    assert_eq!(merged["value"]["profile"], json!({"a":2}));
}

// Documented: a clear removes what the clearing peer saw. A write it never saw stays in
// the hidden value and reappears when a peer that never saw it either recreates the value.
#[test]
fn writes_the_clearer_never_saw_reappear_on_recreation() {
    let (checkpoint, base) = seed(json!([
        {"type":"set","path":["box"],"value":{"title":"T","rows":[{"$id":"r0","name":"zero"}],"hits":0,"tags":[]}},
        {"type":"set","path":["profile"],"value":{"a":1}},
    ]));
    let mut clearer = Peer::open(&checkpoint);
    clearer.apply(json!([{"type":"clear","path":["box"]},{"type":"clear","path":["profile"]}]));
    let mut editor = Peer::open(&checkpoint);
    editor.apply(json!([
        {"type":"increment","path":["box","hits"],"by":4},
        {"type":"insert","path":["box","tags"],"value":"late"},
        {"type":"set","path":["box","title"],"value":"T!"},
        {"type":"insert","path":["box","rows"],"id":"r1","value":{"name":"one"}},
        {"type":"set","path":["profile","b"],"value":2},
    ]));
    let mut creator = Peer::open(&checkpoint);
    creator.merge(&clearer.doc.export_since(&base).unwrap());
    creator.apply(json!([
        {"type":"set","path":["box"],"value":{"title":"N","rows":[],"hits":1,"tags":["n"]}},
        {"type":"set","path":["profile"],"value":{"a":3}},
    ]));
    let merged = settle(&mut [clearer, editor, creator], &checkpoint, &base);
    let made = &merged["value"]["box"];
    assert_eq!(merged["value"]["profile"], json!({"a":3,"b":2}));
    assert_eq!(made["hits"], 5);
    let mut tags: Vec<&str> = made["tags"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
    tags.sort();
    assert_eq!(tags, ["late", "n"]);
    assert_eq!(sorted(&made["title"]), ['!', 'N'], "the cleared T is gone; the unseen ! is not");
    assert_eq!(ids(&made["rows"]), ["r1"]);
    assert_eq!(merged["issues"], json!([]));
}

// Setting a present object writes only what changed, so a concurrent edit to a field it
// leaves unchanged survives.
#[test]
fn setting_a_present_object_keeps_concurrently_edited_fields() {
    let merged = concurrently(json!([{"type":"set","path":["profile"],"value":{"a":1,"b":1}}]), [
        json!([{"type":"set","path":["profile"],"value":{"a":1,"b":5}}]),
        json!([{"type":"set","path":["profile","a"],"value":9}]),
    ]);
    assert_eq!(merged["value"]["profile"], json!({"a":9,"b":5}));
}

#[test]
fn setting_after_clear_does_not_resurface_old_content() {
    let (checkpoint, _) = seed(json!([]));
    let mut doc = Peer::open(&checkpoint);
    doc.apply(json!([
        {"type":"set","path":["profile"],"value":{"a":1,"b":2}},
        {"type":"set","path":["note"],"value":"old"},
        {"type":"set","path":["entries","k"],"value":{"a":1,"b":1}},
        {"type":"set","path":["box"],"value":{"title":"Old","rows":[{"$id":"r0","name":"zero"}],"hits":3,"tags":["x"]}},
    ]));
    doc.apply(json!([
        {"type":"clear","path":["profile"]}, {"type":"clear","path":["note"]},
        {"type":"clear","path":["entries","k"]}, {"type":"clear","path":["box"]},
    ]));
    doc.apply(json!([
        {"type":"set","path":["profile"],"value":{"a":3}},
        {"type":"set","path":["note"],"value":"new"},
        {"type":"set","path":["entries","k"],"value":{"b":2}},
        {"type":"set","path":["box"],"value":{"title":"New","rows":[{"$id":"r1","name":"one"}],"hits":1,"tags":["y"]}},
    ]));
    // Cleared and set again within one batch.
    doc.apply(json!([
        {"type":"clear","path":["profile"]}, {"type":"set","path":["profile"],"value":{"c":7}},
        {"type":"clear","path":["note"]}, {"type":"set","path":["note"],"value":"again"},
        {"type":"clear","path":["entries","k"]}, {"type":"set","path":["entries","k"],"value":{"a":4}},
        {"type":"clear","path":["box"]},
        {"type":"set","path":["box"],"value":{"title":"Again","rows":[{"$id":"r2","name":"two"}],"hits":2,"tags":["z"]}},
    ]));
    assert_eq!(value(&doc.doc), json!({
        "n":0,"rows":[],"profile":{"c":7},"note":"again","entries":{"k":{"a":4}},
        "box":{"title":"Again","rows":[{"$id":"r2","name":"two"}],"hits":2,"tags":["z"]},
    }));
    reopens(&doc.doc);
}

const BOX: &str = r#"{"title":"T","rows":[{"$id":"r0","name":"zero"}],"hits":0,"tags":[]}"#;
fn with_box() -> Peer {
    let box_value: Value = serde_json::from_str(BOX).unwrap();
    Peer::open(&seed(json!([{"type":"set","path":["box"],"value":box_value}])).0)
}

// A list recreated in the same batch keeps its Loro identity; later intents in the batch
// must address its new rows, including when an earlier intent already changed it.
#[test]
fn rows_recreated_in_one_batch_are_addressed_by_their_new_ids() {
    for earlier in [json!([]), json!([{"type":"insert","path":["box","rows"],"id":"r9","value":{"name":"nine"}}])] {
        let mut doc = with_box();
        let mut intents = earlier.as_array().unwrap().clone();
        intents.extend(json!([
            {"type":"clear","path":["box"]},
            {"type":"set","path":["box"],"value":{"title":"","rows":[{"$id":"r1","name":"one"},{"$id":"r2","name":"two"}],"hits":0,"tags":[]}},
            {"type":"set","path":["box","rows",{"id":"r1"},"name"],"value":"uno"},
            {"type":"move","path":["box","rows"],"id":"r2","at":{"before":"r1"}},
            {"type":"insert","path":["box","rows"],"id":"r3","value":{"name":"three"},"at":{"after":"r1"}},
            {"type":"remove","path":["box","rows"],"id":"r2"},
        ]).as_array().unwrap().iter().cloned());
        doc.apply(Value::Array(intents));
        assert_eq!(value(&doc.doc)["box"]["rows"], json!([{"$id":"r1","name":"uno"},{"$id":"r3","name":"three"}]));
        // The next batch reads the index publication maintained.
        doc.apply(json!([{"type":"insert","path":["box","rows"],"id":"r4","value":{"name":"four"},"at":{"before":"r3"}}]));
        assert_eq!(ids(&value(&doc.doc)["box"]["rows"]), ["r1", "r4", "r3"]);
        reopens(&doc.doc);
    }
}

#[test]
fn a_recreated_list_does_not_resolve_its_old_rows() {
    let mut doc = with_box();
    let error = doc.refused(json!([
        {"type":"clear","path":["box"]},
        {"type":"set","path":["box"],"value":{"title":"","rows":[{"$id":"r1","name":"one"}],"hits":0,"tags":[]}},
        {"type":"set","path":["box","rows",{"id":"r0"},"name"],"value":"ghost"},
    ]));
    assert_eq!((error.code, error.op_index), (Code::PathNotFound, Some(2)));
}

#[test]
fn a_refused_intent_after_recreation_leaves_the_document_unchanged() {
    let mut doc = with_box();
    let error = doc.refused(json!([
        {"type":"clear","path":["box"]},
        {"type":"set","path":["box"],"value":{"title":"N","rows":[{"$id":"r1","name":"one"}],"hits":1,"tags":["n"]}},
        {"type":"increment","path":["box","hits"],"by":0},
    ]));
    assert_eq!(error.op_index, Some(2));
    doc.apply(json!([{"type":"insert","path":["box","rows"],"id":"r5","value":{"name":"five"},"at":{"after":"r0"}}]));
    assert_eq!(ids(&value(&doc.doc)["box"]["rows"]), ["r0", "r5"]);
    reopens(&doc.doc);
}

#[test]
fn undo_and_redo_restore_cleared_recreated_and_removed_values() {
    let mut doc = with_box();
    doc.apply(json!([{"type":"insert","path":["rows"],"id":"q","value":{"body":"b","note":"hello"}}]));
    let original = value(&doc.doc);
    doc.apply(json!([{"type":"clear","path":["box"]}]));
    let cleared = value(&doc.doc);
    doc.undo();
    assert_eq!(value(&doc.doc), original);
    doc.redo();
    assert_eq!(value(&doc.doc), cleared);
    doc.apply(json!([{"type":"set","path":["box"],"value":{"title":"N","rows":[{"$id":"r1","name":"one"}],"hits":1,"tags":[]}}]));
    let recreated = value(&doc.doc);
    doc.undo();
    assert_eq!(value(&doc.doc), cleared);
    doc.redo();
    assert_eq!(value(&doc.doc), recreated);
    doc.apply(json!([{"type":"remove","path":["rows"],"id":"q"}]));
    let removed = value(&doc.doc);
    doc.undo();
    assert_eq!(value(&doc.doc), recreated, "the row returns with its optional text");
    doc.redo();
    assert_eq!(value(&doc.doc), removed);
    doc.apply(json!([{"type":"insert","path":["box","rows"],"id":"r2","value":{"name":"two"},"at":{"before":"r1"}}]));
    assert_eq!(ids(&value(&doc.doc)["box"]["rows"]), ["r2", "r1"]);
    reopens(&doc.doc);
}

// A replica may store kinds the core never writes, such as a plain Loro list; clearing and
// undoing around one preserves and flags it instead of failing.
#[test]
fn clearing_and_undoing_around_a_merged_plain_list_keeps_it_flagged() {
    let mut doc = with_box();
    let peer = LoroDoc::new();
    peer.import(&doc.doc.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    let Some(loro::ValueOrContainer::Container(loro::Container::Map(stored))) = peer.get_map("data").get("box") else {
        panic!("box");
    };
    stored.insert_container("extra", loro::LoroList::new()).unwrap().push(1).unwrap();
    peer.commit();
    doc.merge(&peer.export(ExportMode::updates(&from)).unwrap());
    let flagged = state(&doc.doc);
    assert_eq!(flagged["issues"], json!([{"code":"unknown_field","path":["box","extra"]}]));
    doc.apply(json!([{"type":"clear","path":["box"]}]));
    doc.undo();
    assert_eq!(state(&doc.doc)["value"], flagged["value"]);
    assert_eq!(state(&doc.doc)["issues"], flagged["issues"]);
    reopens(&doc.doc);
}

fn next(rng: &mut u64) -> u64 { *rng ^= *rng << 13; *rng ^= *rng >> 7; *rng ^= *rng << 17; *rng }
/// Text that snapshots cannot compress away, so a size bound sees whether it was kept.
fn prose(seed: u64) -> String {
    let mut rng = seed.wrapping_mul(0x9e3779b97f4a7c15) | 1;
    (0..5000).map(|_| char::from(b'a' + (next(&mut rng) % 26) as u8)).collect()
}

// Writes through the core never leave a visible object without its required fields,
// whichever of a concurrent clear and creation wins.
#[test]
fn concurrent_clears_and_creations_keep_required_fields() {
    for seed_number in 1..=support::workload("HITSLOP_CREATION_SEEDS", 64) as u64 {
        let mut rng = seed_number.wrapping_mul(0x9e3779b97f4a7c15);
        let present = next(&mut rng) % 2 == 0;
        let (checkpoint, base) = seed(if present {
            json!([{"type":"set","path":["card"],"value":{"title":"t","count":0}}])
        } else { json!([]) });
        let mut peers: Vec<Peer> = (0..2).map(|_| Peer::open(&checkpoint)).collect();
        for (p, peer) in peers.iter_mut().enumerate() {
            for n in 0..next(&mut rng) % 8 { peer.apply(json!([{"type":"set","path":["n"],"value":n}])); }
            for step in 0..1 + next(&mut rng) % 3 {
                let intent = match next(&mut rng) % 4 {
                    0 => json!({"type":"set","path":["card"],"value":{"title":format!("p{p}s{step}"),"count":step}}),
                    1 => json!({"type":"clear","path":["card"]}),
                    2 => json!({"type":"set","path":["card","count"],"value":10 + step}),
                    _ => json!({"type":"set","path":["card","extra"],"value":step}),
                };
                // Field edits of an absent card are refused; that is part of the workload.
                let _ = peer.doc.apply(&batch(json!([intent]))).map(|p| peer.view.publish(&p));
                peer.view.check(&peer.doc, "after a workload batch");
            }
        }
        let merged = settle(&mut peers, &checkpoint, &base);
        assert_eq!(merged["issues"], json!([]), "seed {seed_number}: {merged}");
    }
}

// Removed content must not outlive trimming: mergeable containers are retained by
// identity, so removal empties them first.
#[test]
fn removed_content_leaves_trimmed_documents() {
    let (checkpoint, _) = seed(json!([]));
    let mut doc = Peer::open(&checkpoint);
    let long = prose(1);
    doc.apply(json!([
        {"type":"insert","path":["rows"],"id":"q","value":{"body":long,"note":long}},
        {"type":"set","path":["note"],"value":long},
        {"type":"set","path":["entries","k"],"value":{"a":1}},
        {"type":"set","path":["box"],"value":{"title":long,"rows":[{"$id":"r0","name":long}],"hits":1,"tags":[long]}},
    ]));
    doc.apply(json!([
        {"type":"remove","path":["rows"],"id":"q"},
        {"type":"clear","path":["note"]},
        {"type":"clear","path":["entries","k"]},
        {"type":"clear","path":["box"]},
    ]));
    let bytes = trimmed(&doc.doc.checkpoint().unwrap());
    assert!(bytes.len() < 2000, "a trimmed document kept removed content: {} bytes", bytes.len());
    reopens(&doc.doc);
}

// Undo removes values through Loro's revert, which hides mergeable values without
// emptying them; the core empties them as a clear does.
#[test]
fn content_removed_by_undo_leaves_trimmed_documents() {
    let (checkpoint, _) = seed(json!([]));
    let mut doc = Peer::open(&checkpoint);
    let long = prose(1);
    doc.apply(json!([{"type":"insert","path":["rows"],"id":"q","value":{"body":"b","note":long}}]));
    doc.apply(json!([{"type":"set","path":["box"],"value":{"title":long,"rows":[{"$id":"r0","name":long}],"hits":1,"tags":[long]}}]));
    doc.undo();
    doc.undo();
    assert_eq!(value(&doc.doc), json!({"n":0,"entries":{},"rows":[]}));
    let bytes = trimmed(&doc.doc.checkpoint().unwrap());
    assert!(bytes.len() < 2000, "a trimmed document kept undone content: {} bytes", bytes.len());
    doc.redo();
    doc.redo();
    assert_eq!(value(&doc.doc)["rows"][0]["note"], json!(long));
    assert_eq!(value(&doc.doc)["box"]["title"], json!(long), "redo shows the content once");
    reopens(&doc.doc);
}
