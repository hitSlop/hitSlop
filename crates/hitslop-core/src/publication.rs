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

#[derive(Clone, Debug)]
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

/// Each row list's rows in application order as of the last publication, by the list's
/// own map container.
pub(crate) type Lists = HashMap<ContainerID, Vec<String>>;

/// Every row list in the document, as it reads now. O(document), open only.
pub(super) fn index_all(doc: &LoroDoc, schema: &Node) -> Lists {
    let mut out = Lists::new();
    index(schema, &ValueOrContainer::Container(Container::Map(doc.get_map("data"))), &mut out);
    out
}
/// Records every row list inside `value`, which `node` describes.
fn index(node: &Node, value: &ValueOrContainer, lists: &mut Lists) {
    match (unwrap_optional(node), value) {
        (Node::Object { properties }, ValueOrContainer::Container(Container::Map(map))) => {
            for (key, child) in properties {
                if let Some(value) = map.get(key) {
                    index(child, &value, lists);
                }
            }
        }
        (Node::Record { value: entry }, ValueOrContainer::Container(Container::Map(map))) => {
            map.for_each(|_, value| index(entry, &value, lists));
        }
        (Node::List { item }, value) if !is_scalar(item) => {
            if let Ok(list) = RowList::of(value) {
                let ids = list.ids();
                for id in &ids {
                    if let Some(row) = list.row(id) {
                        index(item, &ValueOrContainer::Container(Container::Map(row)), lists);
                    }
                }
                lists.insert(list.map.id(), ids);
            }
        }
        _ => {}
    }
}

/// Where a changed container sits in the application's value.
enum Place<'s> {
    /// A field's own container (an object, record entry, row, text, counter or scalar list).
    Field { path: Vec<Segment>, node: &'s Node },
    /// A row list: its own map, its rows or its order.
    List { path: Vec<Segment>, node: &'s Node, list: ContainerID },
}
/// Places a Loro container path from the document root; `None` when the path does not
/// follow the layout.
fn place<'s>(schema: &'s Node, path: &[(ContainerID, Index)]) -> Option<Place<'s>> {
    enum In {
        Field,
        ListMap(ContainerID),
        Rows(ContainerID),
        Order(ContainerID),
    }
    let mut node = schema;
    let mut out = vec![];
    let mut at = In::Field;
    for (child, index) in path.iter().skip(1) {
        let Index::Key(key) = index else { return None };
        at = match at {
            In::Field => {
                node = match unwrap_optional(node) {
                    Node::Object { properties } => properties.get(key.as_str())?,
                    Node::Record { value } => value,
                    _ => return None,
                };
                out.push(Segment::Key(key.to_string()));
                match unwrap_optional(node) {
                    Node::List { item } if !is_scalar(item) => In::ListMap(child.clone()),
                    _ => In::Field,
                }
            }
            In::ListMap(list) => match key.as_str() {
                "rows" => In::Rows(list),
                "order" => In::Order(list),
                _ => return None,
            },
            In::Rows(_) => {
                node = match unwrap_optional(node) {
                    Node::List { item } => item,
                    _ => return None,
                };
                out.push(Segment::Id { id: key.to_string() });
                In::Field
            }
            In::Order(_) => return None,
        };
    }
    Some(match at {
        In::Field => Place::Field { path: out, node },
        In::ListMap(list) | In::Rows(list) | In::Order(list) => Place::List { path: out, node, list },
    })
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
    // Containers this change created or revived are published whole by the map update that
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
    events.retain(|e| {
        !e.path.iter().take(e.path.len() - 1).any(|(c, _)| fresh.contains(c)) && !fresh.contains(&e.target)
    });
    let mut ops = vec![];
    let mut touched: Vec<(Vec<Segment>, &Node, ContainerID)> = vec![];
    for e in &events {
        let Some(place) = place(schema, &e.path) else {
            return Ok(Some(everything(doc, schema, lists)));
        };
        match (place, &e.change) {
            (Place::List { path, node, list }, Change::Map(_) | Change::List) => {
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
                ops.push(PatchOp::Set { path, value: project(Some(node), deep(doc, &e.target)) });
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
                        None => ops.push(PatchOp::Remove { path }),
                        Some(s) => {
                            if let Slot::Container(c) = s
                                && let Some(value) = container(doc, c)
                            {
                                index(declared, &value, lists);
                            }
                            ops.push(PatchOp::Set { path, value: project(Some(declared), materialize(doc, s)) });
                        }
                    }
                }
            }
            _ => return Ok(Some(everything(doc, schema, lists))),
        }
    }
    for (path, node, list) in touched {
        let Some(rows) = container(doc, &list).and_then(|value| RowList::of(&value).ok()) else {
            return Ok(Some(everything(doc, schema, lists)));
        };
        let Node::List { item } = unwrap_optional(node) else {
            return Ok(Some(everything(doc, schema, lists)));
        };
        let new = rows.ids();
        let Some(old) = lists.get(&list).cloned() else {
            index(node, &ValueOrContainer::Container(Container::Map(rows.map.clone())), lists);
            ops.push(PatchOp::Set { path, value: project(Some(node), deep(doc, &list)) });
            continue;
        };
        let (old_set, new_set): (HashSet<&String>, HashSet<&String>) = (old.iter().collect(), new.iter().collect());
        // Rows that appear or go are published whole, or as one deletion: drop any field
        // ops this change made inside them.
        let changed: HashSet<&String> = old_set.symmetric_difference(&new_set).copied().collect();
        ops.retain(|op| {
            !op.path().get(path.len()).is_some_and(|segment| {
                op.path().starts_with(&path) && matches!(segment, Segment::Id { id } if changed.contains(id))
            })
        });
        let Some(diff) = diff(&path, &old, &new, |id| {
            let row = rows.row(id)?;
            index(item, &ValueOrContainer::Container(Container::Map(row.clone())), lists);
            let mut value = project(Some(item), deep(doc, &row.id()));
            if let Value::Object(fields) = &mut value {
                fields.insert("$id".into(), Value::String(id.to_owned()));
            }
            Some(value)
        }) else {
            ops.retain(|op| !op.path().starts_with(&path));
            ops.push(PatchOp::Set { path, value: project(Some(node), deep(doc, &list)) });
            lists.insert(list, new);
            continue;
        };
        ops.extend(diff);
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
    vec![PatchOp::Set { path: vec![], value: project(Some(schema), json(doc.get_map("data").get_deep_value())) }]
}
