// Failure: counter values differ between live use, replay, reopen or replicas, or
// concurrent increments are lost. Oracles: independent integer arithmetic over every
// accepted increment, literal values, the patch consumer and fresh snapshots.
// Gap: Loro's Counter regroups float deltas on export (spikes/engine-placement/
// counter-probe.ts) and no hitSlop counter design had convergence evidence.
mod support;
use hitslop_core_spike::Document;
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};

const MAX_SAFE: i64 = 9_007_199_254_740_991;
fn schema() -> String {
    json!({"format":1,"root":{"kind":"object","properties":{
        "hits":{"kind":"counter"},
        "rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"},"votes":{"kind":"counter"}}}}
    }}})
    .to_string()
}
fn initial() -> String {
    json!({"hits":5,"rows":[{"$id":"r1","text":"a","votes":0}]}).to_string()
}
fn snapshot(d: &Document) -> Value {
    serde_json::from_str(&d.snapshot().unwrap()).unwrap()
}
fn increment(path: Value, by: i64) -> String {
    json!({"intents":[{"type":"increment","path":path,"by":by}]}).to_string()
}
fn checked(d: &mut Document, projected: &mut Value, reply: String) {
    let reply: Value = serde_json::from_str(&reply).unwrap();
    support::apply_patches(projected, &reply["patch"]["ops"]);
    let fresh = snapshot(d);
    assert_eq!(*projected, fresh["value"]);
    assert_eq!(reply["patch"]["issues"], fresh["issues"]);
}

