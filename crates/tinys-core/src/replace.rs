//! `replace`: the value at a path (the whole document when empty) becomes the given JSON,
//! written as its differences. Rows are matched by `$id`; kept rows, text and every other
//! container keep their identity, so open text fields, row handles and concurrent edits
//! survive. An unchanged value writes nothing.
use super::*;
use execute::{put, rewrite_list};
use layout::RowList;
use std::collections::HashSet;

/// Validates `value` completely before the first mutation.
pub(super) fn replace(
    doc: &LoroDoc,
    schema: &Node,
    path: &[Segment],
    value: &Value,
    ids: &mut Vec<String>,
) -> Result<()> {
    if path.len() > crate::wire::PATH_SEGMENTS {
        return Err(err(Code::InvalidPath, "Path length"));
    }
    let mut to = Reconcile { ids };
    if path.is_empty() {
        schema.validate(value, false)?;
        return to.object(&doc.get_map("data"), schema, value);
    }
    let at = resolve(doc, schema, path)?;
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

/// The present object `map` becomes the fully validated `value`, written as its
/// differences so surviving text, lists and rows retain their identities.
pub(super) fn object(map: &LoroMap, node: &Node, value: &Value, ids: &mut Vec<String>) -> Result<()> {
    Reconcile { ids }.object(map, node, value)
}

/// A stored scalar as the snapshot shows it, or none when it is not one.
fn stored(kind: &Node, value: &ValueOrContainer) -> Option<Value> {
    match value {
        ValueOrContainer::Value(value) => Some(project(Some(kind), json(value.clone()))),
        ValueOrContainer::Container(_) => None,
    }
}

struct Reconcile<'a> {
    ids: &'a mut Vec<String>,
}
impl Reconcile<'_> {
    /// `map[key]` becomes `value`; an absent field or entry is created.
    fn field(&mut self, map: &LoroMap, key: &str, node: &Node, value: &Value) -> Result<()> {
        let kind = unwrap_optional(node);
        let Some(current) = map.get(key) else {
            return put(map, key, kind, value);
        };
        match (kind, current) {
            (scalar, current) if is_scalar(scalar) => {
                if stored(scalar, &current) != Some(project(Some(scalar), value.clone())) {
                    map.insert(key, loro_scalar(scalar, value)).map_err(engine)?;
                }
            }
            (Node::Counter {}, ValueOrContainer::Container(Container::Counter(counter))) => {
                layout::set_counter(&counter, value.as_i64().expect("validated counter"))?;
            }
            (Node::Text {}, ValueOrContainer::Container(Container::Text(text))) => {
                layout::set_text(&text, value.as_str().expect("validated text"))?;
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
            (Node::List { item }, current) => {
                self.list(&RowList::of(&current)?, item, value.as_array().expect("validated list"))?;
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
    fn list(&mut self, list: &RowList, item: &Node, values: &[Value]) -> Result<()> {
        let wanted: Vec<String> = values
            .iter()
            .map(|value| value.get("$id").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(application_id))
            .collect();
        // The common import keeps its row IDs in place: reconcile those rows directly.
        if list.ids() == wanted {
            for (id, value) in wanted.iter().zip(values) {
                self.object(&list.row(id).ok_or_else(unexpected)?, item, value)?;
            }
            return Ok(());
        }
        let keep: HashSet<&str> = wanted.iter().map(String::as_str).collect();
        for id in list.ids().iter().filter(|id| !keep.contains(id.as_str())) {
            list.remove(id)?;
        }
        // A replacement states the whole order: `order` then names each row once.
        list.normalize()?;
        let existing: HashSet<String> = list.ids().into_iter().collect();
        let kept: Vec<&str> = wanted.iter().map(String::as_str).filter(|id| existing.contains(*id)).collect();
        let positions: Vec<usize> = kept.iter().map(|id| list.index(id)).collect::<Result<_>>()?;
        let staying: HashSet<&str> = layout::longest_increasing(&positions).into_iter().map(|i| kept[i]).collect();
        let mut previous: Option<&str> = None;
        for (id, value) in wanted.iter().zip(values) {
            let after = match previous {
                Some(previous) => list.index(previous)? + 1,
                None => 0,
            };
            if existing.contains(id) {
                self.object(&list.row(id).ok_or_else(unexpected)?, item, value)?;
                let from = list.index(id)?;
                let to = if after > from { after - 1 } else { after };
                if !staying.contains(id.as_str()) && from != to {
                    list.mov_index(from, to)?;
                }
            } else {
                list.insert(after, id, item, value)?;
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
