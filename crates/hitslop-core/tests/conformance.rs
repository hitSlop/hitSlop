mod support;
use support::{Edit, View, apply_patches, fixture, snapshot};
// Failure: the core accepts a wrong edit or publishes part of a rejected batch.
// Oracle: literal fixtures for identity and atomicity cases, with Unicode results
// spelled out independently.
use hitslop_core::Document;
use loro::{ExportMode, LoroDoc};
use serde_json::{json, Value};
use hitslop_core::Origin;

fn batch(case: &Value, version: &str) -> String {
    let mut intents = case["intents"].clone();
    for op in intents.as_array_mut().unwrap() {
        if op["base"] == "$current" {
            op["base"] = json!(version);
        }
    }
    json!({"intents":intents}).to_string()
}

/// Every literal scenario file, so a new kind's fixture runs with the rest.
fn fixtures() -> Vec<Value> {
    [include_str!("../fixtures/checklist.json"), include_str!("../fixtures/scalars.json"), include_str!("../fixtures/collections.json")]
        .into_iter()
        .map(|f| serde_json::from_str(f).unwrap())
        .collect()
}

fn cases(errors: bool) {
    for f in fixtures() {
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
                assert_eq!(result.unwrap_err().code.as_str(), expected, "{name}");
                assert_eq!(
                    snapshot(&d),
                    before,
                    "{name}: rejected batch changed state/version/publication"
                );
            } else {
                let reply: Value = serde_json::from_str(&result.unwrap()).unwrap();
                assert_eq!(snapshot(&d)["value"], case["after"], "{name}");
                let mut patched = before["value"].clone();
                apply_patches(&mut patched, &reply["ops"]);
                assert_eq!(
                    patched, case["after"],
                    "{name}: patch did not reconstruct state"
                );
                assert!(reply.get("issues").is_none_or(|issues| *issues == snapshot(&d)["issues"]));
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
                // Every supported kind must also survive the shared history path,
                // including optional containers, records and reordered lists.
                if before["value"] != case["after"] {
                    let mut view = View::of(&d);
                    for _ in 0..2 {
                        view.publish(&d.undo().unwrap().publication.unwrap());
                        assert_eq!(snapshot(&d)["value"], before["value"], "{name}: undo");
                        view.check(&d, name);
                        view.publish(&d.redo().unwrap().publication.unwrap());
                        assert_eq!(snapshot(&d)["value"], case["after"], "{name}: redo");
                        view.check(&d, name);
                    }
                }
            }
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
    let f = fixture("checklist");
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let before = snapshot(&d);
    assert_eq!(
        d.import(b"not a loro snapshot").unwrap_err().code.as_str(),
        "invalid_bytes"
    );
    assert_eq!(snapshot(&d), before);
}