#[test]
fn loro_counter_loses_precision_where_the_hitslop_counter_stays_exact() {
    // Upstream behaviour, reproduced: (1e16 - 1e16) + 1 replays as 0.
    let a = LoroDoc::new();
    let c = a.get_counter("c");
    c.increment(1e16).unwrap();
    a.commit();
    let seed = a.export(ExportMode::Snapshot).unwrap();
    let from = a.oplog_vv();
    c.increment(-1e16).unwrap();
    a.commit();
    c.increment(1.0).unwrap();
    a.commit();
    let b = LoroDoc::new();
    b.import(&seed).unwrap();
    b.import(&a.export(ExportMode::updates(&from)).unwrap()).unwrap();
    assert_eq!((c.get_value(), b.get_counter("c").get_value()), (1.0, 0.0));
    // Even individually safe increments go wrong once the running float sum
    // crosses 2^53: the exact integer total is 1.
    let live = LoroDoc::new();
    let lc = live.get_counter("c");
    for by in [9e15, 9e15, 1.0, -9e15, -9e15] {
        lc.increment(by).unwrap();
        live.commit();
    }
    assert_ne!(lc.get_value(), 1.0);

    // hitSlop counter: exact integers, bounded to the safe range.
    let mut d = Document::create(&schema(), &json!({"hits":0,"rows":[]}).to_string()).unwrap();
    let seed = d.checkpoint().unwrap();
    let v0 = d.version();
    d.apply(&increment(json!(["hits"]), 9_000_000_000_000_000)).unwrap();
    let e = d.apply(&increment(json!(["hits"]), 9_000_000_000_000_000)).unwrap_err();
    assert_eq!(e.code, "out_of_range");
    assert_eq!(snapshot(&d)["value"]["hits"], 9_000_000_000_000_000i64);
    for by in [1, -9_000_000_000_000_000, MAX_SAFE - 1, -MAX_SAFE + 1] {
        d.apply(&increment(json!(["hits"]), by)).unwrap();
    }
    assert_eq!(snapshot(&d)["value"]["hits"], 1);
    let replayed = Document::open(&schema(), &seed, &[d.export_since(&v0).unwrap()]).unwrap();
    assert_eq!(snapshot(&replayed)["value"]["hits"], 1);
    let reopened = Document::open(&schema(), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"]["hits"], 1);
    assert_eq!(snapshot(&reopened)["issues"], json!([]));
}

fn next(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}
#[test]
fn replicas_converge_to_the_exact_sum_under_any_delivery() {
    let mut rng = 0xc0417e5u64;
    for _round in 0..200 {
        let base = Document::create(&schema(), &initial()).unwrap();
        let seed = base.checkpoint().unwrap();
        let v0 = base.version();
        let mut replicas: Vec<Document> =
            (0..3).map(|_| Document::open(&schema(), &seed, &[]).unwrap()).collect();
        let mut projected: Vec<Value> = replicas.iter().map(|d| snapshot(d)["value"].clone()).collect();
        let (mut hits, mut votes) = (5i64, 0i64);
        let mut sent: Vec<Vec<u8>> = vec![];
        for _ in 0..30 {
            let k = next(&mut rng) as usize % 3;
            let by = (next(&mut rng) % 2_000_000_000_001) as i64 - 1_000_000_000_000;
            let by = if by == 0 { 1 } else { by };
            let (path, total) = if next(&mut rng) % 2 == 0 {
                (json!(["hits"]), &mut hits)
            } else {
                (json!(["rows", {"id":"r1"}, "votes"]), &mut votes)
            };
            let before = replicas[k].version();
            let reply = replicas[k].apply(&increment(path, by)).unwrap();
            *total += by;
            checked(&mut replicas[k], &mut projected[k], reply);
            sent.push(replicas[k].export_since(&before).unwrap());
            // Deliver a random earlier update to a random replica, possibly again.
            let j = next(&mut rng) as usize % 3;
            let pick = next(&mut rng) as usize % sent.len();
            let known = replicas[j].version();
            if let Ok(reply) = replicas[j].import(&sent[pick]) {
                checked(&mut replicas[j], &mut projected[j], reply);
            } else {
                // Missing causal dependencies: that delivery simply waits.
                assert_eq!(replicas[j].version(), known);
            }
        }
        // Final anti-entropy in a shuffled order, with duplicates.
        for round in 0..2 {
            for j in 0..3 {
                for i in (0..3).map(|x| (x + j + round) % 3) {
                    if i != j {
                        let bytes = replicas[i].export_since(&v0).unwrap();
                        let reply = replicas[j].import(&bytes).unwrap();
                        checked(&mut replicas[j], &mut projected[j], reply);
                    }
                }
            }
        }
        for d in &replicas {
            let view = snapshot(d);
            assert_eq!(view["value"]["hits"], hits);
            assert_eq!(view["value"]["rows"][0]["votes"], votes);
            assert_eq!(view["issues"], json!([]));
        }
    }
}

#[test]
fn restored_checkpoints_and_finder_copies_keep_every_increment() {
    let mut a = Document::create(&schema(), &initial()).unwrap();
    let old = a.checkpoint().unwrap();
    let v0 = a.version();
    a.apply(&increment(json!(["hits"]), 10)).unwrap();
    // An older checkpoint restored (or a Finder copy) edits independently.
    let mut b = Document::open(&schema(), &old, &[]).unwrap();
    b.apply(&increment(json!(["hits"]), 100)).unwrap();
    b.apply(&increment(json!(["rows", {"id":"r1"}, "votes"]), -3)).unwrap();
    let (ab, ba) = (a.export_since(&v0).unwrap(), b.export_since(&v0).unwrap());
    a.import(&ba).unwrap();
    b.import(&ab).unwrap();
    for d in [&a, &b] {
        assert_eq!(snapshot(d)["value"]["hits"], 115);
        assert_eq!(snapshot(d)["value"]["rows"][0]["votes"], -3);
    }
}

#[test]
fn anomalous_contributions_are_preserved_flagged_and_block_local_increments() {
    let mut d = Document::create(&schema(), &initial()).unwrap();
    let mut projected = snapshot(&d)["value"].clone();
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    let hits = peer.get_map("data").get("hits").unwrap().into_container().unwrap().into_map().unwrap();
    hits.insert("remote", 1.5).unwrap();
    peer.commit();
    let reply = d.import(&peer.export(ExportMode::updates(&from)).unwrap()).unwrap();
    checked(&mut d, &mut projected, reply);
    let view = snapshot(&d);
    assert_eq!(view["issues"], json!([{"code":"type_mismatch","path":["hits"]}]));
    assert_eq!(view["value"]["hits"]["remote"], 1.5);
    assert_eq!(d.apply(&increment(json!(["hits"]), 1)).unwrap_err().code, "type_mismatch");
    // Concurrent writers can together leave the safe range: preserved and flagged.
    let mut x = Document::create(&schema(), &json!({"hits":0,"rows":[]}).to_string()).unwrap();
    let mut y = Document::open(&schema(), &x.checkpoint().unwrap(), &[]).unwrap();
    let v0 = x.version();
    x.apply(&increment(json!(["hits"]), MAX_SAFE)).unwrap();
    y.apply(&increment(json!(["hits"]), 1)).unwrap();
    let mut projected = snapshot(&x)["value"].clone();
    let reply = x.import(&y.export_since(&v0).unwrap()).unwrap();
    checked(&mut x, &mut projected, reply);
    assert_eq!(snapshot(&x)["issues"], json!([{"code":"type_mismatch","path":["hits"]}]));
}

#[test]
fn counters_reject_set_zero_and_non_counter_targets() {
    let mut d = Document::create(&schema(), &initial()).unwrap();
    let before = snapshot(&d);
    for (batch, code) in [
        (increment(json!(["hits"]), 0), "out_of_range"),
        (increment(json!(["rows", {"id":"r1"}, "text"]), 1), "type_mismatch"),
        (json!({"intents":[{"type":"set","path":["hits"],"value":3}]}).to_string(), "type_mismatch"),
    ] {
        assert_eq!(d.apply(&batch).unwrap_err().code, code);
        assert_eq!(snapshot(&d), before);
    }
}
