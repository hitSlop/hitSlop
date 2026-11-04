//! Stored state matches its descriptor: every write keeps it, every open and every import
//! checks it, refusing state that breaks it without changing the file. Merging valid edits
//! from several writers always yields state this accepts.
use super::*;

/// A scalar against its descriptor: a value of the wrong kind, or outside its bounds.
pub(super) fn scalar(node: &Node, value: &Value) -> Result<()> {
    let out_of_range = || Err(err(Code::OutOfRange, "Value is outside its bounds"));
    let bounded = |n: f64, min: Option<f64>, max: Option<f64>| {
        if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) { out_of_range() } else { Ok(()) }
    };
    match (node, value) {
        (Node::Boolean { .. }, Value::Bool(_)) => Ok(()),
        (Node::String { min_length, max_length, .. }, Value::String(s)) => {
            let length = s.chars().count() as u64;
            if min_length.is_some_and(|min| length < min) || max_length.is_some_and(|max| length > max) {
                out_of_range()
            } else {
                Ok(())
            }
        }
        (Node::Enum { values, .. }, Value::String(s)) if values.iter().any(|v| v == s) => Ok(()),
        (Node::Number { min, max, .. }, Value::Number(n)) => match n.as_f64() {
            Some(f) if f.is_finite() => bounded(f, *min, *max),
            _ => Err(mismatch(node)),
        },
        (Node::Integer { min, max, .. }, Value::Number(_)) => match integer(value) {
            Some(i) => bounded(i as f64, min.map(|m| m as f64), max.map(|m| m as f64)),
            None => Err(mismatch(node)),
        },
        _ => Err(mismatch(node)),
    }
}
fn mismatch(node: &Node) -> Error {
    err(
        Code::TypeMismatch,
        match node {
            Node::String { .. } => "must be a string",
            Node::Boolean { .. } => "must be a boolean",
            Node::Number { .. } => "must be a finite number",
            Node::Integer { .. } => "must be a safe integer",
            Node::Enum { .. } => "must be one of the declared values",
            _ => "Value does not match descriptor",
        },
    )
}

/// The stored `value` (absent when `None`) against `node`, in layout 2 (`layout`): text in a
/// text container, counters in finite counters, objects and records in maps, scalar lists
/// in movable lists of scalars, row lists as a map of rows by valid `$id` plus their order,
/// and scalars within their rules. It checks only what merging valid edits preserves: an
/// order that names a row twice, names a removed row or misses a row is accepted, because
/// two writers' valid edits produce it and reading resolves it (`RowList::ids`).
pub(super) fn stored(layout: Layout, node: &Node, value: Option<ValueOrContainer>) -> Result<()> {
    let invalid =
        || err(Code::InvalidBytes, "Saved state does not match the document's schema; keep the file for recovery");
    match (node, value) {
        (Node::Optional { .. }, None) => Ok(()),
        (Node::Optional { inner }, value) if layout.generations() && !is_scalar(inner) => {
            stored_slot(layout, inner, value)
        }
        (Node::Optional { inner }, value) => stored(layout, inner, value),
        (node, None) if layout.generations() && is_scalar(node) && node.default_value().is_some() => Ok(()),
        (Node::Text {}, Some(ValueOrContainer::Container(Container::Text(_)))) => Ok(()),
        (Node::Counter {}, Some(ValueOrContainer::Container(Container::Counter(counter)))) => {
            if counter.get_value().is_finite() { Ok(()) } else { Err(invalid()) }
        }
        (node, Some(ValueOrContainer::Value(value))) if is_scalar(node) => {
            scalar(node, &json(value)).map_err(|_| invalid())
        }
        (Node::Object { properties }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            if map.keys().any(|key| !properties.contains_key(&*key)) {
                return Err(invalid());
            }
            properties.iter().try_for_each(|(key, child)| stored(layout, child, map.get(key)))
        }
        (Node::Record { value: entry }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            let keys: Vec<String> = map.keys().map(|key| key.to_string()).collect();
            keys.iter().try_for_each(|key| {
                if !valid_key(key) {
                    return Err(invalid());
                }
                if layout::slotted(layout, entry, true) {
                    stored_slot(layout, entry, map.get(key))
                } else {
                    stored(layout, entry, map.get(key))
                }
            })
        }
        (Node::List { item }, Some(ValueOrContainer::Container(Container::MovableList(list)))) if is_scalar(item) => {
            let mut result = Ok(());
            list.for_each(|element| {
                if result.is_ok() {
                    result = match element {
                        ValueOrContainer::Value(value) => scalar(item, &json(value)).map_err(|_| invalid()),
                        ValueOrContainer::Container(_) => Err(invalid()),
                    };
                }
            });
            result
        }
        (Node::List { item }, Some(ValueOrContainer::Container(Container::Map(map)))) if !is_scalar(item) => {
            if map.keys().any(|key| !["rows", "order"].contains(&&*key)) {
                return Err(invalid());
            }
            let list = layout::RowList::in_map(layout, &map).ok_or_else(invalid)?;
            let ids: Vec<String> = list.rows().keys().map(|key| key.to_string()).collect();
            ids.iter().try_for_each(|id| {
                if !valid_id(id) {
                    return Err(invalid());
                }
                if layout.generations() {
                    stored_slot(layout, item, list.rows().get(id))
                } else {
                    stored(layout, item, list.rows().get(id))
                }
            })?;
            let mut result = Ok(());
            list.order().for_each(|entry| {
                if result.is_ok() && !matches!(entry, ValueOrContainer::Value(LoroValue::String(_))) {
                    result = Err(invalid());
                }
            });
            result
        }
        _ => Err(invalid()),
    }
}

/// Every retained generation is valid, including one a later undo can select again.
fn stored_slot(layout: Layout, node: &Node, value: Option<ValueOrContainer>) -> Result<()> {
    let invalid = || err(Code::InvalidBytes, "Saved state has an invalid generation slot");
    let Some(value) = value else { return Ok(()) };
    let ValueOrContainer::Container(Container::Map(slot)) = value else { return Err(invalid()) };
    layout::generation(&slot)?;
    for key in slot.keys() {
        if key.as_ref() == layout::CURRENT {
            continue;
        }
        if !layout::valid_generation(&key) {
            return Err(invalid());
        }
        stored(layout, node, slot.get(&key))?;
    }
    Ok(())
}
