use super::*;

pub(super) fn fill(map: &LoroMap, node: &Node, value: &Value, rows: &mut Rows) -> Result<()> {
    let Node::Object { properties } = node else {
        return Err(err(Code::TypeMismatch, "Expected object"));
    };
    for (key, child) in properties {
        match value.get(key) {
            Some(value) => put(map, key, child, value, rows)?,
            None if matches!(child, Node::Optional { .. }) => {}
            None => return Err(err(Code::TypeMismatch, format!("Missing {key}"))),
        }
    }
    Ok(())
}
/// Stores a validated value at `map[key]` in the representation its kind uses: a plain
/// value for scalars and counters, a new container for text, objects, lists and records.
/// A container stored over an earlier one replaces it whole.
pub(super) fn put(map: &LoroMap, key: &str, node: &Node, value: &Value, rows: &mut Rows) -> Result<()> {
    match node {
        Node::Optional { inner } => put(map, key, inner, value, rows),
        Node::Counter {} => map.insert(key, value.as_i64().expect("validated counter")).map_err(engine),
        scalar if is_scalar(scalar) => map.insert(key, loro_scalar(scalar, value)).map_err(engine),
        Node::Text {} => {
            let text = map.insert_container(key, LoroText::new()).map_err(engine)?;
            text.insert_utf16(0, value.as_str().expect("validated text")).map_err(engine)
        }
        Node::Object { .. } => fill(&map.insert_container(key, LoroMap::new()).map_err(engine)?, node, value, rows),
        Node::List { item } if is_scalar(item) => {
            let list = map.insert_container(key, LoroMovableList::new()).map_err(engine)?;
            for (index, element) in value.as_array().expect("validated list").iter().enumerate() {
                list.insert(index, loro_scalar(item, element)).map_err(engine)?;
            }
            Ok(())
        }
        Node::Record { value: entry } => {
            let record = map.insert_container(key, LoroMap::new()).map_err(engine)?;
            for (key, value) in value.as_object().expect("validated record") {
                put(&record, key, entry, value, rows)?;
            }
            Ok(())
        }
        Node::List { item } => {
            let list = map.insert_container(key, LoroMovableList::new()).map_err(engine)?;
            for (index, row) in value.as_array().expect("validated list").iter().enumerate() {
                let id = row.get("$id").and_then(Value::as_str).map(String::from).unwrap_or_else(application_id);
                insert_row(&list, item, index, &id, row, rows)?;
            }
            Ok(())
        }
        // Validated values only: the descriptor admits nothing else.
        _ => unreachable!("put of a validated {node:?}"),
    }
}
/// Stores a validated row with its ID.
pub(super) fn insert_row(
    list: &LoroMovableList,
    item: &Node,
    index: usize,
    id: &str,
    value: &Value,
    rows: &mut Rows,
) -> Result<()> {
    let row = list.insert_container(index, LoroMap::new()).map_err(engine)?;
    row.insert("$id", id).map_err(engine)?;
    fill(&row, item, value, rows)
}
/// Row lookup during one batch. Untouched lists use the persistent index published
/// from Loro events; a list changed earlier in the same batch keeps its row IDs here,
/// updated with each change.
pub(crate) struct Rows<'a> {
    lists: &'a HashMap<ContainerID, ListState>,
    touched: HashMap<ContainerID, Vec<String>>,
}
pub(super) enum Change {
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
    /// The row from the published index, for a list this batch has not changed.
    fn published(&self, list: &LoroMovableList, id: &str) -> Option<(&ListState, Result<&ContainerID>)> {
        if self.touched.contains_key(&list.id()) {
            return None;
        }
        let state = self.lists.get(&list.id())?;
        Some((state, state.by_id.get(id).ok_or_else(Self::absent)))
    }
    /// The row's current index.
    pub(super) fn index(&self, list: &LoroMovableList, id: &str) -> Result<usize> {
        match (self.published(list, id), self.touched.get(&list.id())) {
            (Some((state, cid)), _) => {
                let cid = cid?;
                state.order.iter().position(|c| c == cid).ok_or_else(|| err(Code::EngineError, "Row index out of sync"))
            }
            (None, Some(ids)) => ids.iter().position(|x| x == id).ok_or_else(Self::absent),
            (None, None) => identity::rows(list).iter().position(|x| x == id).ok_or_else(Self::absent),
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
    pub(super) fn changed(&mut self, list: &LoroMovableList, change: Change) {
        match self.touched.get_mut(&list.id()) {
            Some(ids) => match change {
                Change::Inserted(index, id) => ids.insert(index, id),
                Change::Removed(index) => {
                    ids.remove(index);
                }
                Change::Moved(from, to) => {
                    let id = ids.remove(from);
                    ids.insert(to, id);
                }
            },
            // First change: read the list as it now is.
            None => {
                self.touched.insert(list.id(), identity::rows(list));
            }
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
                        ValueOrContainer::Value(loro::LoroValue::Null)
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
                let child = list.get(*index).ok_or_else(|| err(Code::PathNotFound, "No element at that index"))?;
                element = Some((list.clone(), *index));
                node = item;
                value = child;
                parent = None;
            }
            (Segment::Id { id }, Node::List { item }, ValueOrContainer::Container(Container::MovableList(list)))
                if !is_scalar(item) =>
            {
                let map = rows.map(doc, list, id)?;
                node = item;
                value = ValueOrContainer::Container(Container::Map(map));
                parent = None;
            }
            _ => return Err(err(Code::TypeMismatch, "Path traverses an incompatible value")),
        }
    }
    Ok(Location { node, value, parent, absent, entry, element })
}

/// Validates each intent completely before its first Loro mutation. A failure in a
/// later intent can still leave earlier intents applied; `Document::abort` owns that.
pub(super) fn execute(
    doc: &LoroDoc,
    app: &AppSpec,
    op: &Intent,
    ids: &mut Vec<String>,
    rows: &mut Rows,
    texts: &mut text::Texts,
) -> Result<()> {
    let schema = &app.schema;
    match op {
        Intent::SetTheme { values, replace } => {
            app.theme.set(&doc.get_map(theme::ROOT), values, replace.unwrap_or(false))?
        }
        Intent::ImportTheme { file } => app.theme.import(&doc.get_map(theme::ROOT), file)?,
        // Replace resolves its own path, which may be empty (the whole document).
        Intent::Replace { path, value } => replace::replace(doc, schema, path, value, ids, rows)?,
        Intent::Set { value, from, selection, .. } => {
            let at = resolve(doc, schema, op.path(), rows)?;
            let kind = unwrap_optional(at.node);
            // A text set from the batch's base merges with what changed since.
            if matches!(kind, Node::Text {})
                && at.element.is_none()
                && (texts.base.is_some() || from.is_some() || selection.is_some())
            {
                return text::set(doc, schema, op.path(), at, value, from.as_deref(), *selection, rows, texts);
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
            kind.validate(value, false)?;
            if !at.absent {
                // Replacing an object must not discard identity-bearing collections.
                if replaces_object && holds_collections(kind) {
                    return Err(err(Code::Exists, "Object is already set; edit its fields"));
                }
                // A present object takes the value field by field: unchanged fields write
                // nothing, so concurrent edits to them survive.
                if let (true, ValueOrContainer::Container(Container::Map(object))) = (replaces_object, &at.value) {
                    return replace::object(doc, object, kind, value, ids, rows);
                }
            }
            let (map, key) = at.parent.ok_or_else(|| err(Code::TypeMismatch, "Cannot replace a row"))?;
            put(&map, &key, kind, value, rows)?;
        }
        Intent::Clear { .. } => {
            let at = resolve(doc, schema, op.path(), rows)?;
            if !matches!(at.node, Node::Optional { .. }) && !at.entry {
                return Err(err(Code::TypeMismatch, "Only optional fields and record entries can be cleared"));
            }
            if !at.absent {
                let (map, key) = at.parent.ok_or_else(|| err(Code::TypeMismatch, "Cannot clear a row"))?;
                map.delete(&key).map_err(engine)?;
            }
        }
        Intent::Insert { id, value, at: anchor, index, .. } => {
            let at = resolve(doc, schema, op.path(), rows)?;
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) = (at.node, at.value)
            else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(item) {
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
            if index.is_some() {
                return Err(err(Code::InvalidRequest, "Rows insert by anchor, not index"));
            }
            item.validate(value, true)?;
            let id = id.clone().unwrap_or_else(application_id);
            if !valid_id(&id) {
                return Err(err(Code::InvalidId, "Expected a safe 1–64 character application ID"));
            }
            if value.get("$id").is_some_and(|v| v.as_str() != Some(&*id)) {
                return Err(err(Code::InvalidId, "Conflicting IDs"));
            }
            match rows.map(doc, &list, &id) {
                Err(e) if e.code == Code::PathNotFound => {}
                _ => return Err(err(Code::DuplicateId, "Row already exists")),
            }
            let index = position(&list, anchor, rows)?;
            insert_row(&list, item, index, &id, value, rows)?;
            rows.changed(&list, Change::Inserted(index, id.clone()));
            ids.push(id);
        }
        Intent::Increment { by, .. } => {
            let at = resolve(doc, schema, op.path(), rows)?;
            let (Node::Counter {}, ValueOrContainer::Value(loro::LoroValue::I64(count)), Some((map, key))) =
                (&at.node, &at.value, &at.parent)
            else {
                return Err(err(Code::TypeMismatch, "Expected counter"));
            };
            if *by == 0 || !safe(*by) {
                return Err(err(Code::OutOfRange, "Increment must be a nonzero safe integer"));
            }
            let next = count
                .checked_add(*by)
                .filter(|n| safe(*n))
                .ok_or_else(|| err(Code::OutOfRange, "Counter would leave the safe integer range"))?;
            map.insert(key, next).map_err(engine)?;
        }
        Intent::Remove { id, index, count, .. } => {
            let at = resolve(doc, schema, op.path(), rows)?;
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) = (&at.node, at.value)
            else {
                return Err(err(Code::TypeMismatch, "Expected list"));
            };
            if is_scalar(item) {
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
            let (Some(id), None, None) = (id, index, count) else {
                return Err(err(Code::InvalidRequest, "Rows are removed by id"));
            };
            let index = rows.index(&list, id)?;
            list.delete(index, 1).map_err(engine)?;
            rows.changed(&list, Change::Removed(index));
        }
        Intent::Move { id, at: anchor, .. } => {
            let at = resolve(doc, schema, op.path(), rows)?;
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) = (&at.node, at.value)
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
