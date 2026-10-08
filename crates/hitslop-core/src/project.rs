use super::*;

/// The application view of a raw stored value. A number that is a whole safe integer
/// reads as an integer, so a snapshot compares equal in Rust, Swift and JavaScript.
pub(super) fn project(node: Option<&Node>, value: Value) -> Value {
    match (node, value) {
        (Some(Node::Optional { inner }), value) => project(Some(inner), value),
        (Some(Node::Number { .. }), Value::Number(n)) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() <= MAX_SAFE as f64 => json!(f as i64),
            _ => Value::Number(n),
        },
        (Some(Node::Object { properties }), Value::Object(mut map)) => {
            for (key, child) in properties {
                if let Some(v) = map.remove(key) {
                    map.insert(key.clone(), project(Some(child), v));
                }
            }
            Value::Object(map)
        }
        (Some(Node::List { item }), Value::Array(rows)) => {
            Value::Array(rows.into_iter().map(|r| project(Some(item), r)).collect())
        }
        (Some(Node::Record { value: entry }), Value::Object(map)) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, project(Some(entry), v))).collect())
        }
        (_, value) => value,
    }
}
