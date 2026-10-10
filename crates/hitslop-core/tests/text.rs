// Stateless text. A page reports that a field went from `from`, the text it last confirmed,
// to `to`; the owner merges it three ways with whatever else changed. Failure: lost or
// duplicated characters, a misplaced caret, a resurrected row, or a refused edit that
// changed the document. Oracle: literal merged strings and UTF-16 carets, and an unchanged
// snapshot on refusal.
mod support;
use hitslop_core::{Document, Origin};
use serde_json::{Value, json};
use support::ApplyJson;
use support::{Edit, app, fixture, snapshot, trimmed, type_text, typed};

const ROW: &str = "00000000000000000000000000000001";
fn schema() -> String {
    fixture("checklist")["schema"].to_string()
}
fn setup() -> Document {
    Document::create(&app(schema()), &fixture("checklist")["initial"].to_string()).unwrap()
}
fn title(d: &Document) -> String {
    snapshot(d)["value"]["title"].as_str().unwrap().to_owned()
}
fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}
/// What a page reads from its text edit's reply.
struct Reply {
    selection_start: usize,
    publication: Option<String>,
}
impl Reply {
    fn of(applied: hitslop_core::Applied) -> Self {
        let text = applied.text.expect("a text edit's reply");
        Self { selection_start: text.selection[0], publication: applied.publication }
    }
}
/// One page binding: the text it last sent, which its next edit goes from.
struct Binding {
    path: Value,
    text: String,
}
impl Binding {
    fn new(d: &Document, path: Value) -> Self {
        let mut at = &snapshot(d)["value"];
        for segment in path.as_array().unwrap() {
            at = match segment {
                Value::String(key) => &at[key],
                Value::Object(id) => at.as_array().unwrap().iter().find(|row| row["$id"] == id["id"]).unwrap(),
                _ => unreachable!(),
            };
        }
        Self { path, text: at.as_str().unwrap().to_owned() }
    }
    fn request(&self, to: &str, caret: usize) -> String {
        typed(self.path.clone(), &self.text, to, caret)
    }
    /// Sends `to` with the caret at a UTF-16 offset; keeps typing from what it sent.
    fn edit(&mut self, d: &mut Document, to: &str, caret: usize) -> Reply {
        let reply = Reply::of(d.apply_json(&self.request(to, caret), Origin::Page).unwrap());
        self.text = to.to_owned();
        reply
    }
    fn refused(&self, d: &mut Document, to: &str) -> String {
        let before = snapshot(d);
        let code = d.apply_json(&self.request(to, utf16(to)), Origin::Page).unwrap_err().code.as_str();
        assert_eq!(snapshot(d), before, "a refused edit changed the document");
        code.to_owned()
    }
}
/// Another writer (the CLI) inserts at a UTF-16 offset of the current title.
fn splice(d: &mut Document, index: usize, insert: &str) {
    let units: Vec<u16> = title(d).encode_utf16().collect();
    let value = String::from_utf16(&units[..index]).unwrap() + insert + &String::from_utf16(&units[index..]).unwrap();
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":value}]}).to_string()).unwrap();
}

#[test]
fn queued_edits_merge_from_the_text_they_sent_not_the_merged_view() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["title"]));
    page.edit(&mut d, "abcX", 4);
    splice(&mut d, 0, "R"); // a CLI edit lands between acknowledgements
    page.edit(&mut d, "abcXY", 5);
    let reply = page.edit(&mut d, "abcXYZ", 6);
    assert_eq!(title(&d), "RabcXYZ");
    assert_eq!(reply.selection_start, 7);
    let reopened = Document::open(&app(schema()), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(title(&reopened), "RabcXYZ");
}