#[test]
fn merged_anomaly_is_preserved_flagged_and_not_repaired_on_read() {
    let schema =
        json!({"kind":"object","properties":{"done":{"kind":"boolean"}}})
            .to_string();
    let mut d = Document::create(&schema, r#"{"done":false}"#).unwrap();
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    peer.get_map("data").insert("done", "invalid").unwrap();
    peer.get_map("data").insert("extra", "preserved").unwrap();
    peer.commit();
    let reply: Value = serde_json::from_str(
        &d.merge(&peer.export(ExportMode::updates(&from)).unwrap())
            .unwrap(),
    )
    .unwrap();
    let before = d.version();
    let view = snapshot(&d);
    assert_eq!(view["value"], json!({"done":"invalid","extra":"preserved"}));
    assert_eq!(
        view["issues"],
        json!([{"code":"type_mismatch","path":["done"]},{"code":"unknown_field","path":["extra"]}])
    );
    assert_eq!(reply["issues"], view["issues"]);
    let mut ops = reply["ops"].as_array().unwrap().clone();
    ops.sort_by_key(|op| op["path"].to_string());
    assert_eq!(ops, vec![json!({"type":"set","path":["done"],"value":"invalid"}), json!({"type":"set","path":["extra"],"value":"preserved"})]);
    assert_eq!(d.version(), before);
    assert_eq!(
        d.apply(r#"{"intents":[{"type":"set","path":["done"],"value":true}]}"#)
            .unwrap_err()
            .code.as_str(),
        "type_mismatch"
    );
    let reopened = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"], view["value"]);
    assert_eq!(snapshot(&reopened)["issues"], view["issues"]);
}

#[test]
fn independent_replicas_merge_and_duplicate_delivery_is_idempotent() {
    let f = fixture("checklist");
    let mut a = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let mut b = Document::open(&f["schema"].to_string(), &a.checkpoint().unwrap(), &[]).unwrap();
    let from = a.version();
    a.apply(r#"{"intents":[{"type":"set","path":["title"],"value":"abcX"}]}"#).unwrap();
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
    let f = fixture("checklist");
    let mut d = Document::create(&f["schema"].to_string(), r#"{"title":"abc","hits":0,"rows":[]}"#).unwrap();
    let applied = d.apply_batch(r#"{"intents":[{"type":"insert","path":["rows"],"value":{"text":"new","done":false}}]}"#, Origin::Page).unwrap();
    let id = applied.ids[0].as_str();
    assert_eq!(id.len(), 26);
    assert!(id
        .bytes()
        .all(|b| b"0123456789abcdefghjkmnpqrstvwxyz".contains(&b)));
    let reopened = Document::open(&f["schema"].to_string(), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"]["rows"][0]["$id"], id);
}

// Failure: the owner rebuilt after a late rejection stops publishing, loses the
// page's authored text, or exports bytes that no longer replay. Oracle: independent patch
// consumer, fresh snapshots and a literal final title. Gap: atomic_rejection only
// checks the state immediately after the rejection.
#[test]
fn owner_keeps_working_after_a_late_rejection() {
    let f = fixture("checklist");
    let schema = f["schema"].to_string();
    let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let seed = d.checkpoint().unwrap();
    let v0 = d.version();
    let mut projected = View::of(&d);
    let check = |d: &Document, projected: &mut View, reply: &str| {
        projected.publish(reply);
        projected.check(d, "publication");
    };
    let edit = |base: &str, from: &str, to: &str| {
        let caret = to.encode_utf16().count();
        json!({"base":base,"path":["title"],"from":from,"to":to,"selectionStart":caret,"selectionEnd":caret}).to_string()
    };
    let e = d.edit_text(&edit(&v0, "abc", "abcX")).unwrap();
    check(&d, &mut projected, e.publication.as_deref().unwrap());
    // Late rejection: the first intent mutated before the second failed.
    let late = r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000001"},"done"],"value":true},{"type":"remove","path":["rows"],"id":"missing"}]}"#;
    assert_eq!(d.apply(late).unwrap_err().code.as_str(), "path_not_found");
    assert_eq!(snapshot(&d)["value"], projected.value);
    // Local edits and remote imports still publish.
    let r = d.apply(r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000002"},"done"],"value":true}]}"#).unwrap();
    check(&d, &mut projected, &r);
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    let rows = peer.get_map("data").get("rows").unwrap().into_container().unwrap().into_movable_list().unwrap();
    rows.mov(0, 1).unwrap();
    peer.commit();
    let r = d.merge(&peer.export(ExportMode::updates(&from)).unwrap()).unwrap();
    check(&d, &mut projected, &r);
    // The page keeps typing from its authored version while the owner moved on.
    let e = d.edit_text(&edit(&e.authored, "abcX", "abcXY")).unwrap();
    check(&d, &mut projected, e.publication.as_deref().unwrap());
    let e = d.edit_text(&edit(&e.authored, "abcXY", "abcXYZ")).unwrap();
    check(&d, &mut projected, e.publication.as_deref().unwrap());
    assert_eq!(snapshot(&d)["value"]["title"], "abcXYZ");
    // Bytes exported by the rebuilt owner replay from before the rejection.
    let replayed = Document::open(&schema, &seed, &[d.export_since(&v0).unwrap()]).unwrap();
    assert_eq!(snapshot(&replayed)["value"], projected.value);
    let reopened = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"], projected.value);
}
