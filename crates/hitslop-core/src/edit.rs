//! Stateless text editing. The page reports that a field went from `from` (its text at
//! `base`) to `to`; the owner computes the edit script and merges it. No draft state
//! survives a request.
use super::*;
use loro::{cursor::Side, event::Diff, TextDelta, UpdateOptions};

/// Bounds the diff; past it the script falls back to a single caret-hinted splice.
const SCRIPT_TIMEOUT_MS: f64 = 50.0;

fn text_at(
    doc: &LoroDoc,
    schema: &Node,
    path: &[Segment],
    lists: &HashMap<ContainerID, ListState>,
) -> Result<LoroText> {
    let loc = resolve(doc, schema, path, &Rows::new(lists))?;
    match (loc.node, loc.value) {
        (Node::Text, ValueOrContainer::Container(Container::Text(text))) => Ok(text),
        _ => Err(err("type_mismatch", "Expected text")),
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
            "out_of_range",
            "Selection splits a surrogate or exceeds text",
        ))
    }
}
fn utf16_offset(text: &str, unicode: usize) -> usize {
    text.chars().take(unicode).map(char::len_utf16).sum()
}

/// The edits that turn `from` into `to`, in Unicode scalars. Computed on a throwaway
/// document, because `LoroText::update` mutates while it diffs and a timeout would
/// leave a partial edit. Several hunks keep disjoint edits and repeated characters
/// from becoming one delete-and-reinsert that merges badly with concurrent edits.
pub(crate) fn script(from: &str, to: &str, caret: usize) -> Vec<TextDelta> {
    let diffed = (|| {
        let scratch = LoroDoc::new();
        let text = scratch.get_text("t");
        text.insert(0, from).ok()?;
        scratch.commit();
        let start = scratch.oplog_frontiers();
        text.update(to, UpdateOptions { timeout_ms: Some(SCRIPT_TIMEOUT_MS), use_refined_diff: false })
            .ok()?;
        scratch.commit();
        let batch = scratch.diff(&start, &scratch.oplog_frontiers()).ok()?;
        let delta = batch.iter().find_map(|(_, diff)| match diff {
            Diff::Text(delta) => Some(delta.clone()),
            _ => None,
        })?;
        // Proof, not trust: the script must reproduce `to` exactly from `from`.
        let check = LoroDoc::new();
        let replay = check.get_text("t");
        replay.insert(0, from).ok()?;
        replay.apply_delta(&delta).ok()?;
        (replay.to_string() == to).then_some(delta)
    })();
    diffed.unwrap_or_else(|| splice(from, to, caret))
}
/// One replacement between the common prefix and suffix. The prefix stops at the
/// caret, so typing a repeated character inserts where the user typed it.
fn splice(from: &str, to: &str, caret: usize) -> Vec<TextDelta> {
    let (a, b): (Vec<char>, Vec<char>) = (from.chars().collect(), to.chars().collect());
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count().min(caret);
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let mut delta = vec![];
    if prefix > 0 {
        delta.push(TextDelta::Retain { retain: prefix, attributes: None });
    }
    if a.len() - prefix - suffix > 0 {
        delta.push(TextDelta::Delete { delete: a.len() - prefix - suffix });
    }
    let insert: String = b[prefix..b.len() - suffix].iter().collect();
    if !insert.is_empty() {
        delta.push(TextDelta::Insert { insert, attributes: None });
    }
    delta
}

impl Document {
    pub fn edit_text(&mut self, request: &str) -> Result<TextEdit> {
        let r: wire::EditText = parse(request)?;
        // Every path validates the base first: an unknown operation must never reach Loro.
        let (base_at, base_vv) = decode_version(&self.doc, &r.base)?;
        let current = text_at(&self.doc, &self.schema, &r.path, &self.lists)?;
        // The field must be the same container the page edited: a row removed and
        // reinserted with the same `$id` has a new text that `base` never saw.
        if let ContainerID::Normal { peer, counter, .. } = current.id() {
            if !base_vv.includes_id(ID::new(peer, counter)) {
                return Err(err("path_not_found", "Text identity changed"));
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
        let (authored, positions) = if current.to_string() == r.from {
            // Nearly every keystroke: nothing else changed this field since `base`.
            current.apply_delta(&delta).map_err(engine)?;
            self.doc.commit();
            (self.version(), [r.selectionStart, r.selectionEnd])
        } else {
            // This field changed concurrently: author the edit on a branch at `base` and
            // let Loro merge it, so neither side's characters are lost.
            let branch = self.doc.fork_at(&base_at).map_err(engine)?;
            let text = text_at(&branch, &self.schema, &r.path, &HashMap::new())?;
            if text.id() != current.id() {
                return Err(err("path_not_found", "Text identity changed"));
            }
            if text.to_string() != r.from {
                return Err(err("stale_base", "The field was not `from` at `base`"));
            }
            text.apply_delta(&delta).map_err(engine)?;
            branch.commit();
            let cursors = selection.map(|offset| {
                text.get_cursor(offset, Side::Middle)
                    .ok_or_else(|| err("out_of_range", "Cannot anchor selection"))
            });
            let [start, end] = cursors;
            let cursors = [start?, end?];
            self.doc
                .import(&branch.export(ExportMode::updates(&base_vv)).map_err(engine)?)
                .map_err(|e| err("invalid_bytes", e))?;
            let merged = current.to_string();
            let mut positions = [0usize; 2];
            for (slot, cursor) in positions.iter_mut().zip(cursors) {
                let pos = self.doc.get_cursor_pos(&cursor).map_err(engine)?.current.pos;
                *slot = utf16_offset(&merged, pos);
            }
            (version_token(&branch.oplog_frontiers()), positions)
        };
        let publication = self.publish()?;
        Ok(TextEdit {
            sequence: self.sequence,
            authored,
            selection_start: positions[0],
            selection_end: positions[1],
            publication: Some(publication),
        })
    }
}
