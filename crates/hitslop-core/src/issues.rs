use super::*;

/// A scalar's stored value against its descriptor: `None`, or the issue code.
pub(super) fn scalar_issue(node: &Node, value: &Value) -> Option<IssueCode> {
    let bounded = |n: f64, min: Option<f64>, max: Option<f64>| {
        if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) { Some(IssueCode::OutOfRange) } else { None }
    };
    match (node, value) {
        (Node::Boolean {}, Value::Bool(_)) => None,
        (Node::String { max_length }, Value::String(s)) => {
            max_length.filter(|max| utf16_len(s) > *max).map(|_| IssueCode::OutOfRange)
        }
        (Node::Enum { values }, Value::String(s)) => {
            (!values.iter().any(|v| v == s)).then_some(IssueCode::TypeMismatch)
        }
        (Node::Number { min, max }, Value::Number(n)) => match n.as_f64() {
            Some(f) if f.is_finite() => bounded(f, *min, *max),
            _ => Some(IssueCode::TypeMismatch),
        },
        (Node::Integer { min, max }, Value::Number(n)) => match n.as_i64().filter(|n| safe(*n)) {
            Some(i) => bounded(i as f64, min.map(|m| m as f64), max.map(|m| m as f64)),
            None => Some(IssueCode::TypeMismatch),
        },
        _ => Some(IssueCode::TypeMismatch),
    }
}
/// How an issue path names row `index` of a row list: by the `$id` the application sees
/// for it (`row`, as projected), or by position when it has none.
fn row_segment(row: Option<&Value>, index: usize) -> Segment {
    match row.and_then(|row| row.get("$id")).and_then(Value::as_str).filter(|id| valid_id(id)) {
        Some(id) => Segment::Id { id: id.to_owned() },
        None => Segment::Index { index },
    }
}
fn push(result: &mut Vec<Issue>, code: IssueCode, path: &[Segment]) {
    result.push(Issue { code, path: path.to_vec() });
}
/// The JSON oracle. `names` is the projected value, which names rows by their effective
/// `$id`; without it rows are named by position.
pub(super) fn issues(node: &Node, value: &Value, names: Option<&Value>, path: &mut Vec<Segment>, result: &mut Vec<Issue>) {
    match (node, value) {
        (Node::Text {}, Value::String(_)) => {}
        (Node::Optional { inner }, value) => issues(inner, value, names, path, result),
        (node, value) if is_scalar(node) => {
            if let Some(code) = scalar_issue(node, value) {
                push(result, code, path);
            }
        }
        (Node::Counter {}, raw @ Value::Object(_)) => {
            if counter_sum(raw).is_none() {
                push(result, IssueCode::TypeMismatch, path);
            }
        }
        (Node::Object { properties }, Value::Object(map)) => {
            for (key, child) in properties {
                let value = map.get(key);
                // An absent optional is valid; a stored null is not.
                if value.is_none() && matches!(child, Node::Optional { .. }) {
                    continue;
                }
                path.push(Segment::Key(key.clone()));
                issues(child, value.unwrap_or(&Value::Null), names.and_then(|n| n.get(key)), path, result);
                path.pop();
            }
            for key in map.keys().filter(|key| key.as_str() != "$id" && !properties.contains_key(*key)) {
                path.push(Segment::Key(key.clone()));
                push(result, IssueCode::UnknownField, path);
                path.pop();
            }
        }
        (Node::List { item }, Value::Array(elements)) if is_scalar(item) => {
            for (index, element) in elements.iter().enumerate() {
                if let Some(code) = scalar_issue(item, element) {
                    path.push(Segment::Index { index });
                    push(result, code, path);
                    path.pop();
                }
            }
        }
        (Node::Record { value: entry }, Value::Object(map)) => {
            for (key, value) in map {
                path.push(Segment::Key(key.clone()));
                if valid_key(key) {
                    issues(entry, value, names.and_then(|n| n.get(key)), path, result);
                } else {
                    push(result, IssueCode::InvalidKey, path);
                }
                path.pop();
            }
        }
        (Node::List { item }, Value::Array(rows)) => {
            let mut seen = BTreeSet::new();
            for (index, row) in rows.iter().enumerate() {
                let name = names.and_then(|n| n.get(index));
                path.push(row_segment(name, index));
                match row.get("$id").and_then(Value::as_str) {
                    Some(id) if valid_id(id) => {
                        if !seen.insert(id) {
                            push(result, IssueCode::DuplicateId, path);
                        }
                    }
                    _ => push(result, IssueCode::InvalidId, path),
                }
                issues(item, row, name, path, result);
                path.pop();
            }
        }
        _ => push(result, IssueCode::TypeMismatch, path),
    }
}
/// Same result as `issues` over the projected value, read from the containers.
pub(super) fn container_issues(
    node: &Node,
    value: Option<ValueOrContainer>,
    path: &mut Vec<Segment>,
    result: &mut Vec<Issue>,
) {
    match (node, value) {
        (Node::Text {}, Some(ValueOrContainer::Container(Container::Text(_)))) => {}
        (Node::Optional { .. }, None) => {}
        (Node::Optional { inner }, value @ Some(_)) => container_issues(inner, value, path, result),
        (Node::Object { properties }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            for (key, child) in properties {
                path.push(Segment::Key(key.clone()));
                container_issues(child, map.get(key), path, result);
                path.pop();
            }
            let mut keys: Vec<_> = map.keys().map(|key| key.to_string()).collect();
            keys.sort();
            for key in keys.into_iter().filter(|key| key != "$id" && !properties.contains_key(key)) {
                path.push(Segment::Key(key));
                push(result, IssueCode::UnknownField, path);
                path.pop();
            }
        }
        // A scalar list is a movable list of plain values; its elements are checked as JSON.
        (Node::List { item }, Some(ValueOrContainer::Container(Container::MovableList(list)))) if is_scalar(item) => {
            let value = json(list.get_deep_value());
            issues(node, &value, None, path, result);
        }
        (Node::Record { value: entry }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            let mut keys: Vec<_> = map.keys().map(|key| key.to_string()).collect();
            keys.sort();
            for key in keys {
                let value = map.get(&key);
                let valid = valid_key(&key);
                path.push(Segment::Key(key));
                if valid {
                    container_issues(entry, value, path, result);
                } else {
                    push(result, IssueCode::InvalidKey, path);
                }
                path.pop();
            }
        }
        (Node::List { item }, Some(ValueOrContainer::Container(Container::MovableList(list)))) => {
            let effective = identity::rows(&list);
            let mut seen = HashSet::new();
            for (index, effective) in effective.into_iter().enumerate() {
                let row = list.get(index);
                let (stored, segment) = match &row {
                    Some(ValueOrContainer::Container(Container::Map(map))) => {
                        let stored = match map.get("$id") {
                            Some(ValueOrContainer::Value(loro::LoroValue::String(id))) => Some(id.to_string()),
                            _ => None,
                        };
                        (stored, Segment::Id { id: effective.expect("a map row has an effective ID") })
                    }
                    // A plain value (or other container) row: as the JSON oracle sees it.
                    Some(other) => {
                        let value = json(other.get_deep_value());
                        let stored = value.get("$id").and_then(Value::as_str).map(str::to_owned);
                        (stored, row_segment(Some(&value), index))
                    }
                    None => (None, Segment::Index { index }),
                };
                path.push(segment);
                match stored {
                    Some(id) if valid_id(&id) => {
                        if !seen.insert(id) {
                            push(result, IssueCode::DuplicateId, path);
                        }
                    }
                    _ => push(result, IssueCode::InvalidId, path),
                }
                container_issues(item, row, path, result);
                path.pop();
            }
        }
        // Plain values and unusual container kinds: exact JSON semantics, small or rare.
        (node, value) => {
            let value = match value {
                Some(v) => json(v.get_deep_value()),
                None => Value::Null,
            };
            issues(node, &value, Some(&value), path, result);
        }
    }
}
