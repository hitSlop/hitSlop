//! Descriptor-driven operations over three peers: atomic refusals, publication replay,
//! checkpoint reopen and seed-plus-log reopen are independent observations.
mod support;
use hitslop_core::Document;
use support::{Edit, View, next, snapshot};
use serde_json::{json, Value};

/// A valid value of `node`; `n` varies strings, choices and in-range numbers so
/// concurrent last-writer-wins conflicts can actually diverge.
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
/// The descriptor node a path names, and whether the path passes an optional value or a
/// record entry, whose lists are created (and published) with their parent.
fn node_at<'a>(descriptor: &'a Value, path: &[Value]) -> (&'a Value, bool) {
    path.iter().fold((descriptor, false), |(node, created), segment| {
        let (node, created) = if node["kind"] == "optional" { (&node["inner"], true) } else { (node, created) };
        match node["kind"].as_str().unwrap() {
            "object" => (&node["properties"][segment.as_str().unwrap()], created),
            "record" => (&node["value"], true),
            _ => (&node["item"], created),
        }
    })
}
/// Applies a publication to the page's view, which must equal a fresh snapshot. A clean
/// document's row lists publish row operations, never a replacement of the whole list.
fn published(doc: &Document, descriptor: &Value, view: &mut View, publication: &str) {
    view.publish(publication);
    view.check(doc, "publication");
    if snapshot(doc)["issues"] == json!([]) {
        let publication: Value = serde_json::from_str(publication).unwrap();
        for op in publication["ops"].as_array().unwrap() {
            let (node, created) = node_at(descriptor, op["path"].as_array().unwrap());
            let rows = node["kind"] == "list" && node["item"]["kind"] == "object";
            assert!(!(op["type"] == "set" && rows && !created), "a clean row list published a replacement: {op}");
        }
    }
}
fn run(name: &str, fixture: &str) {
    let f: Value = serde_json::from_str(fixture).unwrap(); let schema = f["schema"].to_string();
    let (mut accepted, mut refused) = (0usize, 0usize);
    for seed in 1..=support::workload("HITSLOP_MODEL_SEEDS", 8) {
        let mut rng = (seed as u64) * 0x9e3779b1; let mut serial = 0;
        let original = Document::create(&schema, &f["initial"].to_string()).unwrap();
        let checkpoint = original.checkpoint().unwrap(); let base = original.version();
        let mut peers: Vec<_> = (0..3).map(|_| Document::open(&schema, &checkpoint, &[]).unwrap()).collect();
        let mut views: Vec<_> = peers.iter().map(View::of).collect();
        for step in 0..support::workload("HITSLOP_MODEL_STEPS", 150) {
            let peer = (next(&mut rng) as usize) % 3; let before = snapshot(&peers[peer]);
            let ops: Vec<_> = (0..1 + (next(&mut rng) as usize) % 4).map(|_| intent(&mut rng, &mut serial, &f["schema"], &before["value"])).collect();
            match peers[peer].apply(&json!({"intents":ops}).to_string()) {
                Ok(reply) => { accepted += 1; published(&peers[peer], &f["schema"], &mut views[peer], &reply) }
                Err(_) => {
                    refused += 1;
                    assert_eq!(snapshot(&peers[peer]), before, "{name}, seed {seed}, step {step}: refused batch changed state");
                }
            }
            if step % 7 == 6 {
                let updates: Vec<_> = peers.iter().map(|p| p.export_since(&base).unwrap()).collect();
                for index in 0..3 { for update in &updates {
                    let reply = peers[index].merge(update).unwrap(); published(&peers[index], &f["schema"], &mut views[index], &reply);
                } }
                for index in 1..3 {
                    assert_eq!(views[0].value, views[index].value, "{name}, seed {seed}, step {step}: peers 0 and {index} diverged");
                }
            }
            for (index, peer) in peers.iter().enumerate() {
                let state = snapshot(peer); assert_eq!(views[index].value, state["value"]);
                // The owner's maintained state equals the full recomputation.
                assert_eq!(serde_json::from_str::<Value>(&peer.state().unwrap()).unwrap(), state, "{name}, seed {seed}, step {step}: state");
                // Reopening replays everything since the seed, so it runs at merges only.
                if step % 7 != 6 { continue; }
                for reopened in [Document::open(&schema, &peer.checkpoint().unwrap(), &[]).unwrap(), Document::open(&schema, &checkpoint, &[peer.export_since(&base).unwrap()]).unwrap()] {
                    let restored = snapshot(&reopened);
                    for key in ["value", "issues", "version"] { assert_eq!(restored[key], state[key], "{name}, seed {seed}, step {step}: {key}"); }
                }
            }
        }
    }
    // Refusals prove atomicity; most batches must still apply, or the model tests little.
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

/// One live owner receives CLI changes between a page's typing acknowledgements. This
/// exercises the actual local edit stream independently of replica exchange above.
#[test]
fn single_owner_delayed_typing_agent_edits_undo_and_reopen() {
    use hitslop_core::Origin;
    const SCHEMA: &str = r#"{"kind":"object","properties":{"text":{"kind":"text"},"hits":{"kind":"counter"},"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"}}}}}}"#;
    for seed in 1..=support::workload("HITSLOP_MODEL_SEEDS", 8) {
        let mut doc = Document::create(SCHEMA, r#"{"text":"start","hits":0,"rows":[]}"#).unwrap();
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
            let reopened = Document::open(SCHEMA, &doc.checkpoint().unwrap(), &[]).unwrap();
            for key in ["value", "issues", "theme", "version"] {
                assert_eq!(snapshot(&reopened)[key], snapshot(&doc)[key], "seed {seed}, step {step}: {key}");
            }
        }
    }
}
