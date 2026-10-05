//! Descriptor-driven edits to one owner: atomic refusals, publication replay and reopen
//! are independent observations of every step.
mod support;
use hitslop_core::Document;
use support::{app, View, next, snapshot};
use serde_json::{json, Value};

/// A valid value of `node`; `n` varies strings, choices and in-range numbers so
/// successive edits differ.
fn value(node: &Value, n: usize) -> Value {
    match node["kind"].as_str().unwrap() {
        "text" => json!(format!("edit {n} 🪴")),
        "string" => {
            let limit = node["maxLength"].as_u64().unwrap_or(16) as usize;
            json!(format!("s{n}").chars().take(limit).collect::<String>())
        }
        "boolean" => json!(n % 2 == 0),
        "counter" => json!(0),
        "number" | "integer" => {
            let min = node["min"].as_f64().unwrap_or(-50.0);
            let max = node["max"].as_f64().unwrap_or(50.0);
            // The bounds themselves are valid; anything between too.
            let span = (max - min).max(0.0);
            let x = match n % 3 { 0 => min, 1 => max, _ => min + (n % 97) as f64 / 97.0 * span };
            if node["kind"] == "integer" { json!(x.floor().max(min) as i64) } else { json!(x) }
        }
        "enum" => node["values"][n % node["values"].as_array().unwrap().len()].clone(),
        "optional" => value(&node["inner"], n),
        "list" => json!([]),
        "record" => json!({}),
        "object" => Value::Object(node["properties"].as_object().unwrap().iter()
            .filter(|(_, child)| child["kind"] != "optional")
            .map(|(key, child)| (key.clone(), value(child, n))).collect()),
        kind => panic!("Unhandled descriptor kind {kind}"),
    }
}
/// Every addressable value. Plain objects are not targets themselves (they cannot be
/// replaced); their fields, rows and entries are.
fn targets<'a>(node: &'a Value, current: &'a Value, path: Vec<Value>, out: &mut Vec<(&'a Value, &'a Value, Vec<Value>)>) {
    if !path.is_empty() && node["kind"] != "object" { out.push((node, current, path.clone())); }
    let node = if node["kind"] == "optional" {
        if current.is_null() { return; }
        &node["inner"]
    } else { node };
    let mut walk = |child, current, key| {
        let mut path = path.clone(); path.push(key); targets(child, current, path, out);
    };
    match node["kind"].as_str().unwrap() {
        "object" => for (key, child) in node["properties"].as_object().unwrap() { walk(child, &current[key], json!(key)); },
        "record" => if let Some(entries) = current.as_object() { for (key, item) in entries { walk(&node["value"], item, json!(key)); } },
        "list" => if let Some(items) = current.as_array() { for (index, item) in items.iter().enumerate() {
            let segment = if node["item"]["kind"] == "object" { json!({"id":item["$id"]}) } else { json!({"index":index}) };
            walk(&node["item"], item, segment);
        } },
        _ => {}
    }
}
/// A value the core must refuse: `null`, or a number just outside its bounds.
fn invalid(node: &Value, n: usize) -> Value {
    let node = if node["kind"] == "optional" { &node["inner"] } else { node };
    match (node["kind"].as_str(), node["max"].as_f64(), node["min"].as_f64()) {
        (Some("number" | "integer"), Some(max), _) if n % 2 == 0 => json!(max + 1.0),
        (Some("number" | "integer"), _, Some(min)) => json!(min - 1.0),
        _ => Value::Null,
    }
}
fn anchor(rng: &mut u64, items: &[Value]) -> Option<Value> {
    let n = next(rng) as usize;
    let target = items.get(n % items.len().max(1))?;
    match n % 3 { 0 => Some(json!({"before":target["$id"]})), 1 => Some(json!({"after":target["$id"]})), _ => None }
}
fn intent(rng: &mut u64, serial: &mut usize, descriptor: &Value, current: &Value) -> Value {
    let mut choices = vec![]; targets(descriptor, current, vec![], &mut choices);
    let (node, current, path) = &choices[next(rng) as usize % choices.len()];
    let n = next(rng) as usize; *serial += 1;
    // Deliberate bad values exercise atomic rollback, not a mirrored validator.
    if n % 11 == 0 && node["kind"] != "list" && node["kind"] != "record" && node["kind"] != "counter" {
        return json!({"type":"set","path":path,"value":invalid(node, n)});
    }
    match node["kind"].as_str().unwrap() {
        "counter" => json!({"type":"increment","path":path,"by":if n % 2 == 0 { 1 } else { -1 }}),
        "optional" if n % 3 == 0 => json!({"type":"clear","path":path}),
        "record" => {
            let mut path = path.clone(); path.push(json!(format!("key{}", n % 4)));
            if n % 3 == 0 { json!({"type":"clear","path":path}) }
            else { json!({"type":"set","path":path,"value":value(&node["value"], n)}) }
        }
        "list" => {
            let items = current.as_array().unwrap();
            let rows = node["item"]["kind"] == "object";
            if rows {
                if items.is_empty() || n % 3 == 0 {
                    let mut op = json!({"type":"insert","path":path,"id":format!("generated{serial}"),"value":value(&node["item"], n)});
                    if let Some(at) = anchor(rng, items) { op["at"] = at; }
                    op
                } else if n % 3 == 1 {
                    let mut op = json!({"type":"move","path":path,"id":items[n % items.len()]["$id"]});
                    if let Some(at) = anchor(rng, items) { op["at"] = at; }
                    op
                } else {
                    json!({"type":"remove","path":path,"id":items[n % items.len()]["$id"]})
                }
            } else {
                match n % 4 {
                    _ if items.is_empty() => json!({"type":"insert","path":path,"value":value(&node["item"], n)}),
                    0 => json!({"type":"insert","path":path,"index":n % (items.len() + 1),"value":value(&node["item"], n)}),
                    1 => {
                        let index = n % items.len();
                        json!({"type":"remove","path":path,"index":index,"count":1 + (n / 7) % (items.len() - index)})
                    }
                    2 => json!({"type":"set","path":path,"value":(0..n % 4).map(|i| value(&node["item"], n + i)).collect::<Vec<_>>()}),
                    _ => {
                        let mut element = path.clone(); element.push(json!({"index":n % items.len()}));
                        json!({"type":"set","path":element,"value":value(&node["item"], n)})
                    }
                }
            }
        }
        _ => json!({"type":"set","path":path,"value":value(node, n)}),
    }
}
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
/// batches (including replacement), page text clients whose edits arrive late against
/// older versions, undo and redo. Every accepted step leaves a document whose stored state
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
        for step in 0..support::workload("HITSLOP_MODEL_STEPS", 150) {
            let before = snapshot(&doc);
            let n = next(&mut rng) as usize;
            let outcome: Result<Option<String>, hitslop_core::Error> = match n % 12 {
                0..=4 => {
                    let ops: Vec<_> = (0..1 + next(&mut rng) % 3).map(|_| intent(&mut rng, &mut serial, &f["schema"], &before["value"])).collect();
                    let origin = if n % 2 == 0 { Origin::Agent } else { Origin::Page };
                    doc.apply_batch(&json!({"intents":ops}).to_string(), origin).map(|a| a.publication)
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
                        let request = json!({"base":typist.base,"path":typist.path,"from":typist.text,"to":to,"selectionStart":caret,"selectionEnd":caret});
                        doc.edit_text(&request.to_string()).map(|edit| {
                            // A client keeps typing from the version its edit authored.
                            typists.push(Typist { path: typist.path, base: edit.authored, text: to });
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
            let typed = doc.edit_text(&json!({"base":base,"path":["text"],"from":from,"to":to,
                "selectionStart":caret,"selectionEnd":caret}).to_string()).unwrap();
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
