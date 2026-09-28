//! Change-proportional publications driven by Loro events.
//!
//! Loro reports, after every commit or import, which containers changed and a typed
//! diff for each. Publications are built from those diffs plus persistent per-list
//! row indexes; neither the before nor the after document is materialized. Values
//! are materialized only for changed fields and inserted rows. Anomalous lists
//! (missing, invalid or duplicate row IDs, plain values) fall back to an exact `set`
//! of that list; stored data is never repaired.
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
    Text,
    Other,
}
#[derive(Debug)]
pub(crate) struct Event {
    target: ContainerID,
    path: Vec<(ContainerID, Index)>,
    change: Change,
}
pub(crate) type Events = Arc<Mutex<Vec<Event>>>;

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
        let mut out = sink.lock().unwrap();
        for c in e.events {
            let change = match &c.diff {
                Diff::Map(m) => Change::Map(
                    m.updated
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.as_ref().map(slot)))
                        .collect(),
                ),
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
                Diff::Text(_) => Change::Text,
                _ => Change::Other,
            };
            out.push(Event {
                target: c.target.clone(),
                path: c.path.to_vec(),
                change,
            });
        }
    }))
}

/// Order and row identities of one movable list, as of the last publication.
#[derive(Default, Debug)]
pub(crate) struct ListState {
    pub(crate) order: Vec<Option<ContainerID>>,
    id_of: HashMap<ContainerID, Option<String>>,
    pub(crate) by_id: HashMap<String, Vec<ContainerID>>,
    /// Entries without a valid row ID: plain values, non-map containers, invalid IDs.
    missing: usize,
    /// Entries sharing an ID with an earlier entry.
    extra: usize,
}
impl ListState {
    fn row_id(doc: &LoroDoc, cid: &ContainerID) -> Option<String> {
        if cid.container_type() != ContainerType::Map {
            return None;
        }
        match doc.get_map(cid.clone()).get("$id") {
            Some(ValueOrContainer::Value(LoroValue::String(s))) if valid_id(&s) => {
                Some(s.to_string())
            }
            _ => None,
        }
    }
    fn add(&mut self, cid: ContainerID, id: Option<String>) {
        match &id {
            Some(id) => {
                let rows = self.by_id.entry(id.clone()).or_default();
                if !rows.is_empty() {
                    self.extra += 1;
                }
                rows.push(cid.clone());
            }
            None => self.missing += 1,
        }
        self.id_of.insert(cid, id);
    }
    fn drop_row(&mut self, cid: &ContainerID) {
        match self.id_of.remove(cid) {
            Some(Some(id)) => {
                let rows = self.by_id.get_mut(&id).expect("indexed row");
                rows.retain(|c| c != cid);
                if rows.is_empty() {
                    self.by_id.remove(&id);
                } else {
                    self.extra -= 1;
                }
            }
            Some(None) => self.missing -= 1,
            None => {}
        }
    }
    pub(crate) fn clean(&self) -> bool {
        self.missing == 0 && self.extra == 0
    }
    /// The row's ID if it identifies exactly this row.
    fn unique_id(&self, cid: &ContainerID) -> Option<&str> {
        let id = self.id_of.get(cid)?.as_deref()?;
        (self.by_id.get(id)?.len() == 1).then_some(id)
    }
    fn build(doc: &LoroDoc, list: &LoroMovableList) -> Self {
        let mut state = Self::default();
        state.order.reserve(list.len());
        list.for_each(|v| match v {
            ValueOrContainer::Container(c) => {
                let cid = c.id();
                state.order.push(Some(cid.clone()));
                let id = Self::row_id(doc, &cid);
                state.add(cid, id);
            }
            ValueOrContainer::Value(_) => {
                state.order.push(None);
                state.missing += 1;
            }
        });
        state
    }
}
/// Indexes every movable list reachable from the document root. O(document), open only.
pub(super) fn index_all(doc: &LoroDoc) -> HashMap<ContainerID, ListState> {
    let mut out = HashMap::new();
    walk(doc, &Container::Map(doc.get_map("data")), &mut out);
    out
}
fn walk(doc: &LoroDoc, container: &Container, out: &mut HashMap<ContainerID, ListState>) {
    let mut children = vec![];
    match container {
        Container::Map(map) => map.for_each(|_, v| {
            if let ValueOrContainer::Container(c) = v {
                children.push(c);
            }
        }),
        Container::MovableList(list) => {
            out.insert(list.id(), ListState::build(doc, list));
            list.for_each(|v| {
                if let ValueOrContainer::Container(c) = v {
                    children.push(c);
                }
            });
        }
        Container::List(list) => list.for_each(|v| {
            if let ValueOrContainer::Container(c) = v {
                children.push(c);
            }
        }),
        _ => {}
    }
    for child in &children {
        walk(doc, child, out);
    }
}

