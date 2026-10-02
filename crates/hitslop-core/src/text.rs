//! Stateless text editing. The page reports that a field went from `from` (its text at
//! `base`) to `to`; the owner computes the edit script and merges it. No draft state
//! survives a request.
use super::*;
use loro::{cursor::Side, event::Diff, TextDelta, UpdateOptions};

/// Bounds the diff; past it the script falls back to a single caret-hinted splice.
const SCRIPT_TIMEOUT_MS: f64 = 50.0;

/// A branch at `base` for authoring one concurrent edit: the state there with minimal
/// history, so trimmed documents branch too (Loro has no `fork_at` for them). Safe where a
/// long-lived replica is not (see `replica_at`): it lives for one edit, authors only text
/// operations after checking its text equals `from`, and never imports later changes.
fn branch_at(doc: &LoroDoc, base: &Frontiers) -> Result<LoroDoc> {
    let branch = LoroDoc::new();
    branch.import(&doc.export(ExportMode::state_only(Some(base))).map_err(engine)?).map_err(engine)?;
    Ok(branch)
}
fn text_at(
    doc: &LoroDoc,
    schema: &Node,
    path: &[Segment],
    lists: &HashMap<ContainerID, ListState>,
) -> Result<LoroText> {
    let loc = resolve(doc, schema, path, &Rows::new(lists))?;
    match (unwrap_optional(&loc.node), loc.value) {
        (Node::Text {}, ValueOrContainer::Container(Container::Text(text))) => Ok(text),
        _ => Err(err(Code::TypeMismatch, "Expected text")),
    }
}
/// The Unicode scalar offset of a UTF-16 offset, refusing one inside a surrogate pair.
fn unicode_offset(text: &str, utf16: usize) -> Result<usize> {
    let mut n = 0;
    for (i, c) in text.chars().enumerate() {
        if n == utf16 {
            return Ok(i);
        }
        n += c.len_utf16();
    }
    if n == utf16 {
        Ok(text.chars().count())
    } else {
        Err(err(
            Code::OutOfRange,
            "Selection splits a surrogate or exceeds text",
        ))
    }
}
fn utf16_offset(text: &str, unicode: usize) -> usize {
    text.chars().take(unicode).map(char::len_utf16).sum()
}

/// The edits that turn `from` into `to`, in Unicode scalars. The common prefix (it stops
/// at the caret, so typing a repeated character inserts where the user typed it) and the
/// common suffix are retained; only the changed window between them is diffed.
pub(crate) fn script(from: &str, to: &str, caret: usize) -> Vec<TextDelta> {
    let (a, b): (Vec<char>, Vec<char>) = (from.chars().collect(), to.chars().collect());
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count().min(caret);
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (old, new) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let mut delta = vec![];
    if prefix > 0 {
        delta.push(TextDelta::Retain { retain: prefix, attributes: None });
    }
    // A pure insertion or deletion (nearly every keystroke) is already exact.
    match (old.is_empty(), new.is_empty()) {
        (true, true) => {}
        (false, true) => delta.push(TextDelta::Delete { delete: old.len() }),
        (true, false) => delta.push(TextDelta::Insert { insert: new.iter().collect(), attributes: None }),
        (false, false) => match window(old, new) {
            Some(hunks) => delta.extend(hunks),
            None => {
                delta.push(TextDelta::Delete { delete: old.len() });
                delta.push(TextDelta::Insert { insert: new.iter().collect(), attributes: None });
            }
        },
    }
    delta
}
/// Several hunks for one changed window, so disjoint edits and repeated characters do not
/// become one delete-and-reinsert that merges badly with concurrent edits. Computed on a
/// throwaway document, because `LoroText::update` mutates while it diffs and a timeout
/// would leave a partial edit.
fn window(old: &[char], new: &[char]) -> Option<Vec<TextDelta>> {
    let scratch = LoroDoc::new();
    let text = scratch.get_text("t");
    text.insert(0, &old.iter().collect::<String>()).ok()?;
    scratch.commit();
    let start = scratch.oplog_frontiers();
    text.update(&new.iter().collect::<String>(), UpdateOptions { timeout_ms: Some(SCRIPT_TIMEOUT_MS), use_refined_diff: false })
        .ok()?;
    scratch.commit();
    let batch = scratch.diff(&start, &scratch.oplog_frontiers()).ok()?;
    let delta = batch.iter().find_map(|(_, diff)| match diff {
        Diff::Text(delta) => Some(delta.clone()),
        _ => None,
    })?;
    // Proof, not trust: the script must reproduce `new` exactly from `old`.
    let mut replay = Vec::with_capacity(new.len());
    let mut cursor = 0;
    for item in &delta {
        match item {
            TextDelta::Retain { retain, .. } => {
                replay.extend_from_slice(old.get(cursor..cursor + retain)?);
                cursor += retain;
            }
            TextDelta::Delete { delete } => cursor += delete,
            TextDelta::Insert { insert, .. } => replay.extend(insert.chars()),
        }
    }
    replay.extend_from_slice(old.get(cursor..)?);
    (replay == new).then_some(delta)
}

