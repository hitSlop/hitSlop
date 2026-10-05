// Stateless text (spike S-C). A page reports that a field went from `from` at `base`
// to `to`; the owner merges it with whatever else changed. Failure: lost or duplicated
// characters, a misplaced caret, a resurrected row, or a panic on a bad base.
// Oracle: literal merged strings and UTF-16 carets, and an unchanged snapshot on refusal.
mod support;
use support::{app, Edit, fixture, snapshot, trimmed};
use hitslop_core::{Document, TextEdit};
use serde_json::{json, Value};

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
/// One page binding: its authored version and the text it last sent.
struct Binding {
    path: Value,
    base: String,
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
        Self { path, base: d.version(), text: at.as_str().unwrap().to_owned() }
    }
    fn request(&self, to: &str, caret: usize) -> String {
        json!({"base":self.base,"path":self.path,"from":self.text,"to":to,"selectionStart":caret,"selectionEnd":caret})
            .to_string()
    }
    /// Sends `to` with the caret at a UTF-16 offset; adopts the reply like the page does.
    fn edit(&mut self, d: &mut Document, to: &str, caret: usize) -> TextEdit {
        let reply = d.edit_text(&self.request(to, caret)).unwrap();
        self.base = reply.authored.clone();
        self.text = to.to_owned();
        reply
    }
    fn refused(&self, d: &mut Document, to: &str) -> String {
        let before = snapshot(d);
        let code = d.edit_text(&self.request(to, utf16(to))).unwrap_err().code.as_str();
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
fn queued_edits_branch_from_their_authored_text_not_the_merged_view() {
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

// Failure: a concurrent edit branched with `LoroDoc::fork_at`, which Loro does not
// implement for trimmed documents, so it failed on every document after a checkpoint.
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
    // A concurrent whole-field set elsewhere in the text (a tie at one position would be
    // ordered by peer ID, which is random).
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

#[test]
fn emoji_selection_maps_in_utf16_and_a_split_surrogate_is_refused() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["title"]));
    splice(&mut d, 0, "é");
    let reply = page.edit(&mut d, "abc😀", 5);
    assert_eq!(title(&d), "éabc😀");
    assert_eq!(reply.selection_start, 6);
    let before = snapshot(&d);
    let bad = json!({"base":page.base,"path":["title"],"from":page.text,"to":"abc😀!","selectionStart":4,"selectionEnd":4});
    assert_eq!(d.edit_text(&bad.to_string()).unwrap_err().code.as_str(), "out_of_range");
    assert_eq!(snapshot(&d), before);
}

#[test]
fn caret_only_moves_publish_nothing() {
    let mut d = setup();
    let page = Binding::new(&d, json!(["title"]));
    let before = snapshot(&d);
    let reply = d.edit_text(&page.request("abc", 1)).unwrap();
    assert_eq!(snapshot(&d), before);
    assert_eq!(reply.authored, page.base);
    assert!(reply.publication.is_none());
}

#[test]
fn an_unrelated_edit_keeps_the_fast_path() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["rows",{"id":ROW},"text"]));
    d.apply(&json!({"intents":[{"type":"set","path":["rows",{"id":ROW},"done"],"value":true}]}).to_string()).unwrap();
    let reply = page.edit(&mut d, "AB", 2);
    // The owner edited directly: the authored version is the owner's own.
    assert_eq!(reply.authored, d.version());
    assert_eq!(snapshot(&d)["value"]["rows"][0]["text"], "AB");
}

#[test]
fn removed_or_reinserted_rows_are_never_resurrected() {
    let mut d = setup();
    let mut page = Binding::new(&d, json!(["rows",{"id":ROW},"text"]));
    page.edit(&mut d, "AX", 2);
    d.apply(&json!({"intents":[{"type":"remove","path":["rows"],"id":ROW}]}).to_string()).unwrap();
    assert_eq!(page.refused(&mut d, "AXY"), "path_not_found");
    d.apply(&json!({"intents":[{"type":"insert","path":["rows"],"id":ROW,"value":{"text":"AX","done":false}}]}).to_string()).unwrap();
    // Same `$id`, new text container: the page's base never saw it.
    assert_eq!(page.refused(&mut d, "AXY"), "path_not_found");
}

#[test]
fn bad_bases_are_refused_on_every_path_without_panicking() {
    // A version of another document's history.
    let foreign = {
        let mut other = setup();
        other.apply(&json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}).to_string()).unwrap();
        other.version()
    };
    let mut d = setup();
    splice(&mut d, 0, "R");
    for base in [foreign.as_str(), "zz", "", "00", "0000000000000001ffffffff"] {
        // fast (owner text equals `from`), no-op (`from == to`) and slow paths.
        for (from, to) in [("Rabc", "RabcX"), ("abc", "abc"), ("abc", "abcX")] {
            let before = snapshot(&d);
            let request = json!({"base":base,"path":["title"],"from":from,"to":to,"selectionStart":0,"selectionEnd":0});
            let code = d.edit_text(&request.to_string()).unwrap_err().code.as_str();
            assert!(["stale_base", "invalid_version"].contains(&code), "{base} {from}->{to}: {code}");
            assert_eq!(snapshot(&d), before);
        }
    }
    // A known base whose text was not `from` is stale, never silently rebased.
    let page = Binding { path: json!(["title"]), base: d.version(), text: "zzz".into() };
    splice(&mut d, 0, "S");
    assert_eq!(page.refused(&mut d, "zzzz"), "stale_base");
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
        let edit = d.edit_text(&json!({"base":d.version(),"path":["title"],"from":from,"to":to,"selectionStart":caret,"selectionEnd":caret}).to_string()).unwrap();
        let publication: Value = serde_json::from_str(&edit.publication.unwrap()).unwrap();
        assert_eq!(publication["ops"], json!([{"type":"text","path":["title"],"delta":[{"retain":length / 2},{"insert":"x"}]}]));
        assert_eq!(title(&d), to);
    }
}
