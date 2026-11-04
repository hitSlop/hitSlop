//! Change-proportional publications driven by Loro events.
//!
//! Loro reports, after every commit or import, which containers changed and a typed
//! diff for each. Publications are built from those diffs: a text delta, a field's new
//! value, or, for a row list whose rows or order changed, the row insertions, deletions and
//! moves between its rows as last published and as they read now. Neither the before nor
//! the after document is materialized. A change this module cannot place in the
//! application's value falls back to an exact `set` of the whole document.
use super::*;
use layout::RowList;
use loro::event::{Diff, DiffEvent};
use loro::{LoroValue, Subscription};
use std::collections::HashSet;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Slot {
    Value(LoroValue),
    Container(ContainerID),
}
#[derive(Debug)]
pub(crate) enum Change {
    Map(Vec<(String, Option<Slot>)>),
    /// A list's elements changed; the list is read again.
    List,
    /// In Unicode code points of the previous text.
    Text(Vec<Hunk>),
    Counter,
    Other,
}
#[derive(Debug)]
pub(crate) struct Event {
    target: ContainerID,
    path: Vec<(ContainerID, Index)>,
    change: Change,
}
pub(crate) type Events = Arc<Mutex<Vec<Event>>>;

pub(super) fn theme_changed(doc: &LoroDoc, events: &[Event]) -> bool {
    let root = doc.get_map(theme::ROOT).id();
    events.iter().any(|event| event.path.first().is_some_and(|(container, _)| *container == root))
}

fn slot(v: &ValueOrContainer) -> Slot {
    match v {
        ValueOrContainer::Value(v) => Slot::Value(v.clone()),
        ValueOrContainer::Container(c) => Slot::Container(c.id()),
    }
}
/// Events are emitted synchronously at the end of `commit`/`import`; the owner
/// drains them into one publication immediately afterwards.
pub(super) fn subscribe(doc: &LoroDoc, events: &Events) -> Subscription {
    let sink = events.clone();
    doc.subscribe_root(Arc::new(move |e: DiffEvent| {
        let mut out = lock(&sink);
        for c in e.events {
            let change = match &c.diff {
                Diff::Map(m) => {
                    Change::Map(m.updated.iter().map(|(k, v)| (k.to_string(), v.as_ref().map(slot))).collect())
                }
                Diff::List(_) => Change::List,
                Diff::Text(delta) => {
                    let mut hunks: Vec<Hunk> = delta
                        .iter()
                        .filter_map(|item| match item {
                            loro::TextDelta::Retain { retain, .. } => {
                                (*retain > 0).then_some(Hunk::Retain { retain: *retain })
                            }
                            loro::TextDelta::Insert { insert, .. } => {
                                (!insert.is_empty()).then(|| Hunk::Insert { insert: insert.clone() })
                            }
                            loro::TextDelta::Delete { delete } => {
                                (*delete > 0).then_some(Hunk::Delete { delete: *delete })
                            }
                        })
                        .collect();
                    // The rest of the field is retained anyway.
                    while matches!(hunks.last(), Some(Hunk::Retain { .. })) {
                        hunks.pop();
                    }
                    Change::Text(hunks)
                }
                Diff::Counter(_) => Change::Counter,
                _ => Change::Other,
            };
            out.push(Event { target: c.target.clone(), path: c.path.to_vec(), change });
        }
    }))
}

