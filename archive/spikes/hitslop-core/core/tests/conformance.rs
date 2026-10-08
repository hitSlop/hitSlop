mod support;
use support::apply_patches;
// Failure: bindings/port accept a wrong edit or publish part of a rejected batch.
// Oracle: literal fixtures adapted from current handles.test.ts identity/atomicity
// cases; Unicode result independently spelled out. Gap: current tests exercise TS,
// not the new Rust interpreter. No existing owner coverage is replaced.
use hitslop_core_spike::Document;
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../fixtures/checklist.json")).unwrap()
}
fn snapshot(d: &Document) -> Value {
    serde_json::from_str(&d.snapshot().unwrap()).unwrap()
}
fn batch(case: &Value, version: &str) -> String {
    let mut intents = case["intents"].clone();
    for op in intents.as_array_mut().unwrap() {
        if op["base"] == "$current" {
            op["base"] = json!(version);
        }
    }
    json!({"intents":intents}).to_string()
}

fn cases(errors: bool) {
    let f = fixture();
    for case in f["scenarios"].as_array().unwrap() {
        if case.get("error").is_some() != errors {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        let mut d = Document::create(
            &f["schema"].to_string(),
            &case.get("initial").unwrap_or(&f["initial"]).to_string(),
        )
        .unwrap();
        let before = snapshot(&d);
        let seed = d.checkpoint().unwrap();
        let version = d.version();
        let result = d.apply(&batch(case, &version));
        if let Some(expected) = case["error"].as_str() {
            assert_eq!(result.unwrap_err().code, expected, "{name}");
            assert_eq!(
                snapshot(&d),
                before,
                "{name}: rejected batch changed state/version/publication"
            );
        } else {
            let reply: Value = serde_json::from_str(&result.unwrap()).unwrap();
            assert_eq!(snapshot(&d)["value"], case["after"], "{name}");
            let mut patched = before["value"].clone();
            apply_patches(&mut patched, &reply["patch"]["ops"]);
            assert_eq!(
                patched, case["after"],
                "{name}: patch did not reconstruct state"
            );
            assert_eq!(reply["patch"]["issues"], snapshot(&d)["issues"]);
            let delta = d.export_since(&version).unwrap();
            let reopened = Document::open(&f["schema"].to_string(), &seed, &[delta]).unwrap();
            assert_eq!(
                snapshot(&reopened)["value"],
                case["after"],
                "{name}: incremental replay"
            );
            assert_eq!(reopened.version(), d.version());
            let reopened =
                Document::open(&f["schema"].to_string(), &d.checkpoint().unwrap(), &[]).unwrap();
            assert_eq!(
                snapshot(&reopened)["value"],
                case["after"],
                "{name}: checkpoint reopen"
            );
        }
    }
}

#[test]
fn literal_semantics() {
    cases(false);
}

#[test]
fn atomic_rejection() {
    cases(true);
}

#[test]
fn malformed_import_preserves_the_owner() {
    let f = fixture();
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let before = snapshot(&d);
    assert_eq!(
        d.import(b"not a loro snapshot").unwrap_err().code,
        "invalid_bytes"
    );
    assert_eq!(snapshot(&d), before);
}

#[test]
fn merged_anomaly_is_preserved_flagged_and_not_repaired_on_read() {
    let schema =
        json!({"format":1,"root":{"kind":"object","properties":{"done":{"kind":"boolean"}}}})
            .to_string();
    let mut d = Document::create(&schema, r#"{"done":false}"#).unwrap();
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    peer.get_map("data").insert("done", "invalid").unwrap();
    peer.commit();
    let reply: Value = serde_json::from_str(
        &d.import(&peer.export(ExportMode::updates(&from)).unwrap())
            .unwrap(),
    )
    .unwrap();
    let before = d.version();
    let view = snapshot(&d);
    assert_eq!(view["value"], json!({"done":"invalid"}));
    assert_eq!(
        view["issues"],
        json!([{"code":"type_mismatch","path":["done"]}])
    );
    assert_eq!(reply["patch"]["issues"], view["issues"]);
    assert_eq!(
        reply["patch"]["ops"],
        json!([{"type":"set","path":["done"],"value":"invalid"}])
    );
    assert_eq!(d.version(), before);
    assert_eq!(
        d.apply(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#)
            .unwrap_err()
            .code,
        "type_mismatch"
    );
    let reopened = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"], view["value"]);
    assert_eq!(snapshot(&reopened)["issues"], view["issues"]);
}

#[test]
fn independent_replicas_merge_and_duplicate_delivery_is_idempotent() {
    let f = fixture();
    let mut a = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let mut b = Document::open(&f["schema"].to_string(), &a.checkpoint().unwrap(), &[]).unwrap();
    let from = a.version();
    a.apply(&json!({"intents":[{"type":"splice","path":["title"],"base":from,"index":3,"delete":0,"insert":"X"}]}).to_string()).unwrap();
    b.apply(r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000001"},"done"],"value":true}]}"#).unwrap();
    let left = a.export_since(&from).unwrap();
    let right = b.export_since(&from).unwrap();
    a.import(&right).unwrap();
    b.import(&left).unwrap();
    b.import(&left).unwrap();
    assert_eq!(
        snapshot(&a)["value"],
        json!({"title":"abcX","hits":0,"rows":[{"$id":"00000000000000000000000000000001","text":"A","done":true},{"$id":"00000000000000000000000000000002","text":"B","done":false}]})
    );
    assert_eq!(snapshot(&a)["value"], snapshot(&b)["value"]);
    assert_eq!(a.version(), b.version());
}

#[test]
fn minted_ids_are_application_ids_and_survive_reopen() {
    let f = fixture();
    let mut d = Document::create(&f["schema"].to_string(), r#"{"title":"abc","hits":0,"rows":[]}"#).unwrap();
    let reply: Value = serde_json::from_str(&d.apply(r#"{"intents":[{"type":"insert","path":["rows"],"value":{"text":"new","done":false}}]}"#).unwrap()).unwrap();
    let id = reply["ids"][0].as_str().unwrap();
    assert_eq!(id.len(), 26);
    assert!(id
        .bytes()
        .all(|b| b"0123456789abcdefghjkmnpqrstvwxyz".contains(&b)));
    let reopened = Document::open(&f["schema"].to_string(), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"]["rows"][0]["$id"], id);
}

// Failure: the owner rebuilt after a late rejection stops publishing, loses its
// draft ancestry, or exports bytes that no longer replay. Oracle: independent patch
// consumer, fresh snapshots and a literal final title. Gap: atomic_rejection only
// checks the state immediately after the rejection.
#[test]
fn owner_keeps_working_after_a_late_rejection() {
    let f = fixture();
    let schema = f["schema"].to_string();
    let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let seed = d.checkpoint().unwrap();
    let v0 = d.version();
    let mut projected = snapshot(&d)["value"].clone();
    let check = |d: &Document, projected: &mut Value, reply: &str| {
        let reply: Value = serde_json::from_str(reply).unwrap();
        apply_patches(projected, &reply["patch"]["ops"]);
        assert_eq!(*projected, snapshot(d)["value"]);
    };
    let session = snapshot(&d)["session"].clone();
    let draft = |seq: usize, index: usize, text: &str, base: &str| {
        let mut r = json!({"session":session,"draft":"title","sequence":seq,"base":base,"path":["title"],"index":index,"delete":0,"insert":text,"selectionStart":index+1,"selectionEnd":index+1});
        if seq > 1 { r["parent"] = json!(seq - 1); }
        r.to_string()
    };
    let r = d.text(&draft(1, 3, "X", &v0)).unwrap();
    check(&d, &mut projected, &r);
    // Late rejection: the first intent mutated before the second failed.
    let late = r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000001"},"done"],"value":true},{"type":"remove","path":["rows"],"id":"missing"}]}"#;
    assert_eq!(d.apply(late).unwrap_err().code, "path_not_found");
    assert_eq!(snapshot(&d)["value"], projected);
    // Local edits and remote imports still publish.
    let r = d.apply(r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000002"},"done"],"value":true}]}"#).unwrap();
    check(&d, &mut projected, &r);
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    let rows = peer.get_map("data").get("rows").unwrap().into_container().unwrap().into_movable_list().unwrap();
    rows.mov(0, 1).unwrap();
    peer.commit();
    let r = d.import(&peer.export(ExportMode::updates(&from)).unwrap()).unwrap();
    check(&d, &mut projected, &r);
    // The draft continues: the owner moved on, so this uses the authored branch.
    let r = d.text(&draft(2, 4, "Y", &v0)).unwrap();
    check(&d, &mut projected, &r);
    // And again with nothing intervening: the direct path.
    let r = d.text(&draft(3, 5, "Z", &v0)).unwrap();
    check(&d, &mut projected, &r);
    assert_eq!(snapshot(&d)["value"]["title"], "abcXYZ");
    // Bytes exported by the rebuilt owner replay from before the rejection.
    let replayed = Document::open(&schema, &seed, &[d.export_since(&v0).unwrap()]).unwrap();
    assert_eq!(snapshot(&replayed)["value"], projected);
    let reopened = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"], projected);
}
