use super::*;
use layout::RowList;

/// Writes every declared field of the object `value` into `map`. A field the value leaves
/// out is removed when optional: a revived map may still hold it.
pub(super) fn fill(map: &LoroMap, node: &Node, value: &Value) -> Result<()> {
    let Node::Object { properties } = node else {
        return Err(err(Code::TypeMismatch, "Expected object"));
    };
    for (key, child) in properties {
        match value.get(key) {
            Some(value) => put(map, key, child, value)?,
            None if matches!(child, Node::Optional { .. }) => {
                if map.get(key).is_some() {
                    map.delete(key).map_err(engine)?;
                }
            }
            None => return Err(err(Code::TypeMismatch, format!("Missing {key}"))),
        }
    }
    Ok(())
}
/// Stores a validated value at `map[key]` in the representation its kind uses (`layout`):
/// a plain value for scalars, a mergeable child for text, counters, objects, records and
/// lists. Writing over a child replaces its whole content.
pub(super) fn put(map: &LoroMap, key: &str, node: &Node, value: &Value) -> Result<()> {
    match node {
        Node::Optional { inner } => put(map, key, inner, value),
        scalar if is_scalar(scalar) => map.insert(key, loro_scalar(scalar, value)).map_err(engine),
        Node::Counter {} => {
            let counter = map.ensure_mergeable_counter(key).map_err(engine)?;
            layout::set_counter(&counter, value.as_i64().expect("validated counter"))
        }
        Node::Text {} => {
            let text = map.ensure_mergeable_text(key).map_err(engine)?;
            layout::set_text(&text, value.as_str().expect("validated text"))
        }
        Node::Object { .. } => fill(&map.ensure_mergeable_map(key).map_err(engine)?, node, value),
        Node::Record { value: entry } => {
            let record = map.ensure_mergeable_map(key).map_err(engine)?;
            let wanted = value.as_object().expect("validated record");
            let gone: Vec<String> = record.keys().map(|k| k.to_string()).filter(|k| !wanted.contains_key(k)).collect();
            for key in gone {
                record.delete(&key).map_err(engine)?;
            }
            for (key, value) in wanted {
                put(&record, key, entry, value)?;
            }
            Ok(())
        }
        Node::List { item } if is_scalar(item) => {
            let list = map.ensure_mergeable_movable_list(key).map_err(engine)?;
            rewrite_list(&list, item, value.as_array().expect("validated list"))
        }
        Node::List { item } => {
            let list = RowList::ensure(map, key)?;
            list.clear()?;
            for (index, row) in value.as_array().expect("validated list").iter().enumerate() {
                let id = row.get("$id").and_then(Value::as_str).map(String::from).unwrap_or_else(application_id);
                list.insert(index, &id, item, row)?;
            }
            Ok(())
        }
        // Validated values only: the descriptor admits nothing else.
        _ => unreachable!("put of a validated {node:?}"),
    }
}
pub(super) struct Location<'a> {
    pub(super) node: &'a Node,
    pub(super) value: ValueOrContainer,
    pub(super) parent: Option<(LoroMap, String)>,
    /// The final segment names an optional field or record entry that is not set
    /// (`value` is null).
    pub(super) absent: bool,
    /// The final segment is a record key: the entry is created by `set`, removed by `clear`.
    pub(super) entry: bool,
    /// The final segment is a scalar-list element: the list and its index.
    pub(super) element: Option<(LoroMovableList, usize)>,
}
pub(super) fn resolve<'a>(doc: &LoroDoc, schema: &'a Node, path: &[Segment]) -> Result<Location<'a>> {
    if path.is_empty() || path.len() > crate::wire::PATH_SEGMENTS {
        return Err(err(Code::InvalidPath, "Path length"));
    }
    let mut node = schema;
    let mut value = ValueOrContainer::Container(Container::Map(doc.get_map("data")));
    let mut parent = None;
    let mut absent = false;
    let mut entry = false;
    let mut element = None;
    for (index, segment) in path.iter().enumerate() {
        entry = false;
        element = None;
        if absent {
            return Err(err(Code::PathNotFound, "Optional field is not set"));
        }
        // A set optional behaves as its inner kind when a path continues through it.
        let current = if index == 0 { node } else { unwrap_optional(node) };
        match (segment, current, &value) {
            (Segment::Key(key), Node::Object { properties }, ValueOrContainer::Container(Container::Map(map))) => {
                let next = properties.get(key).ok_or_else(|| err(Code::PathNotFound, "Unknown field"))?;
                let child = match map.get(key) {
                    Some(child) => child,
                    None if matches!(next, Node::Optional { .. }) => {
                        absent = true;
                        ValueOrContainer::Value(LoroValue::Null)
                    }
                    None => return Err(err(Code::PathNotFound, "Missing field")),
                };
                parent = Some((map.clone(), key.clone()));
                node = next;
                value = child;
            }
            (Segment::Key(key), Node::Record { value: next }, ValueOrContainer::Container(Container::Map(map))) => {
                if !valid_key(key) {
                    return Err(err(Code::InvalidKey, "Record keys are 1–256 characters and not reserved"));
                }
                let child = match map.get(key) {
                    Some(child) => child,
                    None => {
                        absent = true;
                        ValueOrContainer::Value(LoroValue::Null)
                    }
                };
                parent = Some((map.clone(), key.clone()));
                entry = true;
                node = next;
                value = child;
            }
            (
                Segment::Index { index },
                Node::List { item },
                ValueOrContainer::Container(Container::MovableList(list)),
            ) if is_scalar(item) => {
                let child = list.get(*index).ok_or_else(|| err(Code::PathNotFound, "No element at that index"))?;
                element = Some((list.clone(), *index));
                node = item;
                value = child;
                parent = None;
            }
            (Segment::Id { id }, Node::List { item }, list) if !is_scalar(item) => {
                let row = RowList::of(list)?.row(id).ok_or_else(|| err(Code::PathNotFound, "Row is absent"))?;
                node = item;
                value = ValueOrContainer::Container(Container::Map(row));
                parent = None;
            }
            _ => return Err(err(Code::TypeMismatch, "Path traverses an incompatible value")),
        }
    }
    Ok(Location { node, value, parent, absent, entry, element })
}