/// Row orders plus reverse ownership by physical map edge. Removing a visible subtree
/// evicts only its cached lists and unregisters their ancestry, including empty branches.
#[derive(Default)]
pub(crate) struct Lists {
    orders: HashMap<ContainerID, Vec<String>>,
    ancestors: HashMap<ContainerID, Vec<(ContainerID, String)>>,
    branches: HashMap<ContainerID, HashMap<String, HashSet<ContainerID>>>,
}
impl Lists {
    fn new() -> Self {
        Self::default()
    }
    fn get(&self, id: &ContainerID) -> Option<&Vec<String>> {
        self.orders.get(id)
    }
    fn insert(&mut self, id: ContainerID, order: Vec<String>) {
        self.orders.insert(id, order);
    }
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.orders.len()
    }

    fn forget(&mut self, id: &ContainerID) {
        self.orders.remove(id);
        for (parent, key) in self.ancestors.remove(id).unwrap_or_default() {
            let empty = if let Some(fields) = self.branches.get_mut(&parent) {
                if let Some(lists) = fields.get_mut(&key) {
                    lists.remove(id);
                    if lists.is_empty() {
                        fields.remove(&key);
                    }
                }
                fields.is_empty()
            } else {
                false
            };
            if empty {
                self.branches.remove(&parent);
            }
        }
    }
    fn forget_field(&mut self, parent: &ContainerID, key: &str) {
        let removed = self.branches.get(parent).and_then(|fields| fields.get(key)).cloned().unwrap_or_default();
        for id in removed {
            self.forget(&id);
        }
    }
    fn forget_children(&mut self, parent: &ContainerID) {
        let removed: HashSet<_> = self
            .branches
            .get(parent)
            .into_iter()
            .flat_map(|fields| fields.values())
            .flat_map(|ids| ids.iter().cloned())
            .collect();
        for id in removed {
            self.forget(&id);
        }
    }
    fn register(&mut self, doc: &LoroDoc, id: ContainerID, order: Vec<String>) {
        self.forget(&id);
        let path = doc.get_path_to_container(&id).unwrap_or_default();
        let ancestors: Vec<_> = path
            .windows(2)
            .filter_map(|edge| {
                let Index::Key(key) = &edge[1].1 else { return None };
                Some((edge[0].0.clone(), key.to_string()))
            })
            .collect();
        for (parent, key) in &ancestors {
            self.branches.entry(parent.clone()).or_default().entry(key.clone()).or_default().insert(id.clone());
        }
        self.ancestors.insert(id.clone(), ancestors);
        self.orders.insert(id, order);
    }
}

/// Every row list in the document, as it reads now. O(document), open only.
pub(super) fn index_all(doc: &LoroDoc, schema: &Node) -> Lists {
    let mut out = Lists::new();
    index(doc, schema, &ValueOrContainer::Container(Container::Map(doc.get_map("data"))), &mut out);
    out
}
/// Records every row list inside `value`, which `node` describes.
fn index(doc: &LoroDoc, node: &Node, value: &ValueOrContainer, lists: &mut Lists) {
    if layout::slotted(Layout::of(doc), node, false) {
        if let Some(value) = layout::slot_value(value) {
            index(doc, unwrap_optional(node), &value, lists);
        }
        return;
    }
    match (unwrap_optional(node), value) {
        (Node::Object { properties }, ValueOrContainer::Container(Container::Map(map))) => {
            for (key, child) in properties {
                if let Some(value) = map.get(key) {
                    index(doc, child, &value, lists);
                }
            }
        }
        (Node::Record { value: entry }, ValueOrContainer::Container(Container::Map(map))) => {
            map.for_each(|_, value| {
                if layout::slotted(Layout::of(doc), entry, true) {
                    if let Some(value) = layout::slot_value(&value) {
                        index(doc, entry, &value, lists);
                    }
                } else {
                    index(doc, entry, &value, lists);
                }
            });
        }
        (Node::List { item }, value) if !is_scalar(item) => {
            if let Ok(list) = RowList::of(Layout::of(doc), value) {
                let ids = list.ids();
                for id in &ids {
                    if let Some(row) = list.row(id) {
                        index(doc, item, &ValueOrContainer::Container(Container::Map(row)), lists);
                    }
                }
                lists.register(doc, list.map.id(), ids);
            }
        }
        _ => {}
    }
}

