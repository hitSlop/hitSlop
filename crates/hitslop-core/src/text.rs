//! Text sets from a version. The page, or an agent, reports that a field went from `from`
//! (its text at the batch's `base`) to the set's value; the owner computes the edit script
//! and merges it with whatever changed since. No draft state survives a batch.
use super::*;
use execute::Location;
use loro::{TextDelta, UpdateOptions, cursor::Side, event::Diff};
use wire::Selection;

/// Bounds the diff; past it the script falls back to a single caret-hinted splice.
const SCRIPT_TIMEOUT_MS: f64 = 50.0;

/// A branch at `base` for authoring one concurrent edit: the state there with minimal
/// history, so trimmed documents branch too (Loro has no `fork_at` for them). Safe where a
/// long-lived replica is not (see `replica_at`): it lives for one edit, authors only text
/// operations after checking its text equals `from`, and never imports later changes.
fn branch_at(doc: &LoroDoc, base: &Frontiers) -> Result<LoroDoc> {
    let branch = LoroDoc::new();
    branch.set_record_timestamp(true);
    branch.import(&doc.export(ExportMode::state_only(Some(base))).map_err(engine)?).map_err(engine)?;
    Ok(branch)
}
/// The text at `path` in a branch; `None` for an optional text that is not set.
fn text_at(doc: &LoroDoc, schema: &Node, path: &[Segment]) -> Result<Option<LoroText>> {
    let loc = resolve(doc, schema, path, &Rows::new(&HashMap::new()))?;
    match (unwrap_optional(loc.node), loc.value) {
        (Node::Text {}, ValueOrContainer::Container(Container::Text(text))) => Ok(Some(text)),
        (Node::Text {}, _) if loc.absent => Ok(None),
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
        Err(err(Code::OutOfRange, "Selection splits a surrogate or exceeds text"))
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
    let suffix = a[prefix..].iter().rev().zip(b[prefix..].iter().rev()).take_while(|(x, y)| x == y).count();
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
/// become one delete-and-reinsert that merges badly with concurrent edits, but a rewrite
/// does (`semantic`). Computed on a throwaway document, because `LoroText::update` mutates
/// while it diffs and a timeout would leave a partial edit. Loro's refined diff stays off:
/// it prices every gap the same whatever the field's length, so it would turn two small
/// edits in a short field into one replacement.
fn window(old: &[char], new: &[char]) -> Option<Vec<TextDelta>> {
    let scratch = LoroDoc::new();
    let text = scratch.get_text("t");
    text.insert(0, &old.iter().collect::<String>()).ok()?;
    scratch.commit();
    let start = scratch.oplog_frontiers();
    text.update(
        &new.iter().collect::<String>(),
        UpdateOptions { timeout_ms: Some(SCRIPT_TIMEOUT_MS), use_refined_diff: false },
    )
    .ok()?;
    scratch.commit();
    let batch = scratch.diff(&start, &scratch.oplog_frontiers()).ok()?;
    let delta = batch.iter().find_map(|(_, diff)| match diff {
        Diff::Text(delta) => semantic(old, delta),
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

/// diff-match-patch's semantic cleanup, without boundary shifting or overlap extraction: a
/// retained run between edits that is no longer than the larger edit on each side becomes
/// part of a replacement. A character-level diff keeps any letters the old and new text
/// happen to share, and a concurrent insert anchored to one would land inside the new word.
fn semantic(old: &[char], delta: &[TextDelta]) -> Option<Vec<TextDelta>> {
    enum Hunk {
        Keep(Vec<char>),
        Delete(Vec<char>),
        Insert(Vec<char>),
    }
    let mut hunks = vec![];
    let mut cursor = 0;
    for item in delta {
        match item {
            TextDelta::Retain { retain, .. } => {
                hunks.push(Hunk::Keep(old.get(cursor..cursor + retain)?.to_vec()));
                cursor += retain;
            }
            TextDelta::Delete { delete } => {
                hunks.push(Hunk::Delete(old.get(cursor..cursor + delete)?.to_vec()));
                cursor += delete;
            }
            TextDelta::Insert { insert, .. } => hunks.push(Hunk::Insert(insert.chars().collect())),
        }
    }
    // The kept runs not yet judged, and the edits on each side of the latest one.
    let mut kept: Vec<usize> = vec![];
    let (mut before, mut after) = ([0usize; 2], [0usize; 2]);
    let mut index = 0;
    while index < hunks.len() {
        match &hunks[index] {
            Hunk::Keep(_) => {
                kept.push(index);
                before = after;
                after = [0, 0];
            }
            Hunk::Delete(text) => after[0] += text.len(),
            Hunk::Insert(text) => after[1] += text.len(),
        }
        let chaff = match kept.last().map(|&at| &hunks[at]) {
            Some(Hunk::Keep(text)) if index != kept[kept.len() - 1] => {
                text.len() <= before[0].max(before[1]) && text.len() <= after[0].max(after[1])
            }
            _ => false,
        };
        if chaff {
            // Delete the run and insert it again, then judge the previous run anew.
            let at = kept.pop().expect("judged a kept run");
            if let Hunk::Keep(text) = &hunks[at] {
                let text = text.clone();
                hunks.splice(at..=at, [Hunk::Delete(text.clone()), Hunk::Insert(text)]);
            }
            kept.pop();
            (before, after) = ([0, 0], [0, 0]);
            index = kept.last().map_or(0, |&at| at + 1);
            continue;
        }
        index += 1;
    }
    // Each run of edits becomes one deletion and one insertion.
    let mut out = vec![];
    let (mut deleted, mut inserted) = (0, String::new());
    let flush = |out: &mut Vec<TextDelta>, deleted: &mut usize, inserted: &mut String| {
        if *deleted > 0 {
            out.push(TextDelta::Delete { delete: std::mem::take(deleted) });
        }
        if !inserted.is_empty() {
            out.push(TextDelta::Insert { insert: std::mem::take(inserted), attributes: None });
        }
    };
    for hunk in hunks {
        match hunk {
            Hunk::Keep(text) => {
                flush(&mut out, &mut deleted, &mut inserted);
                out.push(TextDelta::Retain { retain: text.len(), attributes: None });
            }
            Hunk::Delete(text) => deleted += text.len(),
            Hunk::Insert(text) => inserted.extend(text),
        }
    }
    flush(&mut out, &mut deleted, &mut inserted);
    Some(out)
}

/// A batch's `base`, decoded once before any intent runs.
pub(super) struct Base {
    pub(super) token: String,
    pub(super) at: Frontiers,
    pub(super) vv: VersionVector,
}
/// What a batch's text sets need beyond the document: its base, where the saved history
/// starts, and who sends it. Collects the page's text edit, the set carrying `selection`.
pub(super) struct Texts<'a> {
    pub(super) base: Option<&'a Base>,
    pub(super) floor: &'a VersionVector,
    pub(super) message: &'a str,
    pub(super) typed: Option<Typed>,
}
/// The page's text edit: the field went from `from` to `to` with the caret at `caret`
/// (UTF-16, in `to`). `authored` is its own version when it merged from a branch or
/// changed nothing, else the batch's; `selection` is in the merged text.
pub(super) struct Typed {
    pub(super) path: Vec<Segment>,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) caret: usize,
    pub(super) authored: Option<String>,
    pub(super) selection: [usize; 2],
    pub(super) merged: bool,
}

/// A text set from the batch's base: `at` is the field, `value` its new text.
#[expect(clippy::too_many_arguments, reason = "text merge inputs belong to one bounded operation")]
pub(super) fn set(
    doc: &LoroDoc,
    schema: &Node,
    path: &[Segment],
    at: Location,
    value: &Value,
    from: Option<&str>,
    selection: Option<Selection>,
    rows: &mut Rows,
    texts: &mut Texts,
) -> Result<()> {
    let base = texts.base.ok_or_else(|| err(Code::InvalidRequest, "`from` and `selection` need the batch's `base`"))?;
    let to = value.as_str().ok_or_else(|| err(Code::TypeMismatch, "Expected text"))?;
    let given = selection.map(|s| [s.start, s.end]);
    let selection = [
        unicode_offset(to, given.map_or(0, |s| s[0]))?,
        match given {
            Some([_, end]) => unicode_offset(to, end)?,
            None => to.chars().count(),
        },
    ];
    let current = match at.value {
        ValueOrContainer::Container(Container::Text(text)) => Some(text),
        _ if at.absent => None,
        _ => return Err(err(Code::TypeMismatch, "Expected text")),
    };
    // The text at `base`: the page's own record of it, else read from that version.
    let mut branch = None;
    let from = match from {
        Some(from) => from.to_owned(),
        None if base.at == doc.oplog_frontiers() && doc.get_pending_txn_len() == 0 => {
            current.as_ref().map(LoroText::to_string).unwrap_or_default()
        }
        None => {
            let at_base = branch_at(doc, &base.at)?;
            let text = text_at(&at_base, schema, path)?.map(|t| t.to_string()).unwrap_or_default();
            branch = Some(at_base);
            text
        }
    };
    // Only the page's edit, the set carrying a selection, reports back.
    let typed = |authored: Option<String>, positions: [usize; 2], merged: bool| {
        given.map(|[_, caret]| Typed {
            path: path.to_vec(),
            from: from.clone(),
            to: to.to_owned(),
            caret,
            authored,
            selection: positions,
            merged,
        })
    };
    // An unset optional text reads as "": the first edit from "" creates it.
    let Some(current) = current else {
        if !from.is_empty() {
            return Err(err(Code::PathNotFound, "Text was cleared"));
        }
        if to.is_empty() {
            texts.typed = typed(Some(base.token.clone()), given.unwrap_or_default(), false);
            return Ok(());
        }
        let (map, key) = at.parent.ok_or_else(|| err(Code::TypeMismatch, "Expected a field"))?;
        put(&map, &key, &Node::Text {}, value, rows)?;
        texts.typed = typed(None, given.unwrap_or_default(), false);
        return Ok(());
    };
    // The field must be the same container the writer edited: a row removed and
    // reinserted with the same `$id` has a new text that `base` never saw.
    if let ContainerID::Normal { peer, counter, .. } = current.id()
        && !base.vv.includes_id(ID::new(peer, counter))
    {
        return Err(err(Code::PathNotFound, "Text identity changed"));
    }
    if from == to {
        texts.typed = typed(Some(base.token.clone()), given.unwrap_or_default(), false);
        return Ok(());
    }
    let delta = script(&from, to, selection[1]);
    if current.to_string() == from {
        // Nearly every keystroke: nothing else changed this field since `base`.
        current.apply_delta(&delta).map_err(engine)?;
        texts.typed = typed(None, given.unwrap_or_default(), false);
        return Ok(());
    }
    // This field changed since `base`: author the edit on a branch at `base` and let Loro
    // merge it, so neither side's characters are lost.
    if !base.vv.includes_vv(texts.floor) {
        return Err(err(Code::StaleBase, "Version precedes the saved history"));
    }
    let branch = match branch {
        Some(branch) => branch,
        None => branch_at(doc, &base.at)?,
    };
    let text = text_at(&branch, schema, path)?.ok_or_else(|| err(Code::PathNotFound, "Text identity changed"))?;
    if text.id() != current.id() {
        return Err(err(Code::PathNotFound, "Text identity changed"));
    }
    if text.to_string() != from {
        return Err(err(Code::StaleBase, "The field was not `from` at `base`"));
    }
    text.apply_delta(&delta).map_err(engine)?;
    branch.set_next_commit_message(texts.message);
    branch.commit();
    let [start, end] = selection.map(|offset| {
        text.get_cursor(offset, Side::Middle).ok_or_else(|| err(Code::OutOfRange, "Cannot anchor selection"))
    });
    let cursors = [start?, end?];
    // Importing commits earlier intents too; all belong to this batch's origin.
    doc.set_next_commit_message(texts.message);
    doc.import(&branch.export(ExportMode::updates(&base.vv)).map_err(engine)?)
        .map_err(|e| err(Code::InvalidBytes, e))?;
    let merged = current.to_string();
    let mut positions = [0usize; 2];
    for (slot, cursor) in positions.iter_mut().zip(cursors) {
        let pos = doc.get_cursor_pos(&cursor).map_err(engine)?.current.pos;
        *slot = utf16_offset(&merged, pos);
    }
    texts.typed = typed(Some(version_token(&branch.oplog_frontiers())), positions, true);
    Ok(())
}