fn node_at<'s>(schema: &'s Node, path: &[(ContainerID, Index)]) -> Option<&'s Node> {
    let mut node = schema;
    for (_, index) in path.iter().skip(1) {
        node = match (node, index) {
            (Node::Object { properties }, Index::Key(key)) => properties.get(&key.to_string())?,
            (Node::List { item }, Index::Seq(_)) => item,
            _ => return None,
        };
    }
    Some(node)
}
/// Converts a Loro container path to a publication path. Rows are addressed by
/// `$id`; a row without a unique ID makes its list the fallback container.
fn json_path(
    lists: &HashMap<ContainerID, ListState>,
    path: &[(ContainerID, Index)],
) -> std::result::Result<Vec<Value>, ContainerID> {
    let mut out = Vec::with_capacity(path.len());
    for i in 1..path.len() {
        match &path[i].1 {
            Index::Key(key) => out.push(json!(key.to_string())),
            Index::Seq(_) => {
                let list = &path[i - 1].0;
                match lists.get(list).and_then(|s| s.unique_id(&path[i].0)) {
                    Some(id) => out.push(json!({ "id": id })),
                    None => return Err(list.clone()),
                }
            }
            Index::Node(_) => return Err(path[i - 1].0.clone()),
        }
    }
    Ok(out)
}
fn deep(doc: &LoroDoc, cid: &ContainerID) -> Result<Value> {
    let value = doc
        .get_container(cid.clone())
        .map(|c| ValueOrContainer::Container(c).get_deep_value())
        .unwrap_or(LoroValue::Null);
    serde_json::to_value(value).map_err(engine)
}
fn materialize(doc: &LoroDoc, slot: &Slot) -> Result<Value> {
    match slot {
        Slot::Value(v) => serde_json::to_value(v).map_err(engine),
        Slot::Container(c) => deep(doc, c),
    }
}
fn clean_value(node: &Node, value: &Value) -> bool {
    let mut found = vec![];
    issues(node, value, &mut vec![], &mut found);
    found.is_empty()
}

pub(crate) struct Published {
    pub ops: Vec<Value>,
    /// The incremental checks could not prove the document still has no issues.
    pub rescan: bool,
}
struct Out {
    ops: Vec<Value>,
    fallback: Vec<ContainerID>,
    rescan: bool,
}