/// Where a changed container sits in the application's value.
enum Place<'s> {
    Field {
        path: Vec<Segment>,
        node: &'s Node,
    },
    List {
        path: Vec<Segment>,
        node: &'s Node,
        list: ContainerID,
        rows: bool,
    },
    /// A generation selector or payload reference changes the whole visible field.
    Slot {
        path: Vec<Segment>,
        node: &'s Node,
        slot: ContainerID,
    },
    RowSlot {
        path: Vec<Segment>,
        node: &'s Node,
        list: ContainerID,
        id: String,
    },
    Hidden,
}
/// Translates only the selected generation; events in retained history stay hidden.
fn place<'s>(doc: &LoroDoc, schema: &'s Node, path: &[(ContainerID, Index)]) -> Option<Place<'s>> {
    enum In {
        Field,
        Slot(ContainerID),
        ListMap(ContainerID),
        Rows(ContainerID),
        RowSlot { list: ContainerID, slot: ContainerID, id: String },
        Order(ContainerID),
    }
    let mut node = schema;
    let mut out = vec![];
    let mut at = In::Field;
    let selected = |slot: &ContainerID, key: &str| {
        let Container::Map(slot) = doc.get_container(slot.clone())? else { return None };
        Some(layout::generation(&slot).ok()? == key)
    };
    for (child, index) in path.iter().skip(1) {
        let Index::Key(key) = index else { return None };
        at = match at {
            In::Field => {
                let entry = matches!(unwrap_optional(node), Node::Record { .. });
                node = match unwrap_optional(node) {
                    Node::Object { properties } => properties.get(key.as_str())?,
                    Node::Record { value } => value,
                    _ => return None,
                };
                out.push(Segment::Key(key.to_string()));
                if layout::slotted(Layout::of(doc), node, entry) {
                    In::Slot(child.clone())
                } else {
                    match unwrap_optional(node) {
                        Node::List { item } if !is_scalar(item) => In::ListMap(child.clone()),
                        _ => In::Field,
                    }
                }
            }
            In::Slot(slot) => {
                if !selected(&slot, key)? {
                    return Some(Place::Hidden);
                }
                node = unwrap_optional(node);
                In::Field
            }
            In::ListMap(list) => match key.as_str() {
                "rows" => In::Rows(list),
                "order" => In::Order(list),
                _ => return None,
            },
            In::Rows(list) => {
                if Layout::of(doc).generations() {
                    In::RowSlot { list, slot: child.clone(), id: key.to_string() }
                } else {
                    node = match unwrap_optional(node) {
                        Node::List { item } => item,
                        _ => return None,
                    };
                    out.push(Segment::Id { id: key.to_string() });
                    In::Field
                }
            }
            In::RowSlot { list: _, slot, id } => {
                if !selected(&slot, key)? {
                    return Some(Place::Hidden);
                }
                node = match unwrap_optional(node) {
                    Node::List { item } => item,
                    _ => return None,
                };
                out.push(Segment::Id { id });
                In::Field
            }
            In::Order(_) => return None,
        };
    }
    Some(match at {
        In::Field => Place::Field { path: out, node },
        In::Slot(slot) => Place::Slot { path: out, node, slot },
        In::RowSlot { list, id, .. } => Place::RowSlot { path: out, node, list, id },
        In::Rows(list) => Place::List { path: out, node, list, rows: true },
        In::ListMap(list) | In::Order(list) => Place::List { path: out, node, list, rows: false },
    })
}

/// Only changed map keys from the reverse diff, never a second document snapshot.
type PreviousMaps = HashMap<ContainerID, HashMap<String, Option<Slot>>>;

