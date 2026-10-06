//! `replace`: the value at a path (the whole document when empty) becomes the given JSON,
//! written as its differences. Rows are matched by `$id`; kept rows, text and every other
//! container keep their identity, so open text fields, row handles and concurrent edits
//! survive. An unchanged value writes nothing.
use super::*;
use execute::{insert_row, release, remove, rewrite_list, Change};

/// The one rule for writes over merged anomalies (`set` and `replace`): a stored anomaly at
/// or under the target is preserved and flagged, never repaired, so it refuses the write.
/// An out-of-range value has the right type and may be overwritten. A write inside an
/// anomalous value leaves the anomaly as stored, and one through a wrong-typed value does
/// not resolve.
pub(super) fn refuse_anomalies(issues: &[Issue], path: &[Segment]) -> Result<()> {
    if issues.iter().any(|issue| issue.path.starts_with(path) && issue.code != IssueCode::OutOfRange) {
        return Err(err(Code::TypeMismatch, "Cannot overwrite a value that holds a stored anomaly"));
    }
    Ok(())
}

/// Validates `value` completely before the first mutation.
pub(super) fn replace(
    doc: &LoroDoc,
    schema: &Node,
    path: &[Segment],
    value: &Value,
    issues: &[Issue],
    ids: &mut Vec<String>,
    rows: &mut Rows,
) -> Result<()> {
    if path.len() > crate::wire::PATH_SEGMENTS {
        return Err(err(Code::InvalidPath, "Path length"));
    }
    refuse_anomalies(issues, path)?;
    let writer = writer(doc);
    let mut to = Reconcile { doc, writer: &writer, ids, rows };
    if path.is_empty() {
        schema.validate(value, false)?;
        return to.object(&doc.get_map("data"), schema, value, false);
    }
    let at = resolve(doc, schema, path, to.rows)?;
    let kind = unwrap_optional(at.node);
    if let Some((list, index)) = &at.element {
        kind.validate(value, false)?;
        if stored(kind, &at.value) != Some(project(Some(kind), value.clone())) {
            list.set(*index, loro_scalar(kind, value)).map_err(engine)?;
        }
        return Ok(());
    }
    match &at.parent {
        Some((map, key)) => {
            at.node.validate(value, false)?;
            to.field(map, key, at.node, value, at.shared)
        }
        // A row: its ID stays.
        None => {
            kind.validate(value, true)?;
            let Some(Segment::Id { id }) = path.last() else {
                return Err(err(Code::TypeMismatch, "Cannot replace this value"));
            };
            if value.get("$id").is_some_and(|stored| stored.as_str() != Some(id)) {
                return Err(err(Code::InvalidId, "A row keeps its ID"));
            }
            let ValueOrContainer::Container(Container::Map(row)) = &at.value else {
                return Err(err(Code::TypeMismatch, "Cannot replace an anomalous row"));
            };
            to.object(row, kind, value, at.shared)
        }
    }
}

/// The present object `map` becomes `value`, written as its differences (`set` of an
/// object that holds only scalars).
pub(super) fn object(
    doc: &LoroDoc,
    map: &LoroMap,
    node: &Node,
    value: &Value,
    shared: bool,
    ids: &mut Vec<String>,
    rows: &mut Rows,
) -> Result<()> {
    let writer = writer(doc);
    Reconcile { doc, writer: &writer, ids, rows }.object(map, node, value, shared)
}

/// A stored scalar as the snapshot shows it, or none when it is not one.
fn stored(kind: &Node, value: &ValueOrContainer) -> Option<Value> {
    match value {
        ValueOrContainer::Value(value) => Some(project(Some(kind), json(value.clone()))),
        ValueOrContainer::Container(_) => None,
    }
}

