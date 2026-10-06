//! Generated edits from a descriptor: valid values, every addressable target, and batches
//! of the intents an owner receives (including deliberately invalid values, which must be
//! refused atomically). Shared by the model and the released-writer compatibility test.
use super::next;
use serde_json::{Value, json};

/// A valid value of `node`; `n` varies strings, choices and in-range numbers so
/// successive edits differ.
pub fn value(node: &Value, n: usize) -> Value {
    match node["kind"].as_str().unwrap() {
        "text" => json!(format!("edit {n} 🪴")),
        "string" => {
            let limit = node["maxLength"].as_u64().unwrap_or(16) as usize;
            json!(format!("s{n}").chars().take(limit).collect::<String>())
        }
        "boolean" => json!(n.is_multiple_of(2)),
        "counter" => json!(0),
        "number" | "integer" => {
            let min = node["min"].as_f64().unwrap_or(-50.0);
            let max = node["max"].as_f64().unwrap_or(50.0);
            // The bounds themselves are valid; anything between too.
            let span = (max - min).max(0.0);
            let x = match n % 3 {
                0 => min,
                1 => max,
                _ => min + (n % 97) as f64 / 97.0 * span,
            };
            if node["kind"] == "integer" { json!(x.floor().max(min) as i64) } else { json!(x) }
        }
        "enum" => node["values"][n % node["values"].as_array().unwrap().len()].clone(),
        "optional" => value(&node["inner"], n),
        "list" => json!([]),
        "record" => json!({}),
        "object" => Value::Object(
            node["properties"]
                .as_object()
                .unwrap()
                .iter()
                .filter(|(_, child)| child["kind"] != "optional")
                .map(|(key, child)| (key.clone(), value(child, n)))
                .collect(),
        ),
        kind => panic!("Unhandled descriptor kind {kind}"),
    }
}
/// Every addressable value. Plain objects are not targets themselves (they cannot be
/// replaced); their fields, rows and entries are.
pub fn targets<'a>(
    node: &'a Value,
    current: &'a Value,
    path: Vec<Value>,
    out: &mut Vec<(&'a Value, &'a Value, Vec<Value>)>,
) {
    if !path.is_empty() && node["kind"] != "object" {
        out.push((node, current, path.clone()));
    }
    let node = if node["kind"] == "optional" {
        if current.is_null() {
            return;
        }
        &node["inner"]
    } else {
        node
    };
    let mut walk = |child, current, key| {
        let mut path = path.clone();
        path.push(key);
        targets(child, current, path, out);
    };
    match node["kind"].as_str().unwrap() {
        "object" => {
            for (key, child) in node["properties"].as_object().unwrap() {
                walk(child, &current[key], json!(key));
            }
        }
        "record" => {
            if let Some(entries) = current.as_object() {
                for (key, item) in entries {
                    walk(&node["value"], item, json!(key));
                }
            }
        }
        "list" => {
            if let Some(items) = current.as_array() {
                for (index, item) in items.iter().enumerate() {
                    let segment = if node["item"]["kind"] == "object" {
                        json!({"id":item["$id"]})
                    } else {
                        json!({"index":index})
                    };
                    walk(&node["item"], item, segment);
                }
            }
        }
        _ => {}
    }
}
/// A value the core must refuse: `null`, or a number just outside its bounds.
pub fn invalid(node: &Value, n: usize) -> Value {
    let node = if node["kind"] == "optional" { &node["inner"] } else { node };
    match (node["kind"].as_str(), node["max"].as_f64(), node["min"].as_f64()) {
        (Some("number" | "integer"), Some(max), _) if n.is_multiple_of(2) => json!(max + 1.0),
        (Some("number" | "integer"), _, Some(min)) => json!(min - 1.0),
        _ => Value::Null,
    }
}
pub fn anchor(rng: &mut u64, items: &[Value]) -> Option<Value> {
    let n = next(rng) as usize;
    let target = items.get(n % items.len().max(1))?;
    match n % 3 {
        0 => Some(json!({"before":target["$id"]})),
        1 => Some(json!({"after":target["$id"]})),
        _ => None,
    }
}
pub fn intent(rng: &mut u64, serial: &mut usize, descriptor: &Value, current: &Value) -> Value {
    let mut choices = vec![];
    targets(descriptor, current, vec![], &mut choices);
    let (node, current, path) = &choices[next(rng) as usize % choices.len()];
    let n = next(rng) as usize;
    *serial += 1;
    // Deliberate bad values exercise atomic rollback, not a mirrored validator.
    if n.is_multiple_of(11) && node["kind"] != "list" && node["kind"] != "record" && node["kind"] != "counter" {
        return json!({"type":"set","path":path,"value":invalid(node, n)});
    }
    match node["kind"].as_str().unwrap() {
        "counter" => json!({"type":"increment","path":path,"by":if n.is_multiple_of(2) { 1 } else { -1 }}),
        "optional" if n.is_multiple_of(3) => json!({"type":"clear","path":path}),
        "record" => {
            let mut path = path.clone();
            path.push(json!(format!("key{}", n % 4)));
            if n.is_multiple_of(3) {
                json!({"type":"clear","path":path})
            } else {
                json!({"type":"set","path":path,"value":value(&node["value"], n)})
            }
        }
        "list" => {
            let items = current.as_array().unwrap();
            let rows = node["item"]["kind"] == "object";
            if rows {
                if items.is_empty() || n.is_multiple_of(3) {
                    let mut op = json!({"type":"insert","path":path,"id":format!("generated{serial}"),"value":value(&node["item"], n)});
                    if let Some(at) = anchor(rng, items) {
                        op["at"] = at;
                    }
                    op
                } else if n % 3 == 1 {
                    let mut op = json!({"type":"move","path":path,"id":items[n % items.len()]["$id"]});
                    if let Some(at) = anchor(rng, items) {
                        op["at"] = at;
                    }
                    op
                } else {
                    json!({"type":"remove","path":path,"id":items[n % items.len()]["$id"]})
                }
            } else {
                match n % 4 {
                    _ if items.is_empty() => json!({"type":"insert","path":path,"value":value(&node["item"], n)}),
                    0 => {
                        json!({"type":"insert","path":path,"index":n % (items.len() + 1),"value":value(&node["item"], n)})
                    }
                    1 => {
                        let index = n % items.len();
                        json!({"type":"remove","path":path,"index":index,"count":1 + (n / 7) % (items.len() - index)})
                    }
                    2 => {
                        json!({"type":"set","path":path,"value":(0..n % 4).map(|i| value(&node["item"], n + i)).collect::<Vec<_>>()})
                    }
                    _ => {
                        let mut element = path.clone();
                        element.push(json!({"index":n % items.len()}));
                        json!({"type":"set","path":element,"value":value(&node["item"], n)})
                    }
                }
            }
        }
        _ => json!({"type":"set","path":path,"value":value(node, n)}),
    }
}