fn map_slot(doc: &LoroDoc, map: &ContainerID, key: &str, previous: Option<&PreviousMaps>) -> Option<Slot> {
    if let Some(value) = previous.and_then(|maps| maps.get(map)).and_then(|fields| fields.get(key)) {
        return value.clone();
    }
    let Container::Map(map) = doc.get_container(map.clone())? else { return None };
    map.get(key).as_ref().map(slot)
}
fn selected_payload(doc: &LoroDoc, value: Option<Slot>, previous: Option<&PreviousMaps>) -> Option<Slot> {
    let Slot::Container(map) = value? else { return None };
    let generation = match map_slot(doc, &map, layout::CURRENT, previous) {
        Some(Slot::Value(LoroValue::String(current))) => current.to_string(),
        None => layout::INITIAL.to_owned(),
        _ => return None,
    };
    map_slot(doc, &map, &generation, previous)
}
fn visible_field(
    doc: &LoroDoc,
    map: &ContainerID,
    key: &str,
    node: &Node,
    entry: bool,
    previous: Option<&PreviousMaps>,
) -> Option<Slot> {
    let value = map_slot(doc, map, key, previous);
    if layout::slotted(Layout::of(doc), node, entry) {
        return selected_payload(doc, value, previous);
    }
    if Layout::of(doc).generations() && value.is_none() && !entry && is_scalar(node) {
        return node.default_value().map(|value| Slot::Value(loro_scalar(node, &value)));
    }
    value
}

/// Whether an inverse changes the visible app. A removed parent reference may already
/// have been hidden by a remote clear, so physical event paths alone are insufficient.
pub(super) fn affects_view(
    doc: &LoroDoc,
    schema: &Node,
    lists: &Lists,
    before: &loro::Frontiers,
    events: &[Event],
) -> Result<bool> {
    if theme_changed(doc, events) {
        return Ok(true);
    }
    let root = doc.get_map("data").id();
    let mut previous = None;
    for event in events.iter().filter(|event| event.path.first().is_some_and(|(id, _)| *id == root)) {
        let place = place(doc, schema, &event.path);
        if matches!(place, Some(Place::Hidden)) {
            continue;
        }
        if let Change::Map(updates) = &event.change {
            if previous.is_none() {
                let diff = doc.diff(&doc.state_frontiers(), before).map_err(engine)?;
                previous = Some(
                    diff.iter()
                        .filter_map(|(id, diff)| {
                            let Diff::Map(changes) = diff else { return None };
                            Some((
                                id.clone(),
                                changes
                                    .updated
                                    .iter()
                                    .map(|(key, value)| (key.to_string(), value.as_ref().map(slot)))
                                    .collect(),
                            ))
                        })
                        .collect::<PreviousMaps>(),
                );
            }
            let old = previous.as_ref();
            let changed = match place {
                Some(Place::Slot { .. } | Place::RowSlot { .. }) => {
                    let id = event.target.clone();
                    selected_payload(doc, Some(Slot::Container(id.clone())), old)
                        != selected_payload(doc, Some(Slot::Container(id)), None)
                }
                Some(Place::Field { node, .. }) => {
                    let (properties, entry) = match unwrap_optional(node) {
                        Node::Object { properties } => (Some(properties), None),
                        Node::Record { value } => (None, Some(&**value)),
                        _ => return Ok(true),
                    };
                    updates.iter().any(|(key, _)| {
                        let Some(child) = properties.and_then(|properties| properties.get(key)).or(entry) else {
                            return true;
                        };
                        visible_field(doc, &event.target, key, child, entry.is_some(), old)
                            != visible_field(doc, &event.target, key, child, entry.is_some(), None)
                    })
                }
                Some(Place::List { rows: true, .. }) => updates.iter().any(|(key, _)| {
                    if !Layout::of(doc).generations() {
                        return map_slot(doc, &event.target, key, old) != map_slot(doc, &event.target, key, None);
                    }
                    selected_payload(doc, map_slot(doc, &event.target, key, old), old)
                        != selected_payload(doc, map_slot(doc, &event.target, key, None), None)
                }),
                _ => true,
            };
            if changed {
                return Ok(true);
            }
        } else {
            match place {
                Some(Place::List { list, .. }) => {
                    let Some(rows) = container(doc, &list).and_then(|value| RowList::of(Layout::of(doc), &value).ok())
                    else {
                        return Ok(true);
                    };
                    if lists.get(&list) != Some(&rows.ids()) {
                        return Ok(true);
                    }
                }
                Some(Place::Field { .. }) | Some(Place::Slot { .. }) | Some(Place::RowSlot { .. }) | None => {
                    return Ok(true);
                }
                Some(Place::Hidden) => {}
            }
        }
    }
    Ok(false)
}

