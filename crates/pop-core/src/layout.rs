//! The document layout uses mergeable containers. Deletable containers live in stable
//! slots whose current generation selects a payload; clearing advances the generation
//! without deleting history, so stale edits stay hidden and undo can restore the old payload.
use super::*;
use loro::LoroCounter;
use std::collections::HashSet;

/// The two children of a row list's map.
const ROWS: &str = "rows";
const ORDER: &str = "order";

/// Gets a present child or creates the deterministic child shared by concurrent writers.
/// Reads never call this helper.
pub(crate) fn ensure_child<C: ContainerTrait + Default>(parent: &LoroMap, key: &str) -> Result<C> {
    let container = match parent.get(key) {
        Some(ValueOrContainer::Container(container)) => container,
        Some(_) => return Err(err(Code::TypeMismatch, "Expected container")),
        None => match C::default().to_container() {
            Container::Map(_) => parent.ensure_mergeable_map(key).map(Container::Map),
            Container::Text(_) => parent.ensure_mergeable_text(key).map(Container::Text),
            Container::Counter(_) => parent.ensure_mergeable_counter(key).map(Container::Counter),
            Container::MovableList(_) => parent.ensure_mergeable_movable_list(key).map(Container::MovableList),
            _ => return Err(engine("Unsupported semantic container")),
        }
        .map_err(engine)?,
    };
    C::try_from_container(container).ok_or_else(|| err(Code::TypeMismatch, "Expected container kind"))
}

pub(crate) const CURRENT: &str = "current";
pub(crate) const INITIAL: &str = "initial";

pub(crate) fn slotted(node: &Node, entry: bool) -> bool {
    matches!(node, Node::Optional { inner, .. } if !is_scalar(inner)) || (entry && matches!(node, Node::Object { .. }))
}
pub(crate) fn valid_generation(key: &str) -> bool {
    key == INITIAL || key.strip_prefix("g:").is_some_and(valid_id)
}
pub(crate) fn current_generation(selector: Option<&str>) -> &str {
    selector.unwrap_or(INITIAL)
}
pub(crate) fn generation(slot: &LoroMap) -> Result<String> {
    match slot.get(CURRENT) {
        None => Ok(current_generation(None).to_owned()),
        Some(ValueOrContainer::Value(LoroValue::String(current))) if valid_generation(&current) => {
            Ok(current_generation(Some(&current)).to_owned())
        }
        _ => Err(err(Code::InvalidBytes, "Invalid slot generation")),
    }
}
/// The selected generation's parent and key. Creation never rewrites the selector.
pub(crate) fn slot_parent(parent: &LoroMap, key: &str) -> Result<(LoroMap, String)> {
    let slot = ensure_child(parent, key)?;
    let current = generation(&slot)?;
    Ok((slot, current))
}
pub(crate) fn slot_value(value: &ValueOrContainer) -> Option<ValueOrContainer> {
    let ValueOrContainer::Container(Container::Map(slot)) = value else { return None };
    slot.get(&generation(slot).ok()?)
}
pub(crate) fn field(map: &LoroMap, key: &str, node: &Node, entry: bool) -> Option<ValueOrContainer> {
    let value = map.get(key)?;
    if slotted(node, entry) { slot_value(&value) } else { Some(value) }
}
pub(crate) fn clear_slot(map: &LoroMap, key: &str) -> Result<()> {
    let (slot, _) = slot_parent(map, key)?;
    slot.insert(CURRENT, format!("g:{}", application_id())).map_err(engine)
}
pub(crate) fn clear_field(map: &LoroMap, key: &str, node: &Node, entry: bool) -> Result<()> {
    if field(map, key, node, entry).is_none() {
        return Ok(());
    }
    if slotted(node, entry) { clear_slot(map, key) } else { map.delete(key).map_err(engine) }
}

