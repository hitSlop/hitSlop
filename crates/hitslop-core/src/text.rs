//! Text sets from what the writer last saw. The page, or an agent, reports that a field went
//! from `from` (its text as the writer last confirmed it) to the set's value; the owner
//! computes the edit script and, when the field changed since, merges it three ways with
//! what changed: the writer's script is rebased over `from → current` and applied to the
//! live text as this session's peer. No version is involved and no draft state survives a
//! batch.
use super::*;
use execute::Location;
use loro::{TextDelta, UpdateOptions, event::Diff};
use wire::Selection;

/// Bounds the diff; past it the script falls back to a single caret-hinted splice.
const SCRIPT_TIMEOUT_MS: f64 = 50.0;

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

/// What a script does to each character of the text it starts from: the characters it
/// inserts before character `k` (`k == len` is the end), and whether character `k` stays.
struct Shape {
    before: Vec<Vec<char>>,
    kept: Vec<bool>,
}
fn shape(delta: &[TextDelta], len: usize) -> Shape {
    let mut out = Shape { before: vec![vec![]; len + 1], kept: vec![true; len] };
    let mut at = 0;
    // An insertion right after a deletion replaces it, so it belongs where the deleted span
    // starts: an edit made inside that span lands after the replacement.
    let mut replaced = None;
    for item in delta {
        match item {
            TextDelta::Retain { retain, .. } => {
                at += retain;
                replaced = None;
            }
            TextDelta::Delete { delete } => {
                out.kept[at..at + delete].iter_mut().for_each(|kept| *kept = false);
                replaced = replaced.or(Some(at));
                at += delete;
            }
            TextDelta::Insert { insert, .. } => out.before[replaced.unwrap_or(at)].extend(insert.chars()),
        }
    }
    out
}
fn push(delta: &mut Vec<TextDelta>, item: TextDelta) {
    match (delta.last_mut(), item) {
        (_, TextDelta::Retain { retain: 0, .. } | TextDelta::Delete { delete: 0 }) => {}
        (_, TextDelta::Insert { insert, .. }) if insert.is_empty() => {}
        (Some(TextDelta::Retain { retain, .. }), TextDelta::Retain { retain: more, .. }) => *retain += more,
        (Some(TextDelta::Delete { delete }), TextDelta::Delete { delete: more }) => *delete += more,
        (Some(TextDelta::Insert { insert, .. }), TextDelta::Insert { insert: more, .. }) => insert.push_str(&more),
        (_, item) => delta.push(item),
    }
}
/// The writer's edit `from → to` rebased over `from → current`: the delta to apply to
/// `current`, and `selection` (Unicode offsets in `to`) mapped into the merged text. At one
/// place, what changed concurrently comes first and the writer's insertion after it, so a
/// caret at the end of its own insertion stays there.
pub(crate) fn rebase(from: &str, current: &str, to: &str, selection: [usize; 2]) -> (Vec<TextDelta>, [usize; 2]) {
    let base: Vec<char> = from.chars().collect();
    let theirs = shape(&script(from, current, current.chars().count()), base.len());
    let mine = shape(&script(from, to, selection[1]), base.len());
    let mut delta = vec![];
    let (mut merged, mut in_to) = (0, 0);
    let mut mapped: [Option<usize>; 2] = [None, None];
    let mut note = |in_to: usize, merged: usize| {
        for (slot, offset) in mapped.iter_mut().zip(selection) {
            if slot.is_none() && in_to == offset {
                *slot = Some(merged);
            }
        }
    };
    for k in 0..=base.len() {
        push(&mut delta, TextDelta::Retain { retain: theirs.before[k].len(), attributes: None });
        merged += theirs.before[k].len();
        note(in_to, merged);
        for ch in &mine.before[k] {
            push(&mut delta, TextDelta::Insert { insert: ch.to_string(), attributes: None });
            merged += 1;
            in_to += 1;
            note(in_to, merged);
        }
        if k < base.len() {
            match (theirs.kept[k], mine.kept[k]) {
                (true, true) => {
                    push(&mut delta, TextDelta::Retain { retain: 1, attributes: None });
                    merged += 1;
                }
                (true, false) => push(&mut delta, TextDelta::Delete { delete: 1 }),
                (false, _) => {}
            }
            if mine.kept[k] {
                in_to += 1;
                note(in_to, merged);
            }
        }
    }
    // Trailing retains are implicit.
    while matches!(delta.last(), Some(TextDelta::Retain { .. })) {
        delta.pop();
    }
    (delta, mapped.map(|offset| offset.unwrap_or(merged)))
}

/// The page's text edit: the field went from `from` to `to` with the caret at `caret`
/// (UTF-16, in `to`). `selection` is in the merged text; `merged` says another change was
/// merged in.
pub(super) struct Typed {
    pub(super) path: Vec<Segment>,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) caret: usize,
    pub(super) selection: [usize; 2],
    pub(super) merged: bool,
}

/// A text set from what the writer last saw: `at` is the field, `value` its new text, and
/// `from` the text the writer started from (the current text when absent). Only the page's
/// edit, the set carrying `selection`, reports back through `typed`.
pub(super) fn set(
    path: &[Segment],
    at: Location,
    value: &Value,
    from: Option<&str>,
    selection: Option<Selection>,
    typed: &mut Option<Typed>,
) -> Result<()> {
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
    let now = current.as_ref().map(LoroText::to_string).unwrap_or_default();
    let from = from.map_or_else(|| now.clone(), str::to_owned);
    let record = |positions: [usize; 2], merged: bool| {
        given.map(|[_, caret]| Typed {
            path: path.to_vec(),
            from: from.clone(),
            to: to.to_owned(),
            caret,
            selection: positions,
            merged,
        })
    };
    // An unset optional text reads as "": the first edit from "" creates it.
    let Some(current) = current else {
        if !from.is_empty() {
            return Err(err(Code::PathNotFound, "Text was cleared"));
        }
        if !to.is_empty() {
            let (map, key) = at.parent.ok_or_else(|| err(Code::TypeMismatch, "Expected a field"))?;
            put(&map, &key, &Node::Text {}, value)?;
        }
        *typed = record(given.unwrap_or_default(), false);
        return Ok(());
    };
    if now == from {
        // Nearly every keystroke: nothing else changed this field since the writer saw it.
        if from != to {
            current.apply_delta(&script(&from, to, selection[1])).map_err(engine)?;
        }
        *typed = record(given.unwrap_or_default(), false);
        return Ok(());
    }
    let (delta, positions) = rebase(&from, &now, to, selection);
    if !delta.is_empty() {
        current.apply_delta(&delta).map_err(engine)?;
    }
    let merged = current.to_string();
    *typed = record(positions.map(|offset| utf16_offset(&merged, offset)), true);
    Ok(())
}
