//! Failure: a refused batch rebuilt the owner with `LoroDoc::fork_at`, and that replica
//! later resolved concurrent writes to one field differently from every other replica,
//! although all held the same operations (reproduced with plain Loro 1.16.2).
//! Oracle: a fresh replica opened from the seed plus every peer's updates.
mod support;
use support::{Edit, View};
use hitslop_core::{Code, Document};
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};
use hitslop_core::Origin;

fn next(rng: &mut u64) -> u64 { *rng ^= *rng << 13; *rng ^= *rng >> 7; *rng ^= *rng << 17; *rng }
fn value(doc: &Document) -> Value { serde_json::from_str::<Value>(&doc.snapshot().unwrap()).unwrap()["value"].clone() }

/// A document's full checkpoint and the same document trimmed to its latest version.
fn trimmed_document() -> (String, Vec<u8>, Vec<u8>) {
    let schema = json!({"kind":"object","properties":{"title":{"kind":"text"}}}).to_string();
    let mut source = Document::create(&schema, r#"{"title":"initial"}"#).unwrap();
    for i in 0..20 {
        source.apply_batch(&json!({"intents":[{"type":"set","path":["title"],"value":format!("edit {i}")}]}).to_string(), Origin::Page).unwrap();
    }
    let full = source.checkpoint().unwrap();
    let shallow = trimmed(&full);
    let inspected = LoroDoc::new();
    inspected.import(&shallow).unwrap();
    assert!(inspected.is_shallow(), "fixture must actually trim history");
    (schema, full, shallow)
}

#[test]
fn a_shallow_checkpoint_opens_with_its_value_and_version() {
    let (schema, full, shallow) = trimmed_document();
    let (full, trimmed) = (Document::open(&schema, &full, &[]).unwrap(), Document::open(&schema, &shallow, &[]).unwrap());
    assert_eq!(value(&trimmed), value(&full));
    assert_eq!(trimmed.version(), full.version());
}

// Failure: rollback replayed the document from its first operation, which a trimmed
// document no longer has; a rejected two-intent batch replaced the value with {}.
#[test]
fn a_refused_batch_on_a_trimmed_document_keeps_its_value() {
    let (schema, _, shallow) = trimmed_document();
    let mut owner = Document::open(&schema, &shallow, &[]).unwrap();
    let mut view = View::of(&owner);
    let before = owner.snapshot().unwrap();
    let refused = owner.apply(&json!({"intents":[
        {"type":"set","path":["title"],"value":"mutated"},
        {"type":"set","path":["missing"],"value":"refused"},
    ]}).to_string());
    assert_eq!(refused.err().map(|e| e.op_index), Some(Some(1)));
    assert_eq!(owner.snapshot().unwrap(), before, "value, version, sequence and issues stay unchanged");
    view.publish(&owner.apply(&json!({"intents":[{"type":"set","path":["title"],"value":"after"}]}).to_string()).unwrap());
    view.check(&owner, "after a refused batch");
    let reopened = Document::open(&schema, &owner.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(value(&reopened), json!({"title":"after"}));
}

// Only the checkpoint may start history late.
#[test]
fn shallow_saved_update_is_refused() {
    let (schema, _, shallow) = trimmed_document();
    let empty = LoroDoc::new().export(ExportMode::Snapshot).unwrap();
    assert_eq!(Document::open(&schema, &empty, &[shallow]).err().map(|e| e.code), Some(Code::InvalidBytes));
}

#[test]
fn shallow_import_is_refused_without_changing_the_owner() {
    let (schema, full, shallow) = trimmed_document();
    let mut owner = Document::open(&schema, &full, &[]).unwrap();
    let before = owner.snapshot().unwrap();
    assert_eq!(owner.import(&shallow).err().map(|e| e.code), Some(Code::InvalidBytes));
    assert_eq!(owner.snapshot().unwrap(), before, "value, version, sequence and issues stay unchanged");
}

fn trimmed(checkpoint: &[u8]) -> Vec<u8> {
    let loro = LoroDoc::new();
    loro.import(checkpoint).unwrap();
    loro.export(ExportMode::shallow_snapshot(&loro.oplog_frontiers())).unwrap()
}

#[test]
fn refused_batches_keep_replicas_convergent() {
    convergence(|checkpoint| checkpoint);
}

#[test]
fn refused_batches_keep_shallow_replicas_convergent() {
    convergence(|checkpoint| trimmed(&checkpoint));
}

fn convergence(start: impl Fn(Vec<u8>) -> Vec<u8>) {
    let schema = json!({"kind":"object","properties":{
        "k": {"kind":"optional","inner":{"kind":"integer"}},
        "n": {"kind":"integer","max":5},
    }}).to_string();
    for seed in 1..=support::workload("HITSLOP_REPLICA_SEEDS", 299) as u64 {
        let mut rng = seed * 0x9e3779b1;
        let mut origin = Document::create(&schema, r#"{"k":0,"n":0}"#).unwrap();
        origin.apply(&json!({"intents":[{"type":"set","path":["n"],"value":1}]}).to_string()).unwrap();
        let (checkpoint, base) = (start(origin.checkpoint().unwrap()), origin.version());
        let mut peers: Vec<_> = (0..3).map(|_| Document::open(&schema, &checkpoint, &[]).unwrap()).collect();
        for step in 0..40 {
            let p = (next(&mut rng) % 3) as usize;
            let batch = match next(&mut rng) % 4 {
                0 => json!([{"type":"set","path":["k"],"value":step * 10 + p}]),
                1 => json!([{"type":"clear","path":["k"]}]),
                // Mutates, then is refused: the owner must roll back to the batch start.
                2 => json!([{"type":"set","path":["k"],"value":-1},{"type":"clear","path":["k"]},{"type":"set","path":["n"],"value":99}]),
                _ => {
                    let updates: Vec<_> = peers.iter().map(|d| d.export_since(&base).unwrap()).collect();
                    for peer in &mut peers { for update in &updates { peer.import(update).unwrap(); } }
                    let merged = value(&Document::open(&schema, &checkpoint, &updates).unwrap());
                    for (index, peer) in peers.iter().enumerate() {
                        assert_eq!(value(peer), merged, "seed {seed}, step {step}: peer {index} diverged from a fresh merge");
                    }
                    continue;
                }
            };
            let _ = peers[p].apply(&json!({"intents": batch}).to_string());
        }
    }
}

// Failure: a blob mixing changes a trimmed document can apply with changes that depend
// on history it trimmed was applied in part and then refused, so the document changed
// without a publication. Oracle: whatever landed is published.
#[test]
fn an_import_needing_trimmed_history_publishes_what_landed() {
    let schema = json!({"kind":"object","properties":{"k":{"kind":"integer"},"n":{"kind":"integer"}}}).to_string();
    let set = |field: &str, value: i64| json!({"intents":[{"type":"set","path":[field],"value":value}]}).to_string();
    let mut origin = Document::create(&schema, r#"{"k":0,"n":0}"#).unwrap();
    let (old, old_checkpoint) = (origin.version(), origin.checkpoint().unwrap());
    origin.apply(&set("n", 1)).unwrap();
    let (root, checkpoint) = (origin.version(), origin.checkpoint().unwrap());
    let mut owner = Document::open(&schema, &trimmed(&checkpoint), &[]).unwrap();
    let mut stale = Document::open(&schema, &old_checkpoint, &[]).unwrap();
    stale.apply(&set("k", 7)).unwrap();
    let mut fresh = Document::open(&schema, &checkpoint, &[]).unwrap();
    fresh.apply(&set("n", 2)).unwrap();
    let mut relay = Document::open(&schema, &checkpoint, &[]).unwrap();
    relay.import(&stale.export_since(&old).unwrap()).unwrap();
    relay.import(&fresh.export_since(&root).unwrap()).unwrap();
    let refused = owner.import(&relay.export_since(&root).unwrap());
    assert_eq!(refused.err().map(|e| e.code), Some(Code::StaleBase));
    assert_eq!(value(&owner), json!({"k":0,"n":2}), "the change that applies lands");
    assert_eq!(owner.sequence(), 1, "and is published");
}