fn deep(doc: &LoroDoc, cid: &ContainerID) -> Value {
    json(
        doc.get_container(cid.clone())
            .map(|c| ValueOrContainer::Container(c).get_deep_value())
            .unwrap_or(LoroValue::Null),
    )
}
fn materialize(doc: &LoroDoc, slot: &Slot) -> Value {
    match slot {
        Slot::Value(v) => json(v.clone()),
        Slot::Container(c) => deep(doc, c),
    }
}
/// A container as a value `index` reads.
fn container(doc: &LoroDoc, cid: &ContainerID) -> Option<ValueOrContainer> {
    doc.get_container(cid.clone()).map(ValueOrContainer::Container)
}

/// The change's ops; `None` when nothing in the document root changed.
pub(super) fn publish(
    doc: &LoroDoc,
    schema: &Node,
    lists: &mut Lists,
    mut events: Vec<Event>,
) -> Result<Option<Vec<PatchOp>>> {
    // Only the document root is projected; other Loro roots are not application data.
    let root = doc.get_map("data").id();
    events.retain(|e| e.path.first().is_some_and(|(c, _)| *c == root));
    if events.is_empty() {
        return Ok(None);
    }
    // Invalidate before filtering descendant events: the introducing map event will
    // index the replacement, while a cleared slot contributes no visible descendants.
    for event in &events {
        let Change::Map(updates) = &event.change else { continue };
        match place(doc, schema, &event.path) {
            Some(Place::Slot { .. } | Place::RowSlot { .. }) => lists.forget_children(&event.target),
            Some(Place::Hidden) | None => {}
            _ => {
                for (key, _) in updates {
                    lists.forget_field(&event.target, key);
                }
            }
        }
    }
    // Containers this change created or replaced are published whole by the map update that
    // introduced them (a field set or a row insertion); their own events are skipped.
    let fresh: HashSet<ContainerID> = events
        .iter()
        .flat_map(|e| match &e.change {
            Change::Map(updates) => updates
                .iter()
                .filter_map(|(_, s)| match s {
                    Some(Slot::Container(c)) => Some(c.clone()),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        })
        .collect();
    let replaced_slots: HashSet<ContainerID> = events
        .iter()
        .filter(|e| matches!(place(doc, schema, &e.path), Some(Place::Slot { .. } | Place::RowSlot { .. })))
        .map(|e| e.target.clone())
        .collect();
    events.retain(|e| {
        !e.path.iter().take(e.path.len() - 1).any(|(c, _)| fresh.contains(c) || replaced_slots.contains(c))
            && !fresh.contains(&e.target)
    });
    let mut ops = vec![];
    let mut touched: Vec<(Vec<Segment>, &Node, ContainerID)> = vec![];
    let mut assigned: HashMap<ContainerID, HashSet<String>> = HashMap::new();
    for e in &events {
        let Some(place) = place(doc, schema, &e.path) else {
            return Ok(Some(everything(doc, schema, lists)));
        };
        match (place, &e.change) {
            (Place::Hidden, _) => {}
            (Place::Slot { path, node, slot }, Change::Map(_)) => {
                let value = container(doc, &slot).and_then(|value| layout::slot_value(&value));
                if let Some(value) = value {
                    index(doc, unwrap_optional(node), &value, lists);
                    ops.push(PatchOp::Set {
                        path,
                        value: project::with_layout(
                            Layout::of(doc),
                            Some(unwrap_optional(node)),
                            json(value.get_deep_value()),
                        ),
                    });
                } else {
                    ops.push(PatchOp::Remove { path });
                }
            }
            (Place::RowSlot { path, node, list, id }, Change::Map(_)) => {
                assigned.entry(list.clone()).or_default().insert(id);
                if !touched.iter().any(|(_, _, l)| *l == list) {
                    touched.push((path, node, list));
                }
            }
            (Place::List { path, node, list, rows }, Change::Map(_) | Change::List) => {
                if rows && let Change::Map(updates) = &e.change {
                    assigned.entry(list.clone()).or_default().extend(
                        updates
                            .iter()
                            .filter(|(_, slot)| matches!(slot, Some(Slot::Container(_))))
                            .map(|(id, _)| id.clone()),
                    );
                }
                if !touched.iter().any(|(_, _, l)| *l == list) {
                    touched.push((path, node, list));
                }
            }
            (Place::Field { path, .. }, Change::Text(delta)) => {
                if !delta.is_empty() {
                    ops.push(PatchOp::Text { path, delta: delta.clone() });
                }
            }
            (Place::Field { path, node }, Change::Counter | Change::List) => {
                ops.push(PatchOp::Set {
                    path,
                    value: project::with_layout(Layout::of(doc), Some(node), deep(doc, &e.target)),
                });
            }
            (Place::Field { path, node }, Change::Map(updates)) => {
                // Object fields are declared by name; every record key has the record's kind.
                let (properties, entries) = match unwrap_optional(node) {
                    Node::Object { properties } => (Some(properties), None),
                    Node::Record { value } => (None, Some(&**value)),
                    _ => return Ok(Some(everything(doc, schema, lists))),
                };
                let mut sorted: Vec<_> = updates.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                for (key, s) in sorted {
                    let mut path = path.clone();
                    path.push(Segment::Key(key.to_owned()));
                    let Some(declared) = properties.and_then(|p| p.get(key)).or(entries) else {
                        return Ok(Some(everything(doc, schema, lists)));
                    };
                    match s {
                        None => {
                            if Layout::of(doc).generations()
                                && entries.is_none()
                                && let Some(default) = declared.default_value()
                            {
                                ops.push(PatchOp::Set { path, value: declared.with_defaults(&default) });
                            } else {
                                ops.push(PatchOp::Remove { path });
                            }
                        }
                        Some(s) => {
                            let slotted = layout::slotted(Layout::of(doc), declared, entries.is_some());
                            if let Slot::Container(c) = s
                                && let Some(value) = container(doc, c)
                            {
                                if slotted {
                                    if let Some(value) = layout::slot_value(&value) {
                                        index(doc, unwrap_optional(declared), &value, lists);
                                    }
                                } else {
                                    index(doc, declared, &value, lists);
                                }
                            }
                            let value = if slotted {
                                project::slot(Layout::of(doc), declared, materialize(doc, s))
                            } else {
                                Some(project::with_layout(Layout::of(doc), Some(declared), materialize(doc, s)))
                            };
                            if let Some(value) = value {
                                ops.push(PatchOp::Set { path, value });
                            } else {
                                ops.push(PatchOp::Remove { path });
                            }
                        }
                    }
                }
            }
            _ => return Ok(Some(everything(doc, schema, lists))),
        }
    }
    for (path, node, list) in touched {
        let Some(rows) = container(doc, &list).and_then(|value| RowList::of(Layout::of(doc), &value).ok()) else {
            return Ok(Some(everything(doc, schema, lists)));
        };
        let Node::List { item } = unwrap_optional(node) else {
            return Ok(Some(everything(doc, schema, lists)));
        };
        let new = rows.ids();
        let Some(old) = lists.get(&list).cloned() else {
            index(doc, node, &ValueOrContainer::Container(Container::Map(rows.map.clone())), lists);
            ops.push(PatchOp::Set { path, value: project::with_layout(Layout::of(doc), Some(node), deep(doc, &list)) });
            continue;
        };
        let (old_set, new_set): (HashSet<&String>, HashSet<&String>) = (old.iter().collect(), new.iter().collect());
        let replaced = assigned.remove(&list).unwrap_or_default();
        // Rows that appear, go or change container are published whole, or as one deletion:
        // drop any field ops this change made inside them, even when the row ID survives.
        let changed: HashSet<&String> =
            old_set.symmetric_difference(&new_set).copied().chain(replaced.iter()).collect();
        ops.retain(|op| {
            !op.path().get(path.len()).is_some_and(|segment| {
                op.path().starts_with(&path) && matches!(segment, Segment::Id { id } if changed.contains(id))
            })
        });
        let mut row_value = |id: &str| {
            let row = rows.row(id)?;
            index(doc, item, &ValueOrContainer::Container(Container::Map(row.clone())), lists);
            let mut value = project::with_layout(Layout::of(doc), Some(item), deep(doc, &row.id()));
            if let Value::Object(fields) = &mut value {
                fields.insert("$id".into(), Value::String(id.to_owned()));
            }
            Some(value)
        };
        let Some(diff) = diff(&path, &old, &new, &mut row_value) else {
            ops.retain(|op| !op.path().starts_with(&path));
            ops.push(PatchOp::Set { path, value: project::with_layout(Layout::of(doc), Some(node), deep(doc, &list)) });
            lists.insert(list, new);
            continue;
        };
        ops.extend(diff);
        for id in new.iter().filter(|id| old_set.contains(id) && replaced.contains(*id)) {
            let Some(value) = row_value(id) else {
                return Ok(Some(everything(doc, schema, lists)));
            };
            let mut row_path = path.clone();
            row_path.push(Segment::Id { id: id.clone() });
            ops.push(PatchOp::Set { path: row_path, value });
        }
        lists.insert(list, new);
    }
    Ok(Some(ops))
}

/// The ops that turn the rows `old` into `new`: deletions, then each row that is not part
/// of the longest run already in order, moved or inserted right after its final
/// predecessor. `None` when that does not reproduce `new`.
fn diff(
    path: &[Segment],
    old: &[String],
    new: &[String],
    mut value: impl FnMut(&str) -> Option<Value>,
) -> Option<Vec<PatchOp>> {
    let new_set: HashSet<&String> = new.iter().collect();
    let old_set: HashSet<&String> = old.iter().collect();
    let mut ops = vec![];
    for id in old.iter().filter(|id| !new_set.contains(id)) {
        ops.push(PatchOp::DeleteRow { path: path.to_vec(), id: id.clone() });
    }
    let mut sim: Vec<&String> = old.iter().filter(|id| new_set.contains(id)).collect();
    let position: HashMap<&String, usize> = new.iter().enumerate().map(|(i, id)| (id, i)).collect();
    let positions: Vec<usize> = sim.iter().map(|id| position[id]).collect();
    let staying: HashSet<&String> = layout::longest_increasing(&positions).into_iter().map(|i| sim[i]).collect();
    for (f, id) in new.iter().enumerate() {
        if staying.contains(id) {
            continue;
        }
        let moved = old_set.contains(id);
        if moved {
            sim.retain(|x| *x != id);
        }
        let at = if f == 0 { 0 } else { sim.iter().position(|x| **x == new[f - 1])? + 1 };
        sim.insert(at, id);
        ops.push(if moved {
            PatchOp::MoveRow { path: path.to_vec(), id: id.clone(), index: at }
        } else {
            PatchOp::InsertRow { path: path.to_vec(), index: at, value: value(id)? }
        });
    }
    (sim.iter().map(|id| id.as_str()).eq(new.iter().map(String::as_str))).then_some(ops)
}

/// The fallback: the whole document as one `set`, with every row list indexed again.
fn everything(doc: &LoroDoc, schema: &Node, lists: &mut Lists) -> Vec<PatchOp> {
    *lists = index_all(doc, schema);
    vec![PatchOp::Set {
        path: vec![],
        value: project::with_layout(Layout::of(doc), Some(schema), json(doc.get_map("data").get_deep_value())),
    }]
}