pub(super) fn publish(
    doc: &LoroDoc,
    schema: &Node,
    lists: &mut HashMap<ContainerID, ListState>,
    mut events: Vec<Event>,
) -> Result<Published> {
    // Only the document root is projected; other Loro roots are not application data.
    let root = doc.get_map("data").id();
    events.retain(|e| e.path.first().is_some_and(|(c, _)| *c == root));
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
                for item in items {
                    if let Item::Insert(slots, false) = item {
                        for s in slots {
                            if let Slot::Container(c) = s {
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
    let mut out = Out {
        ops: vec![],
        fallback: vec![],
        rescan: false,
    };
    for e in &events {
        match &e.change {
            Change::Map(updates) => map_ops(doc, schema, lists, e, updates, &mut out)?,
            Change::Text => match json_path(lists, &e.path) {
                Ok(path) => {
                    if !matches!(node_at(schema, &e.path), Some(Node::Text) | None) {
                        out.rescan = true;
                    }
                    let value = doc.get_text(e.target.clone()).to_string();
                    out.ops.push(json!({"type":"set","path":path,"value":value}));
                }
                Err(c) => out.fallback.push(c),
            },
            Change::List(items) => list_ops(doc, schema, lists, e, items, &mut out)?,
            Change::Other => {
                out.rescan = true;
                match json_path(lists, &e.path) {
                    Ok(path) => {
                        let value = project_at(doc, node_at(schema, &e.path), &e.target)?;
                        out.ops.push(json!({"type":"set","path":path,"value":value}));
                    }
                    Err(c) => out.fallback.push(c),
                }
            }
        }
    }
    for cid in &fresh {
        if let Some(container) = doc.get_container(cid.clone()) {
            walk(doc, &container, lists);
        }
    }
    finish_fallbacks(doc, schema, lists, &mut out)?;
    Ok(Published {
        ops: out.ops,
        rescan: out.rescan,
    })
}

fn map_ops(
    doc: &LoroDoc,
    schema: &Node,
    lists: &mut HashMap<ContainerID, ListState>,
    e: &Event,
    updates: &[(String, Option<Slot>)],
    out: &mut Out,
) -> Result<()> {
    let node = node_at(schema, &e.path);
    // A counter's keys are writer contributions: publish the counter's new sum.
    if let Some(Node::Counter) = node {
        match json_path(lists, &e.path) {
            Ok(path) => {
                let raw = deep(doc, &e.target)?;
                if counter_sum(&raw).is_none() {
                    out.rescan = true;
                }
                out.ops.push(json!({"type":"set","path":path,"value":project(node, raw)}));
            }
            Err(c) => out.fallback.push(c),
        }
        return Ok(());
    }
    let row_of = match e.path.last() {
        Some((_, Index::Seq(_))) if e.path.len() >= 2 => Some(e.path[e.path.len() - 2].0.clone()),
        _ => None,
    };
    let mut sorted: Vec<_> = updates.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    // A row identity change re-keys the row: update the index, then publish the
    // whole list exactly, since consumers address rows by their previous ID.
    if let Some(list) = &row_of {
        if sorted.iter().any(|(k, _)| k == "$id") {
            if let Some(state) = lists.get_mut(list) {
                state.drop_row(&e.target);
                state.add(e.target.clone(), ListState::row_id(doc, &e.target));
            }
            out.fallback.push(list.clone());
            out.rescan = true;
            return Ok(());
        }
    }
    let base = match json_path(lists, &e.path) {
        Ok(path) => path,
        Err(c) => {
            out.fallback.push(c);
            return Ok(());
        }
    };
    let properties = match node {
        Some(Node::Object { properties }) => Some(properties),
        Some(_) => {
            out.rescan = true;
            None
        }
        None => None,
    };
    for (key, s) in sorted {
        let mut path = base.clone();
        path.push(json!(key));
        let declared = properties.and_then(|p| p.get(key));
        if declared.is_none() && key != "$id" { out.rescan = true; }
        match s {
            None => {
                if declared.is_some() {
                    out.rescan = true;
                }
                out.ops.push(json!({"type":"remove","path":path}));
            }
            Some(s) => {
                let raw = materialize(doc, s)?;
                if let Some(child) = declared {
                    if !out.rescan && !clean_value(child, &raw) {
                        out.rescan = true;
                    }
                }
                let value = match s { Slot::Container(cid) => project_at(doc, declared, cid)?, _ => project(declared, raw) };
                out.ops.push(json!({"type":"set","path":path,"value":value}));
            }
        }
    }
    Ok(())
}

fn list_ops(
    doc: &LoroDoc,
    schema: &Node,
    lists: &mut HashMap<ContainerID, ListState>,
    e: &Event,
    items: &[Item],
    out: &mut Out,
) -> Result<()> {
    let path = json_path(lists, &e.path);
    let item_node = match node_at(schema, &e.path) {
        Some(Node::List { item }) => Some(&**item),
        Some(_) => {
            out.rescan = true;
            None
        }
        None => None,
    };
    // Only movable lists carry row identity; any other list publishes exactly.
    if e.target.container_type() != ContainerType::MovableList {
        out.fallback.push(e.target.clone());
        out.rescan = true;
        return Ok(());
    }
    let Some(state) = lists.get_mut(&e.target) else {
        // Not indexed: cannot interpret the delta against a previous order.
        let list = doc.get_movable_list(e.target.clone());
        lists.insert(e.target.clone(), ListState::build(doc, &list));
        out.fallback.push(e.target.clone());
        out.rescan = true;
        return Ok(());
    };
    let was_clean = state.clean();
    let old = &state.order;
    let mut next = Vec::with_capacity(old.len() + 1);
    let mut removed = vec![];
    let mut moved = HashSet::new();
    let mut inserted = vec![];
    let mut plain = 0usize;
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
                    match s {
                        Slot::Container(c) => {
                            next.push(Some(c.clone()));
                            if *is_move {
                                moved.insert(c.clone());
                            } else {
                                inserted.push(c.clone());
                            }
                        }
                        Slot::Value(_) => {
                            next.push(None);
                            plain += 1;
                        }
                    }
                }
            }
        }
    }
    if !in_sync {
        *state = ListState::build(doc, &doc.get_movable_list(e.target.clone()));
        out.fallback.push(e.target.clone());
        out.rescan = true;
        return Ok(());
    }
    next.extend_from_slice(&old[cursor..]);
    // A move is reported as a deletion plus an `is_move` insertion of the same row.
    removed.retain(|r| r.as_ref().map_or(true, |c| !moved.contains(c)));
    let fresh: HashSet<_> = inserted.iter().cloned().collect();
    let mut ops = vec![];
    let mut exact = was_clean && plain == 0 && path.is_ok();
    let mut invalid_row = false;
    if exact {
        let path = path.as_ref().unwrap();
        let gone: HashSet<_> = removed.iter().flatten().cloned().collect();
        for r in removed.iter().flatten() {
            ops.push(json!({"type":"deleteRow","path":path,"id":state.unique_id(r)}));
        }
        let mut sim: Vec<Option<ContainerID>> = state
            .order
            .iter()
            .filter(|c| c.as_ref().map_or(true, |c| !gone.contains(c)))
            .cloned()
            .collect();
        // Place each changed row right after its final predecessor, in final order.
        // Every row then directly follows its final predecessor, so `sim == next`.
        for (f, entry) in next.iter().enumerate() {
            let Some(c) = entry else { continue };
            let is_move = moved.contains(c);
            if !is_move && !fresh.contains(c) {
                continue;
            }
            if is_move {
                match sim.iter().position(|x| x.as_ref() == Some(c)) {
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
                match sim.iter().position(|x| x == &next[f - 1]) {
                    Some(p) => p + 1,
                    None => {
                        exact = false;
                        break;
                    }
                }
            };
            sim.insert(at, entry.clone());
            if is_move {
                ops.push(json!({"type":"moveRow","path":path,"id":state.unique_id(c),"index":at}));
            } else {
                let raw = deep(doc, c)?;
                if let Some(item) = item_node {
                    if !clean_value(item, &raw) {
                        invalid_row = true;
                    }
                }
                let value = project_at(doc, item_node, c)?;
                ops.push(json!({"type":"insertRow","path":path,"index":at,"value":value}));
            }
        }
        exact = exact && sim == next;
    }
    for r in &removed {
        match r {
            Some(c) => state.drop_row(c),
            None => state.missing -= 1,
        }
    }
    state.missing += plain;
    for c in &inserted {
        state.add(c.clone(), ListState::row_id(doc, c));
    }
    state.order = next;
    if !state.clean() {
        exact = false;
    }
    if exact {
        if invalid_row {
            out.rescan = true;
        }
        out.ops.extend(ops);
    } else {
        out.fallback.push(match path {
            Err(ancestor) => ancestor,
            Ok(_) => e.target.clone(),
        });
        out.rescan = true;
    }
    Ok(())
}

/// Replaces every op inside a fallback container with one exact `set` of it.
fn finish_fallbacks(
    doc: &LoroDoc,
    schema: &Node,
    lists: &HashMap<ContainerID, ListState>,
    out: &mut Out,
) -> Result<()> {
    let mut resolved: Vec<(Vec<Value>, ContainerID, Option<&Node>)> = vec![];
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
    let prefix = |p: &[Value], op: &Value| {
        op["path"]
            .as_array()
            .is_some_and(|q| q.len() >= p.len() && q[..p.len()] == *p)
    };
    let outer: Vec<_> = resolved
        .iter()
        .filter(|(p, _, _)| {
            !resolved
                .iter()
                .any(|(q, _, _)| q.len() < p.len() && p[..q.len()] == q[..])
        })
        .collect();
    out.ops
        .retain(|op| !outer.iter().any(|(p, _, _)| prefix(p, op)));
    for (p, cid, node) in outer {
        let value = project_at(doc, *node, cid)?;
        out.ops.push(json!({"type":"set","path":p,"value":value}));
    }
    Ok(())
}