impl Document {
    pub fn edit_text(&mut self, request: &str) -> Result<TextEdit> {
        let before = self.doc.state_frontiers();
        match self.edit_text_inner(request, &before) {
            Ok(edit) => Ok(edit),
            Err(error) => {
                self.abort(&before)?;
                Err(error)
            }
        }
    }
    fn edit_text_inner(&mut self, request: &str, before: &Frontiers) -> Result<TextEdit> {
        let r: wire::EditText = parse(request)?;
        // Every path validates the base first: an unknown operation must never reach Loro.
        let (base_at, base_vv) = decode_version(&self.doc, &r.base)?;
        // An unset optional text reads as "": the first edit from "" creates it.
        let at = resolve(&self.doc, &self.schema, &r.path, &Rows::new(&self.lists))?;
        if at.absent && matches!(unwrap_optional(&at.node), Node::Text {}) {
            if !r.from.is_empty() {
                return Err(err(Code::PathNotFound, "Text was cleared"));
            }
            unicode_offset(&r.to, r.selectionStart)?;
            unicode_offset(&r.to, r.selectionEnd)?;
            if r.to.is_empty() {
                return Ok(TextEdit {
                    sequence: self.sequence,
                    authored: r.base,
                    selection_start: r.selectionStart,
                    selection_end: r.selectionEnd,
                    publication: None,
                });
            }
            let (map, key) = at.parent.ok_or_else(|| err(Code::TypeMismatch, "Expected a field"))?;
            put(&map, &key, &Node::Text {}, &json!(r.to), &writer(&self.doc))?;
            self.doc.commit();
            let publication = self.publish()?;
            if publication.is_some() {
                self.record_typing(before.clone(), &r.path, &r.from, &r.to, r.selectionEnd);
            }
            return Ok(TextEdit {
                sequence: self.sequence,
                authored: self.version(),
                selection_start: r.selectionStart,
                selection_end: r.selectionEnd,
                publication,
            });
        }
        let ValueOrContainer::Container(Container::Text(current)) = at.value else {
            return Err(err(Code::TypeMismatch, "Expected text"));
        };
        if !matches!(unwrap_optional(at.node), Node::Text {}) {
            return Err(err(Code::TypeMismatch, "Expected text"));
        }
        // The field must be the same container the page edited: a row removed and
        // reinserted with the same `$id` has a new text that `base` never saw.
        if let ContainerID::Normal { peer, counter, .. } = current.id() {
            if !base_vv.includes_id(ID::new(peer, counter)) {
                return Err(err(Code::PathNotFound, "Text identity changed"));
            }
        }
        let selection = [
            unicode_offset(&r.to, r.selectionStart)?,
            unicode_offset(&r.to, r.selectionEnd)?,
        ];
        if r.from == r.to {
            return Ok(TextEdit {
                sequence: self.sequence,
                authored: r.base,
                selection_start: r.selectionStart,
                selection_end: r.selectionEnd,
                publication: None,
            });
        }
        let delta = script(&r.from, &r.to, selection[1]);
        let concurrent = current.to_string() != r.from;
        let (authored, positions) = if !concurrent {
            // Nearly every keystroke: nothing else changed this field since `base`.
            current.apply_delta(&delta).map_err(engine)?;
            self.doc.commit();
            (self.version(), [r.selectionStart, r.selectionEnd])
        } else {
            // This field changed concurrently: author the edit on a branch at `base` and
            // let Loro merge it, so neither side's characters are lost.
            if !base_vv.includes_vv(&self.floor) {
                return Err(err(Code::StaleBase, "Version precedes the saved history"));
            }
            // Although merged as an import, this is the person's edit. Its step starts
            // at the owner's current version, not the page's older base.
            let branch = branch_at(&self.doc, &base_at)?;
            let text = text_at(&branch, &self.schema, &r.path, &HashMap::new())?;
            if text.id() != current.id() {
                return Err(err(Code::PathNotFound, "Text identity changed"));
            }
            if text.to_string() != r.from {
                return Err(err(Code::StaleBase, "The field was not `from` at `base`"));
            }
            text.apply_delta(&delta).map_err(engine)?;
            branch.commit();
            let cursors = selection.map(|offset| {
                text.get_cursor(offset, Side::Middle)
                    .ok_or_else(|| err(Code::OutOfRange, "Cannot anchor selection"))
            });
            let [start, end] = cursors;
            let cursors = [start?, end?];
            self.doc
                .import(&branch.export(ExportMode::updates(&base_vv)).map_err(engine)?)
                .map_err(|e| err(Code::InvalidBytes, e))?;
            let merged = current.to_string();
            let mut positions = [0usize; 2];
            for (slot, cursor) in positions.iter_mut().zip(cursors) {
                let pos = self.doc.get_cursor_pos(&cursor).map_err(engine)?.current.pos;
                *slot = utf16_offset(&merged, pos);
            }
            (version_token(&branch.oplog_frontiers()), positions)
        };
        let publication = self.publish()?;
        if publication.is_some() {
            if concurrent {
                // A concurrent merge ends the run and is its own undo step.
                self.record(before.clone(), None, false);
            } else {
                self.record_typing(before.clone(), &r.path, &r.from, &r.to, r.selectionEnd);
            }
        }
        Ok(TextEdit {
            sequence: self.sequence,
            authored,
            selection_start: positions[0],
            selection_end: positions[1],
            publication,
        })
    }
}