/// Validates each intent completely before its first Loro mutation. A failure in a
/// later intent can still leave earlier intents applied; `Document::apply` rehearses such
/// batches and `Document::abort` owns what remains.
pub(super) fn execute(
    doc: &LoroDoc,
    app: &AppSpec,
    op: &Intent,
    ids: &mut Vec<String>,
    typed: &mut Option<text::Typed>,
) -> Result<()> {
    let schema = &app.schema;
    match op {
        Intent::SetTheme { values, replace } => {
            app.theme.set(&doc.get_map(theme::ROOT), values, replace.unwrap_or(false))?
        }
        Intent::ImportTheme { file } => app.theme.import(&doc.get_map(theme::ROOT), file)?,
        // Replace resolves its own path, which may be empty (the whole document).
        Intent::Replace { path, value } => replace::replace(doc, schema, path, value, ids)?,
        Intent::Set { value, from, selection, .. } => {
            let at = resolve(doc, schema, op.path())?;
            let kind = unwrap_optional(at.node);
            // A text set from what the writer saw merges with what changed since.
            if matches!(kind, Node::Text {}) && at.element.is_none() && (from.is_some() || selection.is_some()) {
                return text::set(op.path(), at, value, from.as_deref(), *selection, typed);
            }
            if from.is_some() || selection.is_some() {
                return Err(err(Code::TypeMismatch, "`from` and `selection` apply to text"));
            }
            // One scalar-list element: last writer wins.
            if let Some((list, index)) = &at.element {
                kind.validate(value, false)?;
                list.set(*index, loro_scalar(kind, value)).map_err(engine)?;
                return Ok(());
            }
            // Whole-field text replaces the text as it is at execution. The script exists
            // before the first mutation, so a slow diff never leaves a batch half applied.
            if let (Node::Text {}, ValueOrContainer::Container(Container::Text(text))) = (kind, &at.value) {
                let to = value.as_str().ok_or_else(|| err(Code::TypeMismatch, "Expected text"))?;
                return layout::set_text(text, to);
            }
            // A whole scalar list: rewrite it, keeping unchanged positions.
            if let Node::List { item } = kind {
                if !is_scalar(item) {
                    return Err(err(Code::TypeMismatch, "Rows are edited with insert, remove and move"));
                }
                kind.validate(value, false)?;
                let ValueOrContainer::Container(Container::MovableList(list)) = &at.value else {
                    return Err(engine("A scalar list is not stored as a list"));
                };
                return rewrite_list(list, item, value.as_array().expect("validated list"));
            }
            // Optional fields and record entries can be absent, created and replaced.
            let optional = matches!(at.node, Node::Optional { .. }) || at.entry;
            let replaces_object = matches!(kind, Node::Object { .. }) && optional;
            let creates_text = matches!(kind, Node::Text {}) && optional && at.absent;
            if !is_scalar(kind) && !replaces_object && !creates_text {
                return Err(err(Code::TypeMismatch, "set accepts text, scalars, optional values and record entries"));
            }
            // Defaults belong to creation. Replacing a present object requires its
            // complete value, so omitting a field cannot silently reset saved content.
            let created;
            let value = if at.absent {
                created = kind.with_defaults(value);
                &created
            } else {
                value
            };
            kind.validate(value, false)?;
            if !at.absent {
                // A present object takes the value field by field: unchanged fields write
                // nothing, so concurrent edits to them survive.
                if let (true, ValueOrContainer::Container(Container::Map(object))) = (replaces_object, &at.value) {
                    return replace::object(object, kind, value, ids);
                }
            }
            let (map, key) = at.parent.ok_or_else(|| err(Code::TypeMismatch, "Cannot replace a row"))?;
            put(&map, &key, kind, value)?;
        }
        Intent::Clear { .. } => {
            let at = resolve(doc, schema, op.path())?;
            if !matches!(at.node, Node::Optional { .. }) && !at.entry {
                return Err(err(Code::TypeMismatch, "Only optional fields and record entries can be cleared"));
            }
            if !at.absent {
                let (map, key) = at.parent.ok_or_else(|| err(Code::TypeMismatch, "Cannot clear a row"))?;
                map.delete(&key).map_err(engine)?;
            }
        }
        Intent::Insert { id, value, at: anchor, index, .. } => {
            let at = resolve(doc, schema, op.path())?;
            let Node::List { item } = unwrap_optional(at.node) else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(item) {
                let ValueOrContainer::Container(Container::MovableList(list)) = at.value else {
                    return Err(err(Code::TypeMismatch, "Expected list"));
                };
                if id.is_some() || anchor.is_some() {
                    return Err(err(Code::InvalidRequest, "Scalar lists insert by index"));
                }
                item.validate(value, false)?;
                let index = index.unwrap_or(list.len());
                if index > list.len() {
                    return Err(err(Code::OutOfRange, "Insert index is past the end"));
                }
                list.insert(index, loro_scalar(item, value)).map_err(engine)?;
                return Ok(());
            }
            let list = RowList::of(&at.value)?;
            if index.is_some() {
                return Err(err(Code::InvalidRequest, "Rows insert by anchor, not index"));
            }
            let value = &item.with_defaults(value);
            item.validate(value, true)?;
            let id = id.clone().unwrap_or_else(application_id);
            if !valid_id(&id) {
                return Err(err(Code::InvalidId, "Expected a safe 1–64 character application ID"));
            }
            if value.get("$id").is_some_and(|v| v.as_str() != Some(&*id)) {
                return Err(err(Code::InvalidId, "Conflicting IDs"));
            }
            if list.row(&id).is_some() {
                return Err(err(Code::DuplicateId, "Row already exists"));
            }
            let at = list.anchor(anchor)?;
            list.insert(at, &id, item, value)?;
            ids.push(id);
        }
        Intent::Increment { by, .. } => {
            let at = resolve(doc, schema, op.path())?;
            let (Node::Counter {}, ValueOrContainer::Container(Container::Counter(counter))) = (&at.node, &at.value)
            else {
                return Err(err(Code::TypeMismatch, "Expected counter"));
            };
            if *by == 0 || !safe(*by) {
                return Err(err(Code::OutOfRange, "Increment must be a nonzero safe integer"));
            }
            layout::counter_value(counter)
                .checked_add(*by)
                .filter(|n| safe(*n))
                .ok_or_else(|| err(Code::OutOfRange, "Counter would leave the safe integer range"))?;
            counter.increment(*by as f64).map_err(engine)?;
        }
        Intent::Remove { id, index, count, .. } => {
            let at = resolve(doc, schema, op.path())?;
            let Node::List { item } = unwrap_optional(at.node) else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(item) {
                let ValueOrContainer::Container(Container::MovableList(list)) = at.value else {
                    return Err(err(Code::TypeMismatch, "Expected list"));
                };
                let (Some(index), None) = (index, id) else {
                    return Err(err(Code::InvalidRequest, "Scalar lists remove by index"));
                };
                let count = count.unwrap_or(1);
                if count == 0 {
                    return Err(err(Code::OutOfRange, "Remove count must be at least 1"));
                }
                if index.checked_add(count).is_none_or(|end| end > list.len()) {
                    return Err(err(Code::OutOfRange, "Remove range is past the end"));
                }
                list.delete(*index, count).map_err(engine)?;
                return Ok(());
            }
            let list = RowList::of(&at.value)?;
            let (Some(id), None, None) = (id, index, count) else {
                return Err(err(Code::InvalidRequest, "Rows are removed by id"));
            };
            list.row(id).ok_or_else(|| err(Code::PathNotFound, "Row is absent"))?;
            list.remove(id)?;
        }
        Intent::Move { id, at: anchor, .. } => {
            let at = resolve(doc, schema, op.path())?;
            let Node::List { item } = unwrap_optional(at.node) else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(item) {
                return Err(err(Code::TypeMismatch, "Scalar lists are edited by index"));
            }
            let list = RowList::of(&at.value)?;
            list.row(id).ok_or_else(|| err(Code::PathNotFound, "Row is absent"))?;
            if let Some(Anchor::Before { before: other } | Anchor::After { after: other }) = anchor {
                list.row(other).ok_or_else(|| err(Code::PathNotFound, "Row is absent"))?;
            }
            list.mov(id, anchor)?;
        }
    }
    Ok(())
}