#[test]
fn a_concurrent_edit_on_a_trimmed_document_merges() {
    let mut d = Document::open(&app(schema()), &trimmed(&setup().checkpoint().unwrap()), &[]).unwrap();
    let mut page = Binding::new(&d, json!(["title"]));
    page.edit(&mut d, "abcX", 4);
    splice(&mut d, 0, "R");
    let reply = page.edit(&mut d, "abcXY", 5);
    assert_eq!(title(&d), "RabcXY");
    assert_eq!(reply.selection_start, 6);
    let reopened = Document::open(&app(schema()), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(title(&reopened), "RabcXY");
}

#[test]
fn two_bindings_on_one_field_with_delayed_replies_merge() {
    let mut d = setup();
    let mut left = Binding::new(&d, json!(["title"]));
    let mut right = Binding::new(&d, json!(["title"]));
    left.edit(&mut d, "abcL", 4);
    right.edit(&mut d, "Rabc", 1); // its view predates the left edit
    assert_eq!(title(&d), "RabcL");
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":"R-abcL"}]}).to_string()).unwrap();
    left.edit(&mut d, "abcLL", 5);
    assert_eq!(title(&d), "R-abcLL");
}

#[test]
fn disjoint_edits_coalesced_in_flight_merge_around_a_concurrent_change() {
    let mut d = setup();
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":"one two three"}]}).to_string()).unwrap();
    let mut page = Binding::new(&d, json!(["title"]));
    // Someone replaces the middle word while the page capitalizes both ends.
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":"one TWO three"}]}).to_string()).unwrap();
    page.edit(&mut d, "One two Three", 13);
    assert_eq!(title(&d), "One TWO Three");
}

#[test]
fn repeated_characters_insert_where_typed() {
    let mut d = setup();
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":"aaa"}]}).to_string()).unwrap();
    let mut page = Binding::new(&d, json!(["title"]));
    splice(&mut d, 3, "!");
    let reply = page.edit(&mut d, "abaa", 2);
    assert_eq!(title(&d), "abaa!");
    assert_eq!(reply.selection_start, 2);
}

// At one place, the concurrent insertion comes first and the page's after it, so the
// caret stays at the end of what the person typed.
#[test]
fn inserts_at_one_place_keep_the_caret_after_the_persons_typing() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["title"]));
    splice(&mut d, 3, "X");
    let reply = page.edit(&mut d, "abcY", 4);
    assert_eq!(title(&d), "abcXY");
    assert_eq!(reply.selection_start, 5);
}

#[test]
fn emoji_selection_maps_in_utf16_and_a_split_surrogate_is_refused() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["title"]));
    splice(&mut d, 0, "é");
    let reply = page.edit(&mut d, "abc😀", 5);
    assert_eq!(title(&d), "éabc😀");
    assert_eq!(reply.selection_start, 6);
    let before = snapshot(&d);
    let bad = type_text(&mut d, json!(["title"]), &page.text, "abc😀!", 4);
    assert_eq!(bad.unwrap_err().code.as_str(), "out_of_range");
    assert_eq!(snapshot(&d), before);
}

#[test]
fn caret_only_moves_publish_nothing() {
    let mut d = setup();
    let page = Binding::new(&d, json!(["title"]));
    let before = snapshot(&d);
    let reply = Reply::of(d.apply_json(&page.request("abc", 1), Origin::Page).unwrap());
    assert_eq!(snapshot(&d), before);
    assert_eq!(reply.selection_start, 1);
    assert!(reply.publication.is_none());
}

#[test]
fn an_unrelated_edit_keeps_the_fast_path() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["rows",{"id":ROW},"text"]));
    d.apply(&json!({"intents":[{"type":"set","path":["rows",{"id":ROW},"done"],"value":true}]}).to_string()).unwrap();
    let reply = page.edit(&mut d, "AB", 2);
    assert_eq!(reply.selection_start, 2);
    assert_eq!(snapshot(&d)["value"]["rows"][0]["text"], "AB");
}

// A removed row refuses; a row inserted again under the same `$id` is that row now, and
// takes the edit merged from what the page last saw.
#[test]
fn a_removed_row_refuses_and_a_reinserted_one_takes_the_edit() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["rows",{"id":ROW},"text"]));
    page.edit(&mut d, "AX", 2);
    d.apply(&json!({"intents":[{"type":"remove","path":["rows"],"id":ROW}]}).to_string()).unwrap();
    assert_eq!(page.refused(&mut d, "AXY"), "path_not_found");
    d.apply(
        &json!({"intents":[{"type":"insert","path":["rows"],"id":ROW,"value":{"text":"AX","done":false}}]}).to_string(),
    )
    .unwrap();
    page.edit(&mut d, "AXY", 3);
    assert_eq!(snapshot(&d)["value"]["rows"].as_array().unwrap().last().unwrap()["text"], "AXY");
}

#[test]
fn whole_field_set_is_exact_and_atomic_in_a_batch() {
    let mut d = setup();
    // Large, dissimilar texts stress the diff budget; the result is exact either way.
    let a: String = (0..60_000).map(|i| char::from(b'a' + (i * 7 % 26) as u8)).collect();
    let b: String = (0..60_000).map(|i| char::from(b'a' + (i * 11 % 26) as u8)).collect();
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":a}]}).to_string()).unwrap();
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":b}]}).to_string()).unwrap();
    assert_eq!(title(&d), b);
    let before = snapshot(&d);
    let batch = json!({"intents":[
        {"type":"set","path":["title"],"value":"short"},
        {"type":"set","path":["rows",{"id":ROW},"done"],"value":"not a boolean"}]});
    assert_eq!(d.apply(&batch.to_string()).unwrap_err().op_index, Some(1));
    assert_eq!(snapshot(&d), before);
}

