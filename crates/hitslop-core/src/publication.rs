//! Change-proportional publications driven by Loro events.
//!
//! Loro reports, after every commit or import, which containers changed and a typed
//! diff for each. Publications are built from those diffs plus persistent per-list
//! row indexes; neither the before nor the after document is materialized. Values
//! are materialized only for changed fields and inserted rows. A delta this module cannot
//! read against its index falls back to an exact `set` of that list.
use super::*;
use loro::event::{Diff, DiffEvent, ListDiffItem};
use loro::{ContainerType, LoroValue, Subscription};
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub(crate) enum Slot {
    Value(LoroValue),
    Container(ContainerID),
}
#[derive(Debug)]
pub(crate) enum Item {
    Retain(usize),
    Delete(usize),
    Insert(Vec<Slot>, bool),
}
#[derive(Debug)]
pub(crate) enum Change {
    Map(Vec<(String, Option<Slot>)>),
    List(Vec<Item>),
    /// In Unicode code points of the previous text.
    Text(Vec<Hunk>),
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
                Diff::List(items) => Change::List(
                    items
                        .iter()
                        .map(|item| match item {
                            ListDiffItem::Retain { retain } => Item::Retain(*retain),
                            ListDiffItem::Delete { delete } => Item::Delete(*delete),
                            ListDiffItem::Insert { insert, is_move } => {
                                Item::Insert(insert.iter().map(slot).collect(), *is_move)
                            }
                        })
                        .collect(),
                ),
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
                _ => Change::Other,
            };
            out.push(Event { target: c.target.clone(), path: c.path.to_vec(), change });
        }
    }))
}

/// Order and row identities of one row list, as of the last publication. Every row is a
/// map holding its own unique `$id` (the open-time check and every write keep it so).
#[derive(Default, Debug)]
pub(crate) struct ListState {
    pub(crate) order: Vec<ContainerID>,
    id_of: HashMap<ContainerID, String>,
    pub(crate) by_id: HashMap<String, ContainerID>,
}
impl ListState {
    fn row_id(doc: &LoroDoc, cid: &ContainerID) -> Option<String> {
        if cid.container_type() != ContainerType::Map {
            return None;
        }
        stored_id(&doc.get_map(cid.clone()))
    }
    fn add(&mut self, doc: &LoroDoc, cid: ContainerID) {
        if let Some(id) = Self::row_id(doc, &cid) {
            self.by_id.insert(id.clone(), cid.clone());
            self.id_of.insert(cid, id);
        }
    }
    fn holds(&self, cid: &ContainerID) -> bool {
        self.id_of.contains_key(cid)
    }
    fn drop_row(&mut self, cid: &ContainerID) {
        if let Some(id) = self.id_of.remove(cid) {
            self.by_id.remove(&id);
        }
    }
    fn build(doc: &LoroDoc, list: &LoroMovableList) -> Self {
        let mut state = Self::default();
        state.order.reserve(list.len());
        list.for_each(|v| {
            if let ValueOrContainer::Container(c) = v {
                state.order.push(c.id());
                state.add(doc, c.id());
            }
        });
        state
    }
}
/// Indexes every movable list reachable from the document root. O(document), open only.
pub(super) fn index_all(doc: &LoroDoc) -> HashMap<ContainerID, ListState> {
    let mut out = HashMap::new();
    walk(doc, &Container::Map(doc.get_map("data")), &mut out, &mut HashSet::new());
    out
}
/// Indexes every row list under `container`, replacing what the index held.
fn walk(
    doc: &LoroDoc,
    container: &Container,
    out: &mut HashMap<ContainerID, ListState>,
    seen: &mut HashSet<ContainerID>,
) {
    let mut children = vec![];
    match container {
        Container::Map(map) => map.for_each(|_, v| {
            if let ValueOrContainer::Container(c) = v {
                children.push(c);
            }
        }),
        Container::MovableList(list) => {
            // A list inside several new containers is indexed once.
            if !seen.insert(list.id()) {
                return;
            }
            out.insert(list.id(), ListState::build(doc, list));
            list.for_each(|v| {
                if let ValueOrContainer::Container(c) = v {
                    children.push(c);
                }
            });
        }
        _ => {}
    }
    for child in &children {
        walk(doc, child, out, seen);
    }
}