/// Rewrites a scalar list to `values`, keeping the common prefix and suffix and setting
/// overlapping positions, so concurrent edits outside the changed span survive.
pub(super) fn rewrite_list(list: &LoroMovableList, item: &Node, values: &[Value]) -> Result<()> {
    let current: Vec<Value> = (0..list.len())
        .map(|i| match list.get(i) {
            Some(ValueOrContainer::Value(v)) => serde_json::to_value(v).unwrap_or(Value::Null),
            _ => Value::Null,
        })
        .map(|v| project(Some(item), v))
        .collect();
    let target: Vec<Value> = values.iter().map(|v| project(Some(item), v.clone())).collect();
    let prefix = current.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let suffix = current[prefix..].iter().rev().zip(target[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    let (old_mid, new_mid) = (current.len() - prefix - suffix, target.len() - prefix - suffix);
    for offset in 0..old_mid.min(new_mid) {
        if current[prefix + offset] != target[prefix + offset] {
            list.set(prefix + offset, loro_scalar(item, &values[prefix + offset])).map_err(engine)?;
        }
    }
    if old_mid > new_mid {
        list.delete(prefix + new_mid, old_mid - new_mid).map_err(engine)?;
    }
    for offset in old_mid..new_mid {
        list.insert(prefix + offset, loro_scalar(item, &values[prefix + offset])).map_err(engine)?;
    }
    Ok(())
}
