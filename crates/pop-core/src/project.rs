use super::*;

/// The application view of a raw stored value (Loro's deep value). A number that is a
/// whole safe integer reads as an integer, so a snapshot compares equal in Rust, Swift and
/// JavaScript. A counter reads as an integer, and a row list as its rows in order, each with
/// its `$id`.
pub(super) fn project(node: Option<&Node>, value: Value) -> Value {
    match (node, value) {
        (Some(Node::Optional { inner, .. }), value) => {
            if is_scalar(inner) {
                project(Some(inner), value)
            } else {
                slot(inner, value).unwrap_or(Value::Null)
            }
        }
        (Some(Node::Number { .. }), Value::Number(n)) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() <= MAX_SAFE as f64 => json!(f as i64),
            _ => Value::Number(n),
        },
        (Some(Node::Counter { .. }), Value::Number(n)) => json!(layout::counter_total(n.as_f64().unwrap_or(0.0))),
        (Some(Node::Object { properties, .. }), Value::Object(mut map)) => {
            for (key, child) in properties {
                if let Some(v) = map.remove(key) {
                    let projected = project(Some(child), v);
                    if !projected.is_null() {
                        map.insert(key.clone(), projected);
                    }
                } else if let Some(default) = child.default_value() {
                    map.insert(key.clone(), child.with_defaults(&default));
                }
            }
            Value::Object(map)
        }
        (Some(Node::List { item, .. }), Value::Array(elements)) => {
            Value::Array(elements.into_iter().map(|r| project(Some(item), r)).collect())
        }
        (Some(Node::List { item, .. }), Value::Object(mut list)) => {
            let Some(Value::Object(mut rows)) = list.remove("rows") else {
                return Value::Array(vec![]);
            };
            rows.retain(|_, value| {
                value.as_object().is_some_and(|slot| {
                    let current = layout::current_generation(slot.get(layout::CURRENT).and_then(Value::as_str));
                    slot.contains_key(current)
                })
            });
            let order = match list.remove("order") {
                Some(Value::Array(order)) => order,
                _ => vec![],
            };
            let ids = layout::visible(
                order.iter().filter_map(Value::as_str).map(str::to_owned),
                |id| rows.contains_key(id),
                rows.keys().cloned().collect::<Vec<_>>(),
            );
            Value::Array(
                ids.into_iter()
                    .filter_map(|id| {
                        let mut row = slot(item, rows.remove(&id)?)?;
                        if let Value::Object(fields) = &mut row {
                            fields.insert("$id".into(), Value::String(id));
                        }
                        Some(row)
                    })
                    .collect(),
            )
        }
        (Some(Node::Record { value: entry, .. }), Value::Object(map)) => Value::Object(
            map.into_iter()
                .filter_map(|(k, v)| {
                    let value = if layout::slotted(entry, true) { slot(entry, v)? } else { project(Some(entry), v) };
                    Some((k, value))
                })
                .collect(),
        ),
        (_, value) => value,
    }
}

/// Unwraps the visible generation in a raw slot. Hidden generations never project.
fn slot_value(value: Value) -> Option<Value> {
    let Value::Object(mut slot) = value else { return None };
    let current = layout::current_generation(slot.get(layout::CURRENT).and_then(Value::as_str)).to_owned();
    slot.remove(&current)
}
pub(super) fn slot(node: &Node, value: Value) -> Option<Value> {
    slot_value(value).map(|value| project(Some(unwrap_optional(node)), value))
}
