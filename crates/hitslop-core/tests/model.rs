//! Descriptor-driven edits to one owner: atomic refusals, publication replay and reopen
//! are independent observations of every step.
mod support;
use hitslop_core::Document;
use support::{app, View, next, snapshot, type_text};
use support::generate::{intent, targets, value};
use serde_json::{json, Value};

/// One page text client: the field it types in, and the version and text it last saw.
struct Typist { path: Vec<Value>, base: String, text: String }
/// The text fields that hold text now: plain text, and set optional text.
fn text_fields(descriptor: &Value, current: &Value) -> Vec<(Vec<Value>, String)> {
    let mut all = vec![]; targets(descriptor, current, vec![], &mut all);
    all.into_iter().filter(|(node, value, _)| {
        node["kind"] == "text" || (node["kind"] == "optional" && node["inner"]["kind"] == "text" && value.is_string())
    }).map(|(_, value, path)| (path, value.as_str().unwrap().to_owned())).collect()
}
/// The single-writer invariant, over the edits one owner actually receives: the agent's
/// batches (including replacement, and text sets from the version it last read), page
/// text clients whose edits arrive late against older versions, undo and redo. Every accepted step leaves a document whose stored state
/// matches its descriptor (reopening checks it), whose publications replay to a fresh
/// snapshot and which reopens the same; every refusal changes nothing.
fn run(name: &str, fixture: &str) {
    use hitslop_core::Origin;
    let f: Value = serde_json::from_str(fixture).unwrap(); let schema = f["schema"].to_string();
    let (mut accepted, mut refused) = (0usize, 0usize);
    for seed in 1..=support::workload("HITSLOP_MODEL_SEEDS", 8) {
        let mut rng = (seed as u64) * 0x9e3779b1 + 7; let mut serial = 0;
        let mut doc = Document::create(&app(&schema), &f["initial"].to_string()).unwrap();
        let mut view = View::of(&doc);
        let mut typists: Vec<Typist> = vec![];
        // The version the agent last read; its batches' text sets merge from it.
        let mut read: Option<String> = None;
        for step in 0..support::workload("HITSLOP_MODEL_STEPS", 150) {
            let before = snapshot(&doc);
            let n = next(&mut rng) as usize;
            let outcome: Result<Option<String>, hitslop_core::Error> = match n % 12 {
                0..=4 => {
                    let ops: Vec<_> = (0..1 + next(&mut rng) % 3).map(|_| intent(&mut rng, &mut serial, &f["schema"], &before["value"])).collect();
                    let origin = if n % 2 == 0 { Origin::Agent } else { Origin::Page };
                    let mut batch = json!({"intents":ops});
                    if origin == Origin::Agent {
                        if n % 3 == 0 || read.is_none() { read = Some(before["version"].as_str().unwrap().to_owned()); }
                        batch["base"] = json!(read);
                    }
                    doc.apply_batch(&batch.to_string(), origin).map(|a| a.publication)
                }
                5 => {
                    let mut all = vec![]; targets(&f["schema"], &before["value"], vec![], &mut all);
                    let op = match all.get(n % (all.len() + 1)) {
                        Some((node, _, path)) => json!({"type":"replace","path":path,"value":value(node, n)}),
                        None => json!({"type":"replace","path":[],"value":value(&f["schema"], n)}),
                    };
                    doc.apply_batch(&json!({"intents":[op]}).to_string(), Origin::Agent).map(|a| a.publication)
                }
                6 | 7 => {
                    let fields = text_fields(&f["schema"], &before["value"]);
                    if typists.len() < 3 && !fields.is_empty() && n % 2 == 0 {
                        let (path, text) = fields[n % fields.len()].clone();
                        typists.push(Typist { path, base: before["version"].as_str().unwrap().to_owned(), text });
                        Ok(None)
                    } else if typists.is_empty() {
                        Ok(None)
                    } else {
                        let typist = typists.remove(n % typists.len());
                        let to = format!("{}·{serial}", typist.text); serial += 1;
                        let caret = to.encode_utf16().count();
                        type_text(&mut doc, &typist.base, json!(typist.path), &typist.text, &to, caret).map(|edit| {
                            // A client keeps typing from the version its edit authored.
                            typists.push(Typist { path: typist.path, base: edit.text.unwrap().authored, text: to });
                            edit.publication
                        })
                    }
                }
                8..=9 => doc.undo().map(|a| a.publication),
                _ => doc.redo().map(|a| a.publication),
            };
            match outcome {
                Ok(publication) => {
                    accepted += 1;
                    if let Some(publication) = publication { view.publish(&publication); }
                    view.check(&doc, "single owner");
                }
                Err(_) => {
                    refused += 1;
                    assert_eq!(snapshot(&doc), before, "{name}, seed {seed}, step {step}: a refusal changed state");
                }
            }
            let state = snapshot(&doc);
            assert_eq!(serde_json::from_str::<Value>(&doc.state().unwrap()).unwrap(), state, "{name}, seed {seed}, step {step}: maintained state");
            if step % 3 == 2 {
                let reopened = Document::open(&app(&schema), &doc.checkpoint().unwrap(), &[])
                    .unwrap_or_else(|e| panic!("{name}, seed {seed}, step {step}: a local edit stored invalid state: {e}"));
                let restored = snapshot(&reopened);
                for key in ["value", "version"] { assert_eq!(restored[key], state[key], "{name}, seed {seed}, step {step}: reopen {key}"); }
            }
        }
    }
    assert!(accepted >= refused, "{name}: {accepted} accepted, {refused} refused");
}
#[test]
fn checklist_model() { run("checklist", include_str!("../fixtures/checklist.json")) }
#[test]
fn scalars_model() { run("scalars", include_str!("../fixtures/scalars.json")) }
#[test]
fn collections_model() { run("collections", include_str!("../fixtures/collections.json")) }
#[test]
fn nested_model() { run("nested", include_str!("../fixtures/nested.json")) }