// Failure: every keystroke republishes the whole field. Oracle: the publication of one
// typed character carries only that change, whatever the field's length.
#[test]
fn a_keystroke_publishes_only_its_change() {
    for length in [10, 100_000] {
        let mut d = setup();
        let from = "é".repeat(length);
        d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":from}]}).to_string()).unwrap();
        let to = format!("{}x{}", &from[..2 * (length / 2)], &from[2 * (length / 2)..]);
        let caret = utf16(&to[..2 * (length / 2) + 1]);
        let edit = type_text(&mut d, json!(["title"]), &from, &to, caret).unwrap();
        let publication: Value = serde_json::from_str(&edit.publication.unwrap()).unwrap();
        assert_eq!(
            publication["ops"],
            json!([{"type":"text","path":["title"],"delta":[{"retain":length / 2},{"insert":"x"}]}])
        );
        assert_eq!(title(&d), to);
    }
}

// Failure: an agent's text set replaced the field as the owner held it, so whatever the
// person typed after the agent read the document was deleted. Oracle: literal merged
// text, and undo of the agent's step alone.
#[test]
fn an_agents_set_from_its_read_keeps_what_the_person_typed_since() {
    let mut d = setup();
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":"Buy milk"}]}).to_string()).unwrap();
    let mut page = Binding::new(&d, json!(["title"]));
    page.edit(&mut d, "Buy milk and eggs", 17);
    let agent = json!({"intents":[{"type":"set","path":["title"],"value":"Buy oat milk","from":"Buy milk"}]});
    d.apply_json(&agent.to_string(), Origin::Agent).unwrap();
    assert_eq!(title(&d), "Buy oat milk and eggs");
    d.undo().unwrap();
    assert_eq!(title(&d), "Buy milk and eggs", "undo reverts the agent's step alone");
    let reopened = Document::open(&app(schema()), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(title(&reopened), "Buy milk and eggs");
}

// Failure: a batch refused after one of its text sets merged kept the merged text.
// Oracle: an unchanged snapshot, and a document that still edits and reopens.
#[test]
fn a_batch_refused_after_its_text_merge_changes_nothing() {
    let mut d = setup();
    splice(&mut d, 3, "!"); // the field changes after the agent's read: its set merges
    let before = snapshot(&d);
    let batch = json!({"intents":[
        {"type":"set","path":["title"],"value":"xyz","from":"abc"},
        {"type":"set","path":["rows",{"id":ROW},"done"],"value":"not a boolean"}]});
    assert_eq!(d.apply_json(&batch.to_string(), Origin::Agent).unwrap_err().op_index, Some(1));
    assert_eq!(snapshot(&d), before);
    splice(&mut d, 0, "R");
    let reopened = Document::open(&app(schema()), &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(title(&reopened), "Rabc!");
}

#[test]
fn text_set_fields_are_refused_where_they_do_not_apply() {
    let mut d = setup();
    let before = snapshot(&d);
    let refused = |d: &mut Document, batch: Value| {
        d.apply_json(&batch.to_string(), Origin::Page).unwrap_err().code.as_str().to_owned()
    };
    // A text edit with a selection is its own batch: its reply answers that edit.
    let typing = json!({"type":"set","path":["title"],"value":"abcX","from":"abc","selection":{"start":4,"end":4}});
    let increment = json!({"type":"increment","path":["hits"],"by":1});
    assert_eq!(refused(&mut d, json!({"intents":[typing, increment]})), "invalid_request");
    // `from` and `selection` describe a change of text only.
    let done = json!({"type":"set","path":["rows",{"id":ROW},"done"],"value":true,"from":"false"});
    assert_eq!(refused(&mut d, json!({"intents":[done]})), "type_mismatch");
    assert_eq!(snapshot(&d), before);
}

/// An agent rewrites the title from what it read while the page, from the same text,
/// types `typed`; the merged title.
fn rewrite_beside_typing(from: &str, rewrite: &str, typed: &str) -> String {
    let mut d = setup();
    d.apply(&json!({"intents":[{"type":"set","path":["title"],"value":from}]}).to_string()).unwrap();
    let agent = json!({"intents":[{"type":"set","path":["title"],"value":rewrite,"from":from}]});
    d.apply_json(&agent.to_string(), Origin::Agent).unwrap();
    type_text(&mut d, json!(["title"]), from, typed, utf16(typed)).unwrap();
    title(&d)
}

// Failure: a rewrite kept letters of the old word that happen to match the new one, so a
// concurrent keystroke anchored to them landed inside the new word ("lauXndry").
// Oracle: the new word stays whole and the keystroke lands at its edge.
#[test]
fn a_rewrite_keeps_its_new_word_whole_beside_a_concurrent_keystroke() {
    assert_eq!(rewrite_beside_typing("dry cleaning", "laundry", "dry cleaXning"), "laundryX");
}

// Failure (word-sized merging): two people fixing different letters of one word
// duplicated it. Oracle: both fixes, once.
#[test]
fn a_correction_and_a_capitalization_of_one_word_both_apply() {
    assert_eq!(rewrite_beside_typing("recieve", "receive", "Recieve"), "Receive");
}