/// A list of rows: `rows` maps each `$id` to its generation slot; `order` lists the IDs.
/// Merged edits can leave an ID in `order` twice (two restores of one row) or name a row
/// that is gone, and a row can be missing from `order`; `ids` reads every such state.
pub(crate) struct RowList {
    pub(crate) map: LoroMap,
    rows: LoroMap,
    order: LoroMovableList,
}
impl RowList {
    /// The row list stored in `value`, which holds the list field.
    pub(crate) fn of(value: &ValueOrContainer) -> Result<Self> {
        let ValueOrContainer::Container(Container::Map(map)) = value else {
            return Err(err(Code::TypeMismatch, "Expected list"));
        };
        Self::in_map(map).ok_or_else(|| err(Code::TypeMismatch, "Expected list"))
    }
    pub(crate) fn in_map(map: &LoroMap) -> Option<Self> {
        let rows = match map.get(ROWS) {
            Some(ValueOrContainer::Container(Container::Map(rows))) => rows,
            _ => return None,
        };
        let order = match map.get(ORDER) {
            Some(ValueOrContainer::Container(Container::MovableList(order))) => order,
            _ => return None,
        };
        Some(Self { map: map.clone(), rows, order })
    }
    /// The row list at `parent[key]`, created when absent.
    pub(crate) fn ensure(parent: &LoroMap, key: &str) -> Result<Self> {
        let map = ensure_child(parent, key)?;
        let rows = ensure_child(&map, ROWS)?;
        let order = ensure_child(&map, ORDER)?;
        Ok(Self { map, rows, order })
    }
    /// The rows as the application sees them (`visible`).
    pub(crate) fn ids(&self) -> Vec<String> {
        visible(
            self.order_ids(),
            |id| self.row(id).is_some(),
            self.rows.keys().filter(|k| self.row(k).is_some()).map(|k| k.to_string()),
        )
    }
    fn order_ids(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.order.len());
        self.order.for_each(|entry| {
            out.push(match entry {
                ValueOrContainer::Value(LoroValue::String(id)) => id.to_string(),
                _ => String::new(),
            })
        });
        out
    }
    pub(crate) fn row(&self, id: &str) -> Option<LoroMap> {
        match self.rows.get(id).as_ref().and_then(slot_value) {
            Some(ValueOrContainer::Container(Container::Map(row))) => Some(row),
            _ => None,
        }
    }
    /// The map of rows by `$id`.
    pub(crate) fn rows(&self) -> &LoroMap {
        &self.rows
    }
    /// The order, as stored: IDs, possibly repeated or naming removed rows.
    pub(crate) fn order(&self) -> &LoroMovableList {
        &self.order
    }
    /// Where `id` first appears in `order`.
    fn position(&self, id: &str) -> Option<usize> {
        self.order_ids().iter().position(|x| x == id)
    }
    /// Where an insertion lands in `order`: at the end, or beside a row.
    pub(crate) fn anchor(&self, anchor: &Option<Anchor>) -> Result<usize> {
        let absent = || err(Code::PathNotFound, "Row is absent");
        match anchor {
            None => Ok(self.order.len()),
            Some(Anchor::Before { before }) => {
                self.row(before).ok_or_else(absent)?;
                Ok(self.position(before).unwrap_or(self.order.len()))
            }
            Some(Anchor::After { after }) => {
                self.row(after).ok_or_else(absent)?;
                Ok(self.position(after).map_or(self.order.len(), |at| at + 1))
            }
        }
    }
    /// Inserts a validated row with `id` at `at` in `order`.
    pub(crate) fn insert(&self, at: usize, id: &str, item: &Node, value: &Value) -> Result<()> {
        let (slot, generation) = slot_parent(&self.rows, id)?;
        let row = ensure_child(&slot, &generation)?;
        execute::fill(&row, item, value)?;
        self.order.insert(at, id).map_err(engine)
    }
    /// Hides a row behind a fresh generation and removes every place `order` names it.
    pub(crate) fn remove(&self, id: &str) -> Result<()> {
        self.remove_many(&[id])
    }
    /// Removes a replacement's omitted rows with one traversal of the stored order,
    /// including duplicate occurrences left by concurrent restores.
    pub(crate) fn remove_many(&self, ids: &[&str]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        for id in ids {
            clear_slot(&self.rows, id)?;
        }
        let removed: HashSet<&str> = ids.iter().copied().collect();
        let places: Vec<usize> = self
            .order_ids()
            .iter()
            .enumerate()
            .filter(|(_, id)| removed.contains(id.as_str()))
            .map(|(at, _)| at)
            .collect();
        for at in places.into_iter().rev() {
            self.order.delete(at, 1).map_err(engine)?;
        }
        Ok(())
    }
    /// Deletes every place `order` names `id` from the `keep`-th on.
    fn drop_order(&self, id: &str, keep: usize) -> Result<()> {
        let places: Vec<usize> =
            self.order_ids().iter().enumerate().filter(|(_, x)| *x == id).map(|(i, _)| i).skip(keep).collect();
        for at in places.into_iter().rev() {
            self.order.delete(at, 1).map_err(engine)?;
        }
        Ok(())
    }
    /// Moves a row to an anchored place: its first place in `order` moves, later
    /// duplicates go. A row missing from `order` is placed there.
    pub(crate) fn mov(&self, id: &str, anchor: &Option<Anchor>) -> Result<()> {
        self.drop_order(id, 1)?;
        let mut to = self.anchor(anchor)?;
        match self.position(id) {
            Some(from) => {
                if to > from {
                    to -= 1;
                }
                if from != to {
                    self.order.mov(from, to).map_err(engine)?;
                }
                Ok(())
            }
            None => self.order.insert(to, id).map_err(engine),
        }
    }
    /// Makes `order` name exactly the rows as `ids` lists them, once each.
    pub(crate) fn normalize(&self) -> Result<()> {
        let visible = self.ids();
        let raw = self.order_ids();
        if raw == visible {
            return Ok(());
        }
        let mut seen = HashSet::new();
        let mut remove = Vec::new();
        for (at, id) in raw.iter().enumerate() {
            if self.row(id).is_none() || !seen.insert(id.as_str()) {
                remove.push(at);
            }
        }
        for at in remove.into_iter().rev() {
            self.order.delete(at, 1).map_err(engine)?;
        }
        for id in visible.iter().filter(|id| !seen.contains(id.as_str())) {
            self.order.push(id.as_str()).map_err(engine)?;
        }
        Ok(())
    }
    pub(crate) fn mov_index(&self, from: usize, to: usize) -> Result<()> {
        self.order.mov(from, to).map_err(engine)
    }
    /// Removes every row.
    pub(crate) fn clear(&self) -> Result<()> {
        for id in self.ids() {
            clear_slot(&self.rows, &id)?;
        }
        if !self.order.is_empty() {
            self.order.delete(0, self.order.len()).map_err(engine)?;
        }
        Ok(())
    }
}

