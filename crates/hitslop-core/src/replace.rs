//! `replace`: the value at a path (the whole document when empty) becomes the given JSON,
//! written as its differences. Rows are matched by `$id`; kept rows, text and every other
//! container keep their identity, so open text fields, row handles and concurrent edits
//! survive. An unchanged value writes nothing.
use super::*;
use execute::{Change, insert_row, put, rewrite_list};

/// Validates `value` completely before the first mutation.
pub(super) fn replace(
    doc: &LoroDoc,
    schema: &Node,
    path: &[Segment],
    value: &Value,
    ids: &mut Vec<String>,
    rows: &mut Rows,
) -> Result<()> {
    if path.len() > crate::wire::PATH_SEGMENTS {
        return Err(err(Code::InvalidPath, "Path length"));
    }
    let mut to = Reconcile { doc, ids, rows };
    if path.is_empty() {
        schema.validate(value, false)?;
        return to.object(&doc.get_map("data"), schema, value);
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
            to.field(map, key, at.node, value)
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
                return Err(unexpected());
            };
            to.object(row, kind, value)
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
    ids: &mut Vec<String>,
    rows: &mut Rows,
) -> Result<()> {
    Reconcile { doc, ids, rows }.object(map, node, value)
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
    ids: &'a mut Vec<String>,
    rows: &'a mut Rows<'b>,
}
impl Reconcile<'_, '_> {
    /// `map[key]` becomes `value`; an absent field or entry is created.
    fn field(&mut self, map: &LoroMap, key: &str, node: &Node, value: &Value) -> Result<()> {
        let kind = unwrap_optional(node);
        let Some(current) = map.get(key) else {
            return put(map, key, kind, value, self.rows);
        };
        match (kind, current) {
            (scalar, current) if is_scalar(scalar) || matches!(scalar, Node::Counter {}) => {
                if stored(scalar, &current) != Some(project(Some(scalar), value.clone())) {
                    map.insert(
                        key,
                        if is_scalar(scalar) {
                            loro_scalar(scalar, value)
                        } else {
                            value.as_i64().expect("validated counter").into()
                        },
                    )
                    .map_err(engine)?;
                }
            }
            (Node::Text {}, ValueOrContainer::Container(Container::Text(text))) => {
                let to = value.as_str().expect("validated text");
                let delta = text::script(&text.to_string(), to, to.chars().count());
                if !delta.is_empty() {
                    text.apply_delta(&delta).map_err(engine)?;
                }
            }
            (Node::Object { .. }, ValueOrContainer::Container(Container::Map(child))) => {
                self.object(&child, kind, value)?
            }
            (Node::Record { value: entry }, ValueOrContainer::Container(Container::Map(record))) => {
                self.record(&record, entry, value)?;
            }
            (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) if is_scalar(item) => {
                rewrite_list(&list, item, value.as_array().expect("validated list"))?;
            }
            (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) => {
                self.list(&list, item, value.as_array().expect("validated list"))?;
            }
            _ => return Err(unexpected()),
        }
        Ok(())
    }
    /// Each declared field; an optional the value leaves out is removed.
    fn object(&mut self, map: &LoroMap, node: &Node, value: &Value) -> Result<()> {
        let Node::Object { properties } = node else {
            return Err(unexpected());
        };
        for (key, child) in properties {
            match value.get(key) {
                Some(value) => self.field(map, key, child, value)?,
                None if map.get(key).is_some() => map.delete(key).map_err(engine)?,
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
            map.delete(&key).map_err(engine)?;
        }
        for (key, value) in wanted {
            self.field(map, key, entry, value)?;
        }
        Ok(())
    }
    /// Rows by `$id`: those the value leaves out are removed, kept rows are reconciled in
    /// place, new ones are inserted (a row without `$id` gets a new one), and only rows
    /// outside the longest run already in the target order move.
    fn list(&mut self, list: &LoroMovableList, item: &Node, values: &[Value]) -> Result<()> {
        let current = identity::rows(list);
        let wanted: Vec<String> = values
            .iter()
            .map(|value| value.get("$id").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(application_id))
            .collect();
        let keep: HashSet<&str> = wanted.iter().map(String::as_str).collect();
        for (index, id) in current.iter().enumerate().rev() {
            if !keep.contains(id.as_str()) {
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
                self.object(&row, item, value)?;
                let from = self.rows.index(list, id)?;
                let to = if after > from { after - 1 } else { after };
                if !staying.contains(id.as_str()) && from != to {
                    list.mov(from, to).map_err(engine)?;
                    self.rows.changed(list, Change::Moved(from, to));
                }
            } else {
                insert_row(list, item, after, id, value, self.rows)?;
                self.rows.changed(list, Change::Inserted(after, id.clone()));
                self.ids.push(id.clone());
            }
            previous = Some(id);
        }
        Ok(())
    }
}
/// A stored value whose kind is not its descriptor's, which the open-time check and every
/// write rule out.
fn unexpected() -> Error {
    engine("A stored value does not match its descriptor")
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
