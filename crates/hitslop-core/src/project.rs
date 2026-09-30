use super::*;

/// The exact sum of a counter's stored contributions, or `None` when any
/// contribution is not a safe integer or the sum leaves the safe range.
pub(super) fn counter_sum(raw: &Value) -> Option<i64> {
    let sum = raw.as_object()?.values().try_fold(0i128, |sum, v| {
        let n = v.as_i64().filter(|n| safe(*n))?;
        sum.checked_add(i128::from(n))
    })?;
    i64::try_from(sum).ok().filter(|n| safe(*n))
}
/// The application view of a raw stored value: counters become their sum.
/// Anomalous counters read as null; their stored contributions remain untouched.
pub(super) fn project(node: Option<&Node>, value: Value) -> Value {
    match (node, value) {
        (Some(Node::Optional { inner }), value) => project(Some(inner), value),
        // JavaScript has one number type: an integral f64 reads as an integer, so a
        // snapshot compares equal in Rust, Swift and JavaScript.
        (Some(Node::Number { .. }), Value::Number(n)) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() <= MAX_SAFE as f64 => json!(f as i64),
            _ => Value::Number(n),
        },
        (Some(Node::Counter {}), raw) => match counter_sum(&raw) {
            Some(sum) => json!(sum),
            None => Value::Null,
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
/// Container-aware rendering supplies effective row IDs while preserving raw storage.
pub(super) fn project_container(node: Option<&Node>, value: ValueOrContainer) -> Result<Value> {
    match (node, &value) {
        (Some(Node::Optional { inner }), ValueOrContainer::Container(_)) => project_container(Some(inner), value),
        // One pass over the entries: each is read once, declared ones projected by kind.
        (Some(Node::Object { properties }), ValueOrContainer::Container(Container::Map(map))) => {
            let mut result = serde_json::Map::new();
            let mut failure = None;
            map.for_each(|key, value| {
                if failure.is_some() { return; }
                match project_container(properties.get(key), value) {
                    Ok(value) => { result.insert(key.to_owned(), value); }
                    Err(e) => failure = Some(e),
                }
            });
            failure.map_or(Ok(Value::Object(result)), Err)
        }
        (Some(Node::Record { value: entry }), ValueOrContainer::Container(Container::Map(map))) => {
            let mut result = serde_json::Map::new();
            for key in map.keys() {
                let key = key.to_string();
                if let Some(v) = map.get(&key) {
                    result.insert(key, project_container(Some(entry), v)?);
                }
            }
            Ok(Value::Object(result))
        }
        (Some(Node::List { item }), ValueOrContainer::Container(Container::MovableList(list))) if !is_scalar(item) => {
            let ids = identity::rows(list);
            let mut result = Vec::with_capacity(list.len());
            for (index, id) in ids.into_iter().enumerate() {
                let mut row = project_container(Some(item), list.get(index).unwrap())?;
                if let (Some(id), Some(map)) = (id, row.as_object_mut()) { map.insert("$id".into(), json!(id)); }
                result.push(row);
            }
            Ok(Value::Array(result))
        }
        _ => Ok(project(node, json(value.get_deep_value())?)),
    }
}
pub(super) fn project_at(doc: &LoroDoc, node: Option<&Node>, cid: &ContainerID) -> Result<Value> {
    let container = doc.get_container(cid.clone()).ok_or_else(|| err(Code::PathNotFound, "Container is absent"))?;
    project_container(node, ValueOrContainer::Container(container))
}
