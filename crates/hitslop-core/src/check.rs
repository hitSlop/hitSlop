//! The single-writer invariant: a document's stored state always matches its descriptor.
//! Every write keeps it, and every open checks it, refusing a file that breaks it without
//! changing the file.
use super::*;

/// A scalar against its descriptor: a value of the wrong kind, or outside its bounds.
pub(super) fn scalar(node: &Node, value: &Value) -> Result<()> {
    let out_of_range = || Err(err(Code::OutOfRange, "Value is outside its bounds"));
    let bounded = |n: f64, min: Option<f64>, max: Option<f64>| {
        if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) { out_of_range() } else { Ok(()) }
    };
    match (node, value) {
        (Node::Boolean {}, Value::Bool(_)) => Ok(()),
        (Node::String { max_length }, Value::String(s)) => {
            if max_length.is_some_and(|max| utf16_len(s) > max) { out_of_range() } else { Ok(()) }
        }
        (Node::Enum { values }, Value::String(s)) if values.iter().any(|v| v == s) => Ok(()),
        (Node::Number { min, max }, Value::Number(n)) => match n.as_f64() {
            Some(f) if f.is_finite() => bounded(f, *min, *max),
            _ => Err(mismatch()),
        },
        (Node::Integer { min, max }, Value::Number(n)) => match n.as_i64().filter(|n| safe(*n)) {
            Some(i) => bounded(i as f64, min.map(|m| m as f64), max.map(|m| m as f64)),
            None => Err(mismatch()),
        },
        _ => Err(mismatch()),
    }
}
fn mismatch() -> Error {
    err(Code::TypeMismatch, "Value does not match descriptor")
}

/// The stored `value` (absent when `None`) against `node`, as containers: text in a text
/// container, objects and records in maps, lists in movable lists whose rows are maps with
/// unique valid `$id`s, counters as safe integers, scalars within their rules.
pub(super) fn stored(node: &Node, value: Option<ValueOrContainer>) -> Result<()> {
    let invalid = || err(Code::InvalidBytes, "Saved state does not match the document's schema; keep the file for recovery");
    match (node, value) {
        (Node::Optional { .. }, None) => Ok(()),
        (Node::Optional { inner }, value) => stored(inner, value),
        (Node::Text {}, Some(ValueOrContainer::Container(Container::Text(_)))) => Ok(()),
        (Node::Counter {}, Some(ValueOrContainer::Value(loro::LoroValue::I64(n)))) if safe(n) => Ok(()),
        (node, Some(ValueOrContainer::Value(value))) if is_scalar(node) => scalar(node, &json(value)).map_err(|_| invalid()),
        (Node::Object { properties }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            if map.keys().any(|key| &*key != "$id" && !properties.contains_key(&*key)) {
                return Err(invalid());
            }
            properties.iter().try_for_each(|(key, child)| stored(child, map.get(key)))
        }
        (Node::Record { value: entry }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            let keys: Vec<String> = map.keys().map(|key| key.to_string()).collect();
            keys.iter().try_for_each(|key| if valid_key(key) { stored(entry, map.get(key)) } else { Err(invalid()) })
        }
        (Node::List { item }, Some(ValueOrContainer::Container(Container::MovableList(list)))) if is_scalar(item) => {
            let mut result = Ok(());
            list.for_each(|element| if result.is_ok() {
                result = match element {
                    ValueOrContainer::Value(value) => scalar(item, &json(value)).map_err(|_| invalid()),
                    ValueOrContainer::Container(_) => Err(invalid()),
                };
            });
            result
        }
        (Node::List { item }, Some(ValueOrContainer::Container(Container::MovableList(list)))) => {
            let mut seen = HashSet::new();
            let mut rows = vec![];
            list.for_each(|row| rows.push(row));
            rows.into_iter().try_for_each(|row| match &row {
                ValueOrContainer::Container(Container::Map(map)) => match identity::stored_id(map) {
                    Some(id) if seen.insert(id.clone()) => stored(item, Some(row)),
                    _ => Err(invalid()),
                },
                _ => Err(invalid()),
            })
        }
        _ => Err(invalid()),
    }
}