/// A row list's application order, the one rule every reading of one follows: each row in
/// `order` once, at its first place, then the rows `order` misses, by ID. Merged edits can
/// leave an ID in `order` twice, name a row that is gone, or miss one.
pub(crate) fn visible(
    order: impl IntoIterator<Item = String>,
    has: impl Fn(&str) -> bool,
    rows: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for id in order {
        if has(&id) && seen.insert(id.clone()) {
            out.push(id);
        }
    }
    let mut missing: Vec<String> = rows.into_iter().filter(|id| !seen.contains(id)).collect();
    missing.sort();
    out.extend(missing);
    out
}

/// A counter's total as the application reads it: Loro sums increments as f64, exact for
/// any realistic count; a merged total past the safe range reads saturated.
pub(crate) fn counter_total(total: f64) -> i64 {
    total.round().clamp(-(MAX_SAFE as f64), MAX_SAFE as f64) as i64
}
pub(crate) fn counter_value(counter: &LoroCounter) -> i64 {
    counter_total(counter.get_value())
}
/// Sets a counter to `value`, as an increment by the difference.
pub(crate) fn set_counter(counter: &LoroCounter, value: i64) -> Result<()> {
    let by = value - counter_value(counter);
    if by != 0 {
        counter.increment(by as f64).map_err(engine)?;
    }
    Ok(())
}
/// Sets a text to `value` with a minimal edit script.
pub(crate) fn set_text(text: &LoroText, value: &str) -> Result<()> {
    let delta = text::script(&text.to_string(), value, value.chars().count());
    if !delta.is_empty() {
        text.apply_delta(&delta).map_err(engine)?;
    }
    Ok(())
}

/// Indexes into `values` of a longest strictly increasing subsequence.
pub(crate) fn longest_increasing(values: &[usize]) -> Vec<usize> {
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