struct Reconcile<'a, 'b> {
    doc: &'a LoroDoc,
    writer: &'a str,
    ids: &'a mut Vec<String>,
    rows: &'a mut Rows<'b>,
}
impl Reconcile<'_, '_> {
    /// `map[key]` becomes `value`; an absent field or entry is created. `shared` as in
    /// `put`, for the place `map` is.
    fn field(&mut self, map: &LoroMap, key: &str, node: &Node, value: &Value, shared: bool) -> Result<()> {
        let shared = shared || matches!(node, Node::Optional { .. });
        let kind = unwrap_optional(node);
        let Some(current) = map.get(key) else {
            return put(map, key, kind, value, self.writer, shared, self.rows);
        };
        match (kind, current) {
            (scalar, current) if is_scalar(scalar) => {
                if stored(scalar, &current) != Some(project(Some(scalar), value.clone())) {
                    if matches!(current, ValueOrContainer::Container(_)) {
                        return Err(anomalous());
                    }
                    map.insert(key, loro_scalar(scalar, value)).map_err(engine)?;
                }
            }
            (Node::Text {}, ValueOrContainer::Container(Container::Text(text))) => {
                let to = value.as_str().expect("validated text");
                let delta = text::script(&text.to_string(), to, to.chars().count());
                if !delta.is_empty() {
                    text.apply_delta(&delta).map_err(engine)?;
                }
            }
            (Node::Counter {}, ValueOrContainer::Container(Container::Map(counter))) => {
                self.counter(&counter, value.as_i64().expect("validated counter"))?;
            }
            (Node::Object { .. }, ValueOrContainer::Container(Container::Map(child))) => self.object(&child, kind, value, shared)?,
            (Node::Record { value: entry }, ValueOrContainer::Container(Container::Map(record))) => {
                self.record(&record, entry, value)?;
            }
            (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) if is_scalar(item) => {
                rewrite_list(&list, item, value.as_array().expect("validated list"))?;
            }
            (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) => {
                self.list(&list, item, value.as_array().expect("validated list"))?;
            }
            _ => return Err(anomalous()),
        }
        Ok(())
    }
    /// Each declared field; an optional the value leaves out is removed.
    fn object(&mut self, map: &LoroMap, node: &Node, value: &Value, shared: bool) -> Result<()> {
        let Node::Object { properties } = node else {
            return Err(anomalous());
        };
        for (key, child) in properties {
            match value.get(key) {
                Some(value) => self.field(map, key, child, value, shared)?,
                None if map.get(key).is_some() => remove(map, key, self.rows)?,
                None => {}
            }
        }
        Ok(())
    }
    /// Entries the value leaves out are removed; the rest are reconciled or created.
    fn record(&mut self, map: &LoroMap, entry: &Node, value: &Value) -> Result<()> {
        let wanted = value.as_object().expect("validated record");
        let gone: Vec<String> = map.keys().map(|key| key.to_string()).filter(|key| !wanted.contains_key(key)).collect();
        for key in gone {
            remove(map, &key, self.rows)?;
        }
        for (key, value) in wanted {
            self.field(map, key, entry, value, true)?;
        }
        Ok(())
    }
    /// Adds the difference to this writer's contribution, so concurrent increments still
    /// add to the imported total.
    fn counter(&mut self, counter: &LoroMap, target: i64) -> Result<()> {
        let raw = json(counter.get_deep_value());
        let sum = counter_sum(&raw).ok_or_else(anomalous)?;
        let by = target - sum;
        if by == 0 {
            return Ok(());
        }
        let mine = raw.get(self.writer).and_then(Value::as_i64).unwrap_or(0);
        let next = mine
            .checked_add(by)
            .filter(|n| safe(*n))
            .ok_or_else(|| err(Code::OutOfRange, "Counter would leave the safe integer range"))?;
        counter.insert(self.writer, next).map_err(engine)
    }
    /// Rows by `$id`: those the value leaves out are removed, kept rows are reconciled in
    /// place, new ones are inserted (a row without `$id` gets a new one), and only rows
    /// outside the longest run already in the target order move.
    fn list(&mut self, list: &LoroMovableList, item: &Node, values: &[Value]) -> Result<()> {
        let current = identity::clean_rows(list).ok_or_else(anomalous)?;
        let wanted: Vec<String> = values
            .iter()
            .map(|value| value.get("$id").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(application_id))
            .collect();
        let keep: HashSet<&str> = wanted.iter().map(String::as_str).collect();
        for (index, id) in current.iter().enumerate().rev() {
            if !keep.contains(id.as_str()) {
                if let Some(ValueOrContainer::Container(row)) = list.get(index) {
                    release(&row, self.rows)?;
                }
                list.delete(index, 1).map_err(engine)?;
                self.rows.changed(list, Change::Removed(index));
            }
        }
        let existing: HashSet<&str> = current.iter().map(String::as_str).filter(|id| keep.contains(id)).collect();
        let kept: Vec<&str> = wanted.iter().map(String::as_str).filter(|id| existing.contains(id)).collect();
        let positions: Vec<usize> = kept.iter().map(|id| self.rows.index(list, id)).collect::<Result<_>>()?;
        let staying: HashSet<&str> = longest_increasing(&positions).into_iter().map(|i| kept[i]).collect();
        let mut previous: Option<&str> = None;
        for (id, value) in wanted.iter().zip(values) {
            let after = match previous {
                Some(previous) => self.rows.index(list, previous)? + 1,
                None => 0,
            };
            if existing.contains(id.as_str()) {
                let row = self.rows.map(self.doc, list, id)?;
                self.object(&row, item, value, false)?;
                let from = self.rows.index(list, id)?;
                let to = if after > from { after - 1 } else { after };
                if !staying.contains(id.as_str()) && from != to {
                    list.mov(from, to).map_err(engine)?;
                    self.rows.changed(list, Change::Moved(from, to));
                }
            } else {
                insert_row(list, item, after, id, value, self.writer, self.rows)?;
                self.rows.changed(list, Change::Inserted(after, id.clone()));
                self.ids.push(id.clone());
            }
            previous = Some(id);
        }
        Ok(())
    }
}
fn anomalous() -> Error {
    err(Code::TypeMismatch, "Cannot replace an anomalous value")
}
/// Indexes into `values` of a longest strictly increasing subsequence.
fn longest_increasing(values: &[usize]) -> Vec<usize> {
    let mut tails: Vec<usize> = vec![];
    let mut parent = vec![None; values.len()];
    for (i, &value) in values.iter().enumerate() {
        let at = tails.partition_point(|&t| values[t] < value);
        parent[i] = at.checked_sub(1).map(|p| tails[p]);
        if at == tails.len() {
            tails.push(i);
        } else {
            tails[at] = i;
        }
    }
    let mut out = vec![];
    let mut next = tails.last().copied();
    while let Some(i) = next {
        out.push(i);
        next = parent[i];
    }
    out.reverse();
    out
}
