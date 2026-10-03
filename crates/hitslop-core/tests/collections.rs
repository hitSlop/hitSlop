// Records, scalar lists and optional text. Failure: replicas that disagree after
// exchanging updates, a merged anomaly repaired on read, or a publication that drifts
// from a fresh snapshot. Concurrent creation is covered in `concurrent_creation.rs`.
// Oracle: equal snapshots on both replicas, literal issues, and an independent patch
// consumer compared with fresh snapshots over a random workload.
mod support;
use hitslop_core::Document;
use support::{Edit, View};
use loro::{ExportMode, LoroDoc, LoroMap};
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/collections.json")).unwrap()
}
fn view(d: &Document) -> Value {
    serde_json::from_str(&d.snapshot().unwrap()).unwrap()
}
fn apply(d: &mut Document, intents: Value) {
    d.apply(&json!({ "intents": intents }).to_string()).unwrap();
}
fn pair() -> (Document, Document, String) {
    let f = fixture();
    let a = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let b = Document::open(&f["schema"].to_string(), &a.checkpoint().unwrap(), &[]).unwrap();
    let base = a.version();
    (a, b, base)
}
fn exchange(a: &mut Document, b: &mut Document, base: &str) {
    let (left, right) = (a.export_since(base).unwrap(), b.export_since(base).unwrap());
    a.import(&right).unwrap();
    b.import(&left).unwrap();
}

#[test]
fn concurrent_edits_to_fields_of_one_entry_both_survive() {
    let (mut a, mut b, _) = pair();
    apply(&mut a, json!([{"type":"set","path":["cells","A1"],"value":{"input":"start"}}]));
    b.import(&a.export_since(&b.version()).unwrap()).unwrap();
    let base = a.version();
    apply(&mut a, json!([{"type":"set","path":["cells","A1","input"],"value":"typed"}]));
    apply(&mut b, json!([{"type":"set","path":["cells","A1","tint"],"value":"blue"}]));
    exchange(&mut a, &mut b, &base);
    assert_eq!(view(&a)["value"]["cells"]["A1"], json!({"input":"typed","tint":"blue"}));
    assert_eq!(view(&a)["value"], view(&b)["value"]);
}

#[test]
fn concurrent_scalar_list_inserts_both_survive_and_sets_converge() {
    let (mut a, mut b, base) = pair();
    apply(&mut a, json!([{"type":"insert","path":["presets"],"value":100,"index":1},{"type":"set","path":["pixels",{"index":0}],"value":"#a00"}]));
    apply(&mut b, json!([{"type":"insert","path":["presets"],"value":200},{"type":"set","path":["pixels",{"index":0}],"value":"#0b0"}]));
    exchange(&mut a, &mut b, &base);
    let presets = view(&a)["value"]["presets"].clone();
    assert_eq!(presets, json!([60, 100, 90, 200]));
    assert_eq!(view(&a)["value"], view(&b)["value"]);
    assert!(["#a00", "#0b0"].contains(&view(&a)["value"]["pixels"][0].as_str().unwrap()));
}

#[test]
fn page_typing_into_an_unset_optional_text_creates_it() {
    let f = fixture();
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let base = d.version();
    let request = json!({"base":base,"path":["notes"],"from":"","to":"Hi","selectionStart":2,"selectionEnd":2});
    let edit = d.edit_text(&request.to_string()).unwrap();
    assert_eq!(view(&d)["value"]["notes"], "Hi");
    let next = json!({"base":edit.authored,"path":["notes"],"from":"Hi","to":"Hi there","selectionStart":8,"selectionEnd":8});
    d.edit_text(&next.to_string()).unwrap();
    assert_eq!(view(&d)["value"]["notes"], "Hi there");
    // After a clear, an edit from the old text is refused rather than resurrecting it.
    d.apply(&json!({"intents":[{"type":"clear","path":["notes"]}]}).to_string()).unwrap();
    let stale = json!({"base":d.version(),"path":["notes"],"from":"Hi there","to":"Hi there!","selectionStart":9,"selectionEnd":9});
    assert_eq!(d.edit_text(&stale.to_string()).unwrap_err().code.as_str(), "path_not_found");
}

#[test]
fn merged_invalid_entries_and_elements_are_flagged_not_repaired() {
    let f = fixture();
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    let data = peer.get_map("data");
    let widths = data.get("widths").unwrap().into_container().unwrap().into_map().unwrap();
    widths.insert("B", "wide").unwrap();
    let presets = data.get("presets").unwrap().into_container().unwrap().into_movable_list().unwrap();
    presets.insert(0, 999i64).unwrap();
    let done = data.get("done").unwrap().into_container().unwrap().into_map().unwrap();
    let _: LoroMap = done.insert_container("__proto__", LoroMap::new()).unwrap();
    peer.commit();
    d.import(&peer.export(ExportMode::updates(&from)).unwrap()).unwrap();
    let snapshot = view(&d);
    assert_eq!(snapshot["value"]["widths"]["B"], "wide");
    assert_eq!(snapshot["value"]["presets"], json!([999, 60, 90]));
    assert_eq!(
        snapshot["issues"],
        json!([
            {"code":"invalid_key","path":["done","__proto__"]},
            {"code":"out_of_range","path":["presets",{"index":0}]},
            {"code":"type_mismatch","path":["widths","B"]}
        ])
    );
}

fn next(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}
fn random_op(rng: &mut u64, d: &Document) -> Value {
    let value = view(d)["value"].clone();
    let pixels = value["pixels"].as_array().unwrap().len();
    let key = format!("k{}", next(rng) % 5);
    let colour = format!("#{:03x}", next(rng) % 4096);
    match next(rng) % 8 {
        0 => json!({"type":"set","path":["done",key],"value":next(rng) % 2 == 0}),
        1 => json!({"type":"clear","path":["done",key]}),
        2 => json!({"type":"set","path":["cells",key],"value":{"input":"x"}}),
        3 if value["cells"].as_object().unwrap().contains_key(&key) => {
            json!({"type":"set","path":["cells",key,"tint"],"value":"red"})
        }
        4 => json!({"type":"insert","path":["pixels"],"value":colour,"index":next(rng) as usize % (pixels + 1)}),
        5 if pixels > 0 => json!({"type":"set","path":["pixels",{"index":next(rng) as usize % pixels}],"value":colour}),
        6 if pixels > 0 => json!({"type":"remove","path":["pixels"],"index":next(rng) as usize % pixels}),
        _ => json!({"type":"set","path":["habits",{"id":"h1"},"checkins",key],"value":1 + next(rng) % 3}),
    }
}
#[test]
fn seeded_collection_publications_match_fresh_snapshots() {
    let f = fixture();
    let schema = f["schema"].to_string();
    let mut rng = 0x5eed_c011u64;
    for _round in 0..60 {
        let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
        let mut projected = View::of(&d);
        for step in 0..40 {
            let reply = if step % 4 == 0 {
                let mut peer = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
                let base = d.version();
                for _ in 0..1 + next(&mut rng) % 3 {
                    let op = random_op(&mut rng, &peer);
                    peer.apply(&json!({"intents":[op]}).to_string()).unwrap();
                }
                d.merge(&peer.export_since(&base).unwrap()).unwrap()
            } else {
                let op = random_op(&mut rng, &d);
                d.apply(&json!({"intents":[op]}).to_string()).unwrap()
            };
            projected.publish(&reply);
            projected.check(&d, "publication");
            assert_eq!(projected.issues, json!([]));
        }
    }
}