/// One live owner receives CLI changes between a page's typing acknowledgements: a fixed
/// interleaving beside the random models above.
#[test]
fn single_owner_delayed_typing_agent_edits_undo_and_reopen() {
    use hitslop_core::Origin;
    const SCHEMA: &str = r#"{"kind":"object","properties":{"text":{"kind":"text"},"hits":{"kind":"counter"},"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"}}}}}}"#;
    for seed in 1..=support::workload("HITSLOP_MODEL_SEEDS", 8) {
        let mut doc = Document::create(&app(SCHEMA), r#"{"text":"start","hits":0,"rows":[]}"#).unwrap();
        let mut view = View::of(&doc);
        for step in 0..24 {
            let before = snapshot(&doc);
            let from = before["value"]["text"].as_str().unwrap();
            let to = format!("{from}p{seed}-{step}");
            let base = doc.version();
            // This CLI prefix arrives while the page is typing against its older base.
            let prefixed = format!("A{from}");
            let agent = doc.apply_batch(&json!({"intents":[
                {"type":"set","path":["text"],"value":prefixed},
                {"type":"increment","path":["hits"],"by":1},
            ]}).to_string(), Origin::Agent).unwrap();
            view.publish(&agent.publication.unwrap());
            view.check(&doc, "owner CLI batch");
            let caret = to.encode_utf16().count();
            let typed = type_text(&mut doc, &base, json!(["text"]), from, &to, caret).unwrap();
            view.publish(&typed.publication.unwrap());
            view.check(&doc, "delayed page edit");
            assert_eq!(snapshot(&doc)["value"]["text"], format!("A{to}"));
            assert_eq!(snapshot(&doc)["value"]["hits"], step + 1);
            let accepted = snapshot(&doc);
            // An earlier valid mutation in a refused batch must roll back too.
            assert!(doc.apply_batch(r#"{"intents":[{"type":"set","path":["text"],"value":"partial"},{"type":"increment","path":["hits"],"by":0}]}"#, Origin::Agent).is_err());
            assert_eq!(snapshot(&doc), accepted);
            view.publish(&doc.undo().unwrap().publication.unwrap());
            view.check(&doc, "undo delayed typing");
            assert_eq!(snapshot(&doc)["value"]["text"], prefixed);
            view.publish(&doc.redo().unwrap().publication.unwrap());
            view.check(&doc, "redo delayed typing");
            assert_eq!(snapshot(&doc)["value"], accepted["value"]);
            let reopened = Document::open(&app(SCHEMA), &doc.checkpoint().unwrap(), &[]).unwrap();
            for key in ["value", "theme", "version"] {
                assert_eq!(snapshot(&reopened)[key], snapshot(&doc)[key], "seed {seed}, step {step}: {key}");
            }
        }
    }
}
