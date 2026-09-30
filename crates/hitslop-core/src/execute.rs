use super::*;

pub(super) fn fill(map: &LoroMap, node: &Node, value: &Value, writer: &str) -> Result<()> {
    let Node::Object { properties } = node else {
        return Err(err(Code::TypeMismatch, "Expected object"));
    };
    for (key, child) in properties {
        match value.get(key) {
            Some(value) => put(map, key, child, value, writer)?,
            None if matches!(child, Node::Optional { .. }) => {}
            None => return Err(err(Code::TypeMismatch, format!("Missing {key}"))),
        }
    }
    Ok(())
}
/// Stores a validated value at `map[key]` in the representation its kind uses.
pub(super) fn put(map: &LoroMap, key: &str, node: &Node, value: &Value, writer: &str) -> Result<()> {
    match node {
        Node::Optional { inner } => put(map, key, inner, value, writer),
        scalar if is_scalar(scalar) => map.insert(key, loro_scalar(scalar, value)).map_err(engine),
        Node::Counter {} => {
            let counter = map.insert_container(key, LoroMap::new()).map_err(engine)?;
            let initial = value.as_i64().unwrap();
            if initial != 0 {
                counter.insert(writer, initial).map_err(engine)?;
            }
            Ok(())
        }
        Node::Text {} => {
            let text = map.insert_container(key, LoroText::new()).map_err(engine)?;
            text.insert_utf16(0, value.as_str().unwrap()).map_err(engine)
        }
        Node::Object { .. } => {
            let child = map.insert_container(key, LoroMap::new()).map_err(engine)?;
            fill(&child, node, value, writer)
        }
        Node::List { item } if is_scalar(item) => {
            let list = map.insert_container(key, LoroMovableList::new()).map_err(engine)?;
            for (index, element) in value.as_array().unwrap().iter().enumerate() {
                list.insert(index, loro_scalar(item, element)).map_err(engine)?;
            }
            Ok(())
        }
        Node::Record { value: entry } => {
            let record = map.insert_container(key, LoroMap::new()).map_err(engine)?;
            for (key, value) in value.as_object().unwrap() {
                put(&record, key, entry, value, writer)?;
            }
            Ok(())
        }
        Node::List { item } => {
            let list = map.insert_container(key, LoroMovableList::new()).map_err(engine)?;
            for (index, row) in value.as_array().unwrap().iter().enumerate() {
                let id = row
                    .get("$id")
                    .and_then(Value::as_str)
                    .map(String::from)
                    .map(Ok)
                    .unwrap_or_else(application_id)?;
                insert_row(&list, item, index, &id, row, writer)?;
            }
            Ok(())
        }
        // Validated values only: the descriptor admits nothing else.
        _ => unreachable!("put of a validated {node:?}"),
    }
}
/// Stores a validated row with its ID.
fn insert_row(
    list: &LoroMovableList,
    item: &Node,
    index: usize,
    id: &str,
    value: &Value,
    writer: &str,
) -> Result<()> {
    let row = list
        .insert_container(index, LoroMap::new())
        .map_err(engine)?;
    row.insert("$id", id).map_err(engine)?;
    fill(&row, item, value, writer)
}
/// Row lookup during one batch. Untouched lists use the persistent index published
/// from Loro events. A list changed earlier in the same batch keeps its row IDs here,
/// updated with each change while every row has its own unique ID; an anomalous one is
/// rescanned, because removing a row can change the effective IDs of its duplicates.
pub(crate) struct Rows<'a> {
    lists: &'a HashMap<ContainerID, ListState>,
    touched: HashMap<ContainerID, Option<Vec<String>>>,
}
enum Change {
    Inserted(usize, String),
    Removed(usize),
    Moved(usize, usize),
}
impl<'a> Rows<'a> {
    pub(super) fn new(lists: &'a HashMap<ContainerID, ListState>) -> Self {
        Self { lists, touched: HashMap::new() }
    }
    fn absent() -> Error {
        err(Code::PathNotFound, "Row is absent")
    }
    /// The row from the published index, when it can answer for `id`: always for a clean
    /// list; in an anomalous one, for an ID stored by exactly one row, which is then that
    /// row's effective ID. Other IDs may be derived, and need a scan.
    fn published(&self, list: &LoroMovableList, id: &str) -> Option<(&ListState, Result<&ContainerID>)> {
        if self.touched.contains_key(&list.id()) {
            return None;
        }
        let state = self.lists.get(&list.id())?;
        match state.by_id.get(id).map(Vec::as_slice) {
            Some([cid]) => Some((state, Ok(cid))),
            _ if state.clean() => Some((state, Err(Self::absent()))),
            _ => None,
        }
    }
    /// The row's current index.
    pub(super) fn index(&self, list: &LoroMovableList, id: &str) -> Result<usize> {
        match (self.published(list, id), self.touched.get(&list.id())) {
            (Some((state, cid)), _) => {
                let cid = cid?;
                state.order.iter().position(|c| c.as_ref() == Some(cid))
                    .ok_or_else(|| err(Code::EngineError, "Row index out of sync"))
            }
            (None, Some(Some(ids))) => ids.iter().position(|x| x == id).ok_or_else(Self::absent),
            (None, _) => identity::rows(list).iter().position(|x| x.as_deref() == Some(id)).ok_or_else(Self::absent),
        }
    }
    pub(super) fn map(&self, doc: &LoroDoc, list: &LoroMovableList, id: &str) -> Result<LoroMap> {
        if let Some((_, cid)) = self.published(list, id) {
            return Ok(doc.get_map(cid?.clone()));
        }
        match list.get(self.index(list, id)?) {
            Some(ValueOrContainer::Container(Container::Map(map))) => Ok(map),
            _ => Err(Self::absent()),
        }
    }
    /// Records a change already applied to `list`.
    fn changed(&mut self, list: &LoroMovableList, change: Change) {
        match self.touched.get_mut(&list.id()) {
            Some(Some(ids)) => match change {
                Change::Inserted(index, id) => ids.insert(index, id),
                Change::Removed(index) => { ids.remove(index); }
                Change::Moved(from, to) => {
                    let id = ids.remove(from);
                    ids.insert(to, id);
                }
            },
            Some(None) => {}
            // First change: read the list as it now is.
            None => { self.touched.insert(list.id(), identity::clean_rows(list)); }
        }
    }
}
pub(super) fn position(list: &LoroMovableList, anchor: &Option<Anchor>, rows: &Rows) -> Result<usize> {
    match anchor {
        None => Ok(list.len()),
        Some(Anchor::Before { before }) => rows.index(list, before),
        Some(Anchor::After { after }) => Ok(rows.index(list, after)? + 1),
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
pub(super) fn resolve<'a>(doc: &LoroDoc, schema: &'a Node, path: &[Segment], rows: &Rows) -> Result<Location<'a>> {
    if path.is_empty() || path.len() > 64 {
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
            (
                Segment::Key(key),
                Node::Object { properties },
                ValueOrContainer::Container(Container::Map(map)),
            ) => {
                let next = properties
                    .get(key)
                    .ok_or_else(|| err(Code::PathNotFound, "Unknown field"))?;
                let child = match map.get(key) {
                    Some(child) => child,
                    None if matches!(next, Node::Optional { .. }) => {
                        absent = true;
                        ValueOrContainer::Value(loro::LoroValue::Null)
                    }
                    None => return Err(err(Code::PathNotFound, "Missing field")),
                };
                parent = Some((map.clone(), key.clone()));
                node = next;
                value = child;
            }
            (
                Segment::Key(key),
                Node::Record { value: next },
                ValueOrContainer::Container(Container::Map(map)),
            ) => {
                if !valid_key(key) {
                    return Err(err(Code::InvalidKey, "Record keys are 1–256 characters and not reserved"));
                }
                let child = match map.get(key) {
                    Some(child) => child,
                    None => {
                        absent = true;
                        ValueOrContainer::Value(loro::LoroValue::Null)
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
                let child = list
                    .get(*index)
                    .ok_or_else(|| err(Code::PathNotFound, "No element at that index"))?;
                element = Some((list.clone(), *index));
                node = item;
                value = child;
                parent = None;
            }
            (
                Segment::Id { id },
                Node::List { item },
                ValueOrContainer::Container(Container::MovableList(list)),
            ) if !is_scalar(item) => {
                let map = rows.map(doc, list, id)?;
                node = item;
                value = ValueOrContainer::Container(Container::Map(map));
                parent = None;
            }
            _ => return Err(err(Code::TypeMismatch, "Path traverses an incompatible value")),
        }
    }
    Ok(Location {
        node,
        value,
        parent,
        absent,
        entry,
        element,
    })
}

/// Validates each intent completely before its first Loro mutation. A failure in a
/// later intent can still leave earlier intents applied; `Document::abort` owns that.
pub(super) fn execute(
    doc: &LoroDoc,
    schema: &Node,
    op: &Intent,
    ids: &mut Vec<String>,
    rows: &mut Rows,
) -> Result<()> {
    let at = resolve(doc, schema, op.path(), rows)?;
    match op {
        Intent::Set { value, .. } => {
            let kind = unwrap_optional(&at.node);
            // One scalar-list element: last writer wins.
            if let Some((list, index)) = &at.element {
                kind.validate(value, false)?;
                if let ValueOrContainer::Value(stored) = &at.value {
                    let stored = serde_json::to_value(stored).map_err(engine)?;
                    if scalar_issue(kind, &stored) == Some(IssueCode::TypeMismatch) {
                        return Err(err(Code::TypeMismatch, "Cannot edit anomalous element"));
                    }
                }
                list.set(*index, loro_scalar(kind, value)).map_err(engine)?;
                return Ok(());
            }
            // Whole-field text replaces the text as it is at execution. The script exists
            // before the first mutation, so a slow diff never leaves a batch half applied.
            if let (Node::Text {}, ValueOrContainer::Container(Container::Text(text))) = (kind, &at.value) {
                let to = value.as_str().ok_or_else(|| err(Code::TypeMismatch, "Expected text"))?;
                let delta = text::script(&text.to_string(), to, to.chars().count());
                if !delta.is_empty() {
                    text.apply_delta(&delta).map_err(engine)?;
                }
                return Ok(());
            }
            // A whole scalar list: rewrite it, keeping unchanged positions.
            if let Node::List { item } = kind {
                if !is_scalar(item) {
                    return Err(err(Code::TypeMismatch, "Rows are edited with insert, remove and move"));
                }
                kind.validate(value, false)?;
                let ValueOrContainer::Container(Container::MovableList(list)) = &at.value else {
                    return Err(err(Code::TypeMismatch, "Cannot edit anomalous list"));
                };
                return rewrite_list(list, item, value.as_array().unwrap());
            }
            // Optional fields and record entries can be absent, created and replaced.
            let optional = matches!(at.node, Node::Optional { .. }) || at.entry;
            let replaces_object = matches!(kind, Node::Object { .. }) && optional;
            let creates_text = matches!(kind, Node::Text {}) && optional && at.absent;
            if !is_scalar(kind) && !replaces_object && !creates_text {
                return Err(err(Code::TypeMismatch, "set accepts text, scalars, optional values and record entries"));
            }
            kind.validate(value, false)?;
            if !at.absent {
                // A stored value of the wrong type is a preserved anomaly: never overwritten.
                let anomalous = match (&at.value, replaces_object) {
                    (ValueOrContainer::Container(Container::Map(_)), true) => false,
                    (ValueOrContainer::Container(_), _) | (_, true) => true,
                    (ValueOrContainer::Value(stored), false) => {
                        let stored = serde_json::to_value(stored).map_err(engine)?;
                        scalar_issue(kind, &stored) == Some(IssueCode::TypeMismatch)
                    }
                };
                if anomalous {
                    return Err(err(Code::TypeMismatch, "Cannot edit anomalous field"));
                }
                // Replacing an object must not discard identity-bearing collections.
                if replaces_object && holds_collections(kind) {
                    return Err(err(Code::Exists, "Object is already set; edit its fields"));
                }
            }
            let (map, key) = at
                .parent
                .ok_or_else(|| err(Code::TypeMismatch, "Cannot replace a row"))?;
            put(&map, &key, kind, value, &writer(doc))?;
        }
        Intent::Clear { .. } => {
            if !matches!(at.node, Node::Optional { .. }) && !at.entry {
                return Err(err(Code::TypeMismatch, "Only optional fields and record entries can be cleared"));
            }
            if !at.absent {
                let (map, key) = at
                    .parent
                    .ok_or_else(|| err(Code::TypeMismatch, "Cannot clear a row"))?;
                map.delete(&key).map_err(engine)?;
            }
        }
        Intent::Insert {
            id,
            value,
            at: anchor,
            index,
            ..
        } => {
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) =
                (at.node, at.value)
            else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(&item) {
                if id.is_some() || anchor.is_some() {
                    return Err(err(Code::InvalidRequest, "Scalar lists insert by index"));
                }
                item.validate(value, false)?;
                let index = index.unwrap_or(list.len());
                if index > list.len() {
                    return Err(err(Code::OutOfRange, "Insert index is past the end"));
                }
                list.insert(index, loro_scalar(&item, value)).map_err(engine)?;
                return Ok(());
            }
            if index.is_some() {
                return Err(err(Code::InvalidRequest, "Rows insert by anchor, not index"));
            }
            item.validate(value, true)?;
            let id = id.clone().map(Ok).unwrap_or_else(application_id)?;
            if !valid_id(&id) {
                return Err(err(Code::InvalidId, "Expected a safe 1–64 character application ID"));
            }
            if value.get("$id").is_some_and(|v| v.as_str() != Some(&*id)) {
                return Err(err(Code::InvalidId, "Conflicting IDs"));
            }
            match rows.map(doc, &list, &id) {
                Err(e) if e.code == Code::PathNotFound => {}
                _ => return Err(err(Code::DuplicateId, "Row already exists or is ambiguous")),
            }
            let index = position(&list, anchor, rows)?;
            insert_row(&list, &item, index, &id, value, &writer(doc))?;
            rows.changed(&list, Change::Inserted(index, id.clone()));
            ids.push(id);
        }
        Intent::Increment { by, .. } => {
            let (Node::Counter {}, ValueOrContainer::Container(Container::Map(counter))) =
                (&at.node, &at.value)
            else {
                return Err(err(Code::TypeMismatch, "Expected counter"));
            };
            if *by == 0 || !safe(*by) {
                return Err(err(Code::OutOfRange, "Increment must be a nonzero safe integer"));
            }
            let raw = json(counter.get_deep_value())?;
            let sum = counter_sum(&raw)
                .ok_or_else(|| err(Code::TypeMismatch, "Cannot edit anomalous counter"))?;
            let key = writer(doc);
            let mine = raw.get(&key).and_then(Value::as_i64).unwrap_or(0);
            let next = mine.checked_add(*by).filter(|n| safe(*n));
            let total = sum.checked_add(*by).filter(|n| safe(*n));
            let (Some(next), Some(_)) = (next, total) else {
                return Err(err(Code::OutOfRange, "Counter would leave the safe integer range"));
            };
            counter.insert(&key, next).map_err(engine)?;
        }
        Intent::Remove { id, index, count, .. } => {
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) =
                (&at.node, at.value)
            else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(item) {
                let (Some(index), None) = (index, id) else {
                    return Err(err(Code::InvalidRequest, "Scalar lists remove by index"));
                };
                let count = count.unwrap_or(1);
                if index.checked_add(count).is_none_or(|end| end > list.len()) {
                    return Err(err(Code::OutOfRange, "Remove range is past the end"));
                }
                list.delete(*index, count).map_err(engine)?;
                return Ok(());
            }
            let (Some(id), None, None) = (id, index, count) else {
                return Err(err(Code::InvalidRequest, "Rows are removed by id"));
            };
            let index = rows.index(&list, id)?;
            list.delete(index, 1).map_err(engine)?;
            rows.changed(&list, Change::Removed(index));
        }
        Intent::Move { id, at: anchor, .. } => {
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) =
                (&at.node, at.value)
            else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(item) {
                return Err(err(Code::TypeMismatch, "Scalar lists are edited by index"));
            }
            let from = rows.index(&list, id)?;
            let mut to = position(&list, anchor, rows)?;
            if to > from {
                to -= 1;
            }
            if from != to {
                list.mov(from, to).map_err(engine)?;
                rows.changed(&list, Change::Moved(from, to));
            }
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
    let suffix = current[prefix..]
        .iter()
        .rev()
        .zip(target[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
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

