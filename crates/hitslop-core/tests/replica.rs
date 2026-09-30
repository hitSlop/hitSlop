//! Failure: a refused batch rebuilt the owner with `LoroDoc::fork_at`, and that replica
//! later resolved concurrent writes to one field differently from every other replica,
//! although all held the same operations (reproduced with plain Loro 1.16.2).
//! Oracle: a fresh replica opened from the seed plus every peer's updates.
mod support;
use support::Edit;
use hitslop_core::{Code, Document};
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};

fn next(rng: &mut u64) -> u64 { *rng ^= *rng << 13; *rng ^= *rng >> 7; *rng ^= *rng << 17; *rng }
fn value(doc: &Document) -> Value { serde_json::from_str::<Value>(&doc.snapshot().unwrap()).unwrap()["value"].clone() }

// Failure: replay-based rollback cannot reconstruct a trimmed document. A rejected
// two-intent batch on these bytes used to replace the current value with {}.
fn trimmed_document() -> (String, Vec<u8>, Vec<u8>) {
    let schema = json!({"kind":"object","properties":{"title":{"kind":"text"}}}).to_string();
    let mut source = Document::create(&schema, r#"{"title":"initial"}"#).unwrap();
    for i in 0..20 {
        source.apply_batch(&json!({"intents":[{"type":"set","path":["title"],"value":format!("edit {i}")}]}).to_string()).unwrap();
    }
    let full = source.checkpoint().unwrap();
    let loro = LoroDoc::new();
    loro.import(&full).unwrap();
    let shallow = loro.export(ExportMode::shallow_snapshot(&loro.oplog_frontiers())).unwrap();
    let inspected = LoroDoc::new();
    inspected.import(&shallow).unwrap();
    assert!(inspected.is_shallow(), "fixture must actually trim history");
    (schema, full, shallow)
}

#[test]
fn shallow_checkpoint_is_refused() {
    let (schema, full, shallow) = trimmed_document();
    assert!(Document::open(&schema, &full, &[]).is_ok());
    assert_eq!(Document::open(&schema, &shallow, &[]).err().map(|e| e.code), Some(Code::InvalidBytes));
}

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

#[test]
fn refused_batches_keep_replicas_convergent() {
    let schema = json!({"kind":"object","properties":{
        "k": {"kind":"optional","inner":{"kind":"integer"}},
        "n": {"kind":"integer","max":5},
    }}).to_string();
    for seed in 1..300u64 {
        let mut rng = seed * 0x9e3779b1;
        let origin = Document::create(&schema, r#"{"k":0,"n":0}"#).unwrap();
        let (checkpoint, base) = (origin.checkpoint().unwrap(), origin.version());
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
