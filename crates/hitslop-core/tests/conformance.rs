mod support;
use support::{app, Edit, View, apply_patches, fixture, snapshot, type_text, updates_since};
// Failure: the core accepts a wrong edit or publishes part of a rejected batch.
// Oracle: literal fixtures for identity and atomicity cases, with Unicode results
// spelled out independently.
use hitslop_core::Document;
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
            let mut d = Document::create(&app(f["schema"].to_string()),
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
                let delta = updates_since(&seed, &d);
                let reopened = Document::open(&app(f["schema"].to_string()), &seed, &[delta]).unwrap();
                assert_eq!(
                    snapshot(&reopened)["value"],
                    case["after"],
                    "{name}: incremental replay"
                );
                assert_eq!(reopened.version(), d.version());
                let reopened =
                    Document::open(&app(f["schema"].to_string()), &d.checkpoint().unwrap(), &[]).unwrap();
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
fn minted_ids_are_application_ids_and_survive_reopen() {
    let f = fixture("checklist");
    let mut d = Document::create(&app(f["schema"].to_string()), r#"{"title":"abc","hits":0,"rows":[]}"#).unwrap();
    let applied = d.apply_batch(r#"{"intents":[{"type":"insert","path":["rows"],"value":{"text":"new","done":false}}]}"#, Origin::Page).unwrap();
    let id = applied.ids[0].as_str();
    assert_eq!(id.len(), 26);
    assert!(id
        .bytes()
        .all(|b| b"0123456789abcdefghjkmnpqrstvwxyz".contains(&b)));
    let reopened = Document::open(&app(f["schema"].to_string()), &d.checkpoint().unwrap(), &[]).unwrap();
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
    let mut d = Document::create(&app(&schema), &f["initial"].to_string()).unwrap();
    let seed = d.checkpoint().unwrap();
    let v0 = d.version();
    let mut projected = View::of(&d);
    let check = |d: &Document, projected: &mut View, reply: &str| {
        projected.publish(reply);
        projected.check(d, "publication");
    };
    let edit = |d: &mut Document, base: &str, from: &str, to: &str| {
        type_text(d, base, json!(["title"]), from, to, to.encode_utf16().count()).unwrap()
    };
    let e = edit(&mut d, &v0, "abc", "abcX");
    check(&d, &mut projected, e.publication.as_deref().unwrap());
    // Late rejection: the first intent mutated before the second failed.
    let late = r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000001"},"done"],"value":true},{"type":"remove","path":["rows"],"id":"missing"}]}"#;
    assert_eq!(d.apply(late).unwrap_err().code.as_str(), "path_not_found");
    assert_eq!(snapshot(&d)["value"], projected.value);
    // Later edits still publish.
    let r = d.apply(r#"{"intents":[{"type":"set","path":["rows",{"id":"00000000000000000000000000000002"},"done"],"value":true}]}"#).unwrap();
    check(&d, &mut projected, &r);
    let r = d.apply(r#"{"intents":[{"type":"move","path":["rows"],"id":"00000000000000000000000000000001"}]}"#).unwrap();
    check(&d, &mut projected, &r);
    // The page keeps typing from its authored version while the owner moved on.
    let e = edit(&mut d, &e.text.unwrap().authored, "abcX", "abcXY");
    check(&d, &mut projected, e.publication.as_deref().unwrap());
    let e = edit(&mut d, &e.text.unwrap().authored, "abcXY", "abcXYZ");
    check(&d, &mut projected, e.publication.as_deref().unwrap());
    assert_eq!(snapshot(&d)["value"]["title"], "abcXYZ");
    // Updates saved by the rebuilt owner replay from before the rejection.
    let replayed = Document::open(&app(&schema), &seed, &[updates_since(&seed, &d)]).unwrap();
    assert_eq!(snapshot(&replayed)["value"], projected.value);
    let reopened = Document::open(&app(&schema), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"], projected.value);
}