pub(super) fn node_at<'s>(schema: &'s Node, path: &[(ContainerID, Index)]) -> Option<&'s Node> {
    let mut node = schema;
    for (_, index) in path.iter().skip(1) {
        node = match (unwrap_optional(node), index) {
            (Node::Object { properties }, Index::Key(key)) => properties.get(key.as_str())?,
            (Node::Record { value }, Index::Key(_)) => value,
            (Node::List { item }, Index::Seq(_)) => item,
            _ => return None,
        };
    }
    Some(unwrap_optional(node))
}
/// Converts a Loro container path to a publication path. Rows are addressed by `$id`; a
/// row the index does not know makes its list the fallback container.
pub(super) fn json_path(
    lists: &HashMap<ContainerID, ListState>,
    path: &[(ContainerID, Index)],
) -> std::result::Result<Vec<Segment>, ContainerID> {
    let mut out = Vec::with_capacity(path.len());
    for ((parent, _), (child, index)) in path.iter().zip(path.iter().skip(1)) {
        match index {
            Index::Key(key) => out.push(Segment::Key(key.to_string())),
            Index::Seq(_) => match lists.get(parent).and_then(|s| s.id_of.get(child)) {
                Some(id) => out.push(Segment::Id { id: id.clone() }),
                None => return Err(parent.clone()),
            },
            Index::Node(_) => return Err(parent.clone()),
        }
    }
    Ok(out)
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

struct Out {
    ops: Vec<PatchOp>,
    fallback: Vec<ContainerID>,
    /// A removed value may have held lists whose indexes must be dropped.
    detached: bool,
}

/// The change's ops; `None` when nothing in the document root changed.
pub(super) fn publish(
    doc: &LoroDoc,
    schema: &Node,
    lists: &mut HashMap<ContainerID, ListState>,
    mut events: Vec<Event>,
) -> Result<Option<Vec<PatchOp>>> {
    // Only the document root is projected; other Loro roots are not application data.
    let root = doc.get_map("data").id();
    events.retain(|e| e.path.first().is_some_and(|(c, _)| *c == root));
    if events.is_empty() {
        return Ok(None);
    }
    // Containers created by this change are published whole by whichever event
    // introduced them (a row insertion or a map update); their own events are skipped.
    let mut fresh = HashSet::new();
    for e in &events {
        match &e.change {
            Change::Map(updates) => {
                for (_, s) in updates {
                    if let Some(Slot::Container(c)) = s {
                        fresh.insert(c.clone());
                    }
                }
            }
            Change::List(items) => {
                let state = lists.get(&e.target);
                for item in items {
                    if let Item::Insert(slots, is_move) = item {
                        for s in slots {
                            if let Slot::Container(c) = s
                                && (!is_move || !state.is_some_and(|state| state.holds(c)))
                            {
                                fresh.insert(c.clone());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Loro omits events for containers removed by the same change (a nested edit
    // followed by removal of its row publishes only the row deletion), and rows are
    // addressed by `$id`, so events apply in their emitted order.
    events.retain(|e| !e.path.iter().any(|(c, _)| fresh.contains(c)));
    let mut out = Out { ops: vec![], fallback: vec![], detached: false };
    for e in &events {
        match &e.change {
            Change::Map(updates) => map_ops(doc, schema, lists, e, updates, &mut out),
            Change::Text(delta) => match json_path(lists, &e.path) {
                Ok(path) if !delta.is_empty() => out.ops.push(PatchOp::Text { path, delta: delta.clone() }),
                Ok(_) => {}
                Err(c) => out.fallback.push(c),
            },
            Change::List(items) => list_ops(doc, schema, lists, e, items, &mut out),
            Change::Other => out.fallback.push(e.target.clone()),
        }
    }
    let mut seen = HashSet::new();
    for cid in &fresh {
        if let Some(container) = doc.get_container(cid.clone()) {
            walk(doc, &container, lists, &mut seen);
        }
    }
    finish_fallbacks(doc, schema, lists, &mut out);
    if out.detached {
        lists.retain(|cid, _| doc.get_path_to_container(cid).is_some());
    }
    Ok(Some(out.ops))
}

fn map_ops(
    doc: &LoroDoc,
    schema: &Node,
    lists: &mut HashMap<ContainerID, ListState>,
    e: &Event,
    updates: &[(String, Option<Slot>)],
    out: &mut Out,
) {
    let node = node_at(schema, &e.path);
    let base = match json_path(lists, &e.path) {
        Ok(path) => path,
        Err(c) => {
            out.fallback.push(c);
            return;
        }
    };
    // Object fields are declared by name; every record key has the record's value kind.
    let (properties, entries) = match node {
        Some(Node::Object { properties }) => (Some(properties), None),
        Some(Node::Record { value }) => (None, Some(&**value)),
        _ => (None, None),
    };
    let mut sorted: Vec<_> = updates.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (key, s) in sorted {
        let mut path = base.clone();
        path.push(Segment::Key(key.to_owned()));
        let declared = properties.and_then(|p| p.get(key)).or(entries);
        // Replacing or removing a value can detach the lists it held.
        out.detached |= declared.is_none_or(holds_collections);
        match s {
            None => out.ops.push(PatchOp::Remove { path }),
            Some(s) => out.ops.push(PatchOp::Set { path, value: project(declared, materialize(doc, s)) }),
        }
    }
}

fn list_ops(
    doc: &LoroDoc,
    schema: &Node,
    lists: &mut HashMap<ContainerID, ListState>,
    e: &Event,
    items: &[Item],
    out: &mut Out,
) {
    let node = node_at(schema, &e.path);
    let Ok(path) = json_path(lists, &e.path) else {
        out.fallback.push(e.target.clone());
        return;
    };
    let item_node = match node {
        Some(Node::List { item }) => &**item,
        _ => {
            out.fallback.push(e.target.clone());
            return;
        }
    };
    // A scalar list has no row identity: publish it whole. The lists are small.
    if is_scalar(item_node) {
        let value = json(doc.get_movable_list(e.target.clone()).get_deep_value());
        out.ops.push(PatchOp::Set { path, value: project(node, value) });
        return;
    }
    let Some(state) = lists.get_mut(&e.target) else {
        // Not indexed: cannot interpret the delta against a previous order.
        let list = doc.get_movable_list(e.target.clone());
        lists.insert(e.target.clone(), ListState::build(doc, &list));
        out.fallback.push(e.target.clone());
        return;
    };
    let old = &state.order;
    let mut next = Vec::with_capacity(old.len() + 1);
    let mut removed = vec![];
    let mut moved = HashSet::new();
    let mut inserted = vec![];
    let mut cursor = 0usize;
    let mut in_sync = true;
    for item in items {
        match item {
            Item::Retain(n) | Item::Delete(n) if cursor + n > old.len() => {
                in_sync = false;
                break;
            }
            Item::Retain(n) => {
                next.extend_from_slice(&old[cursor..cursor + n]);
                cursor += n;
            }
            Item::Delete(n) => {
                removed.extend_from_slice(&old[cursor..cursor + n]);
                cursor += n;
            }
            Item::Insert(slots, is_move) => {
                for s in slots {
                    let Slot::Container(c) = s else {
                        in_sync = false;
                        break;
                    };
                    next.push(c.clone());
                    // A row inserted and moved in one change arrives as a move of a row
                    // the list never held.
                    if *is_move && state.holds(c) {
                        moved.insert(c.clone());
                    } else {
                        inserted.push(c.clone());
                    }
                }
            }
        }
    }
    if !in_sync {
        *state = ListState::build(doc, &doc.get_movable_list(e.target.clone()));
        out.fallback.push(e.target.clone());
        return;
    }
    next.extend_from_slice(&old[cursor..]);
    // A move is reported as a deletion plus an `is_move` insertion of the same row.
    removed.retain(|r| !moved.contains(r));
    let fresh: HashSet<_> = inserted.iter().cloned().collect();
    let mut ops = vec![];
    let mut exact = true;
    for r in &removed {
        match state.id_of.get(r) {
            Some(id) => ops.push(PatchOp::DeleteRow { path: path.clone(), id: id.clone() }),
            None => exact = false,
        }
    }
    let gone: HashSet<_> = removed.iter().cloned().collect();
    let mut sim: Vec<ContainerID> = state.order.iter().filter(|c| !gone.contains(c)).cloned().collect();
    // Place each changed row right after its final predecessor, in final order. Every row
    // then directly follows its final predecessor, so `sim == next`.
    for (f, c) in next.iter().enumerate() {
        let is_move = moved.contains(c);
        if !is_move && !fresh.contains(c) {
            continue;
        }
        if is_move {
            match sim.iter().position(|x| x == c) {
                Some(j) => {
                    sim.remove(j);
                }
                None => {
                    exact = false;
                    break;
                }
            }
        }
        let at = if f == 0 {
            0
        } else {
            match sim.iter().position(|x| *x == next[f - 1]) {
                Some(p) => p + 1,
                None => {
                    exact = false;
                    break;
                }
            }
        };
        sim.insert(at, c.clone());
        if is_move {
            let Some(id) = state.id_of.get(c) else {
                exact = false;
                break;
            };
            ops.push(PatchOp::MoveRow { path: path.clone(), id: id.clone(), index: at });
        } else {
            ops.push(PatchOp::InsertRow {
                path: path.clone(),
                index: at,
                value: project(Some(item_node), deep(doc, c)),
            });
        }
    }
    let exact = exact && sim == next;
    out.detached |= !removed.is_empty() && holds_collections(item_node);
    for r in &removed {
        state.drop_row(r);
    }
    for c in &inserted {
        state.add(doc, c.clone());
    }
    state.order = next;
    if exact {
        out.ops.extend(ops);
    } else {
        out.fallback.push(e.target.clone());
    }
}

/// Replaces every op inside a fallback container with one exact `set` of it.
fn finish_fallbacks(doc: &LoroDoc, schema: &Node, lists: &HashMap<ContainerID, ListState>, out: &mut Out) {
    let mut resolved: Vec<(Vec<Segment>, ContainerID, Option<&Node>)> = vec![];
    let mut work = std::mem::take(&mut out.fallback);
    while let Some(cid) = work.pop() {
        if resolved.iter().any(|(_, c, _)| *c == cid) {
            continue;
        }
        // A detached (deleted) container has no path and nothing left to publish.
        let Some(path) = doc.get_path_to_container(&cid) else {
            continue;
        };
        match json_path(lists, &path) {
            Ok(p) => resolved.push((p, cid, node_at(schema, &path))),
            Err(ancestor) => work.push(ancestor),
        }
    }
    let outer: Vec<_> = resolved
        .iter()
        .filter(|(p, _, _)| !resolved.iter().any(|(q, _, _)| q.len() < p.len() && p[..q.len()] == q[..]))
        .collect();
    out.ops.retain(|op| !outer.iter().any(|(p, _, _)| op.path().starts_with(p)));
    for (p, cid, node) in outer {
        out.ops.push(PatchOp::Set { path: p.clone(), value: project(*node, deep(doc, cid)) });
    }
}
