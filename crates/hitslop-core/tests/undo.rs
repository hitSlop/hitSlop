// Edit ▸ Undo. Failure: an agent's edit made while the document was open could not be
// undone, a typing run or an agent's run split into many
// steps, an agent's mistake erased the history, or the page's view fell behind the
// document. Oracle: literal values, and every publication applied to the page's view
// equals a fresh snapshot.
mod support;
use hitslop_core::{Document, Origin};
use serde_json::{json, Value};
use support::View;

const ROW: &str = "00000000000000000000000000000001";
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/checklist.json")).unwrap()
}
fn schema() -> String {
    fixture()["schema"].to_string()
}
fn setup() -> (Document, View) {
    let d = Document::create(&schema(), &fixture()["initial"].to_string()).unwrap();
    let view = View::of(&d);
    (d, view)
}
fn value(d: &Document) -> Value {
    serde_json::from_str::<Value>(&d.snapshot().unwrap()).unwrap()["value"].clone()
}
fn batch(intents: Value) -> String {
    json!({ "intents": intents }).to_string()
}
fn set(path: Value, value: Value) -> String {
    batch(json!([{ "type": "set", "path": path, "value": value }]))
}
/// Applies a change and shows its publication to the page.
fn apply(d: &mut Document, view: &mut View, batch: &str, origin: Origin) {
    if let Some(publication) = d.apply_batch(batch, origin).unwrap().publication {
        view.publish(&publication);
    }
}
fn undo(d: &mut Document, view: &mut View) -> bool {
    let publication = d.undo().unwrap().publication;
    publication.map(|p| view.publish(&p)).is_some()
}
fn redo(d: &mut Document, view: &mut View) -> bool {
    let publication = d.redo().unwrap().publication;
    publication.map(|p| view.publish(&p)).is_some()
}
/// One text field typed into like a page binding: each edit names the version and text
/// it started from, with the caret after it.
struct Field {
    path: Value,
    base: String,
    text: String,
}
impl Field {
    fn new(d: &Document, path: Value, text: &str) -> Self {
        Self { path, base: d.version(), text: text.into() }
    }
    fn edit(&mut self, d: &mut Document, view: &mut View, to: &str, caret: usize) {
        let request = json!({"base":self.base,"path":self.path,"from":self.text,"to":to,"selectionStart":caret,"selectionEnd":caret});
        let reply = d.edit_text(&request.to_string()).unwrap();
        if let Some(publication) = reply.publication {
            view.publish(&publication);
        }
        self.base = reply.authored;
        self.text = to.into();
    }
}

#[test]
fn a_page_change_undoes_and_redoes() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    assert!(d.can_undo() && !d.can_redo());
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["rows"][0]["done"], false);
    view.check(&d, "after undo");
    assert!(!d.can_undo() && d.can_redo());
    assert!(redo(&mut d, &mut view));
    assert_eq!(value(&d)["rows"][0]["done"], true);
    view.check(&d, "after redo");
}

#[test]
fn nothing_to_undo_publishes_nothing() {
    let (mut d, mut view) = setup();
    let sequence = d.sequence();
    assert!(!undo(&mut d, &mut view), "the template itself is not undoable");
    assert!(!redo(&mut d, &mut view));
    assert_eq!(d.sequence(), sequence);
}

#[test]
fn an_agents_run_of_batches_is_one_step() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    apply(&mut d, &mut view, &batch(json!([{"type":"increment","path":["hits"],"by":5}])), Origin::Agent);
    apply(&mut d, &mut view, &set(json!(["title"]), json!("Agent")), Origin::Agent);
    assert!(undo(&mut d, &mut view), "what the agent just did");
    assert_eq!((value(&d)["hits"].clone(), value(&d)["title"].clone()), (json!(0), json!("abc")));
    assert_eq!(value(&d)["rows"][0]["done"], true);
    view.check(&d, "after undoing the agent");
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["rows"][0]["done"], false);
    assert!(redo(&mut d, &mut view) && redo(&mut d, &mut view));
    assert_eq!(value(&d)["hits"], 5);
    view.check(&d, "after redoing both");
}

#[test]
fn undo_reverts_an_agents_text_edit_first() {
    let (mut d, mut view) = setup();
    let mut title = Field::new(&d, json!(["title"]), "abc");
    title.edit(&mut d, &mut view, "abcX", 4);
    apply(&mut d, &mut view, &set(json!(["title"]), json!("RabcX")), Origin::Agent);
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "abcX");
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "abc");
    view.check(&d, "after undo");
}

#[test]
fn a_typing_run_is_one_step() {
    let (mut d, mut view) = setup();
    let mut title = Field::new(&d, json!(["title"]), "abc");
    for (to, caret) in [("abcd", 4), ("abcde", 5), ("abcd", 4), ("abcdf", 5)] {
        title.edit(&mut d, &mut view, to, caret);
    }
    // The caret moved: typing elsewhere is a new step.
    title.edit(&mut d, &mut view, "Zabcdf", 1);
    title.edit(&mut d, &mut view, "Zyabcdf", 2);
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "abcdf");
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "abc");
    assert!(!d.can_undo());
    view.check(&d, "after undoing both runs");
    assert!(redo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "abcdf");
}

#[test]
fn another_field_or_a_change_ends_a_typing_run() {
    let (mut d, mut view) = setup();
    let mut title = Field::new(&d, json!(["title"]), "abc");
    title.edit(&mut d, &mut view, "abcd", 4);
    let mut row = Field::new(&d, json!(["rows", {"id": ROW}, "text"]), "A");
    row.edit(&mut d, &mut view, "AB", 2);
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    title.base = d.version();
    title.edit(&mut d, &mut view, "abcde", 5);
    for expected in [json!({"title":"abcd","row":"AB","done":true}), json!({"title":"abcd","row":"AB","done":false}),
        json!({"title":"abcd","row":"A","done":false}), json!({"title":"abc","row":"A","done":false})] {
        assert!(undo(&mut d, &mut view));
        let v = value(&d);
        assert_eq!(json!({"title":v["title"],"row":v["rows"][0]["text"],"done":v["rows"][0]["done"]}), expected);
    }
    view.check(&d, "after undoing every step");
}

#[test]
fn undoing_a_removal_restores_the_row_and_its_id() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &batch(json!([{"type":"remove","path":["rows"],"id":ROW}])), Origin::Page);
    assert!(undo(&mut d, &mut view));
    let v = value(&d);
    assert_eq!(v["rows"][0]["$id"], ROW);
    assert_eq!(v["rows"][0]["text"], "A");
    let state: Value = serde_json::from_str(&d.snapshot().unwrap()).unwrap();
    assert_eq!(state["issues"], json!([]));
    view.check(&d, "after restoring the row");
    // The restored row is editable by its ID.
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    view.check(&d, "after editing the restored row");
}

// Failure: an agent's batch refused after its first intent rebuilt the owner, and with
// it the undo history, so one agent mistake erased the person's undo.
#[test]
fn an_agents_refused_batch_keeps_the_persons_undo() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    let refused = d.apply_batch(&batch(json!([
        {"type":"increment","path":["hits"],"by":1},
        {"type":"set","path":["missing"],"value":1},
    ])), Origin::Agent);
    assert_eq!(refused.err().and_then(|e| e.op_index), Some(1));
    view.check(&d, "after the refusal");
    assert!(undo(&mut d, &mut view), "the person's step survives");
    assert_eq!(value(&d)["rows"][0]["done"], false);
    assert_eq!(value(&d)["hits"], 0);
}

#[test]
fn a_refused_page_batch_keeps_undo_and_redo() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    assert!(d.apply_batch(&batch(json!([
        {"type":"increment","path":["hits"],"by":1},
        {"type":"set","path":["missing"],"value":1},
    ])), Origin::Page).is_err());
    view.check(&d, "after the refusal");
    assert!(undo(&mut d, &mut view), "the earlier step survives the refusal");
    let before = d.snapshot().unwrap();
    assert!(d.apply_batch(&batch(json!([
        {"type":"increment","path":["hits"],"by":1},
        {"type":"set","path":["missing"],"value":1},
    ])), Origin::Page).is_err());
    assert_eq!(d.snapshot().unwrap(), before);
    assert!(redo(&mut d, &mut view), "redo survives the refusal too");
    assert_eq!(value(&d)["rows"][0]["done"], true);
    view.check(&d, "after redo across a refusal");
}

// Failure: the agent tag set for a batch that changed nothing would label the person's
// next change, which a later window would then undo as the agent's.
#[test]
fn an_agent_batch_that_changes_nothing_leaves_the_next_change_the_persons() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &batch(json!([])), Origin::Agent);
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    let reopened = Document::open(&schema(), &d.checkpoint().unwrap(), &[]).unwrap();
    assert!(!reopened.can_undo());
}

/// A session like the CLI's on a closed document: open the saved bytes, edit, save.
fn session(checkpoint: &[u8], batches: &[(String, Origin)]) -> Vec<u8> {
    let mut d = Document::open(&schema(), checkpoint, &[]).unwrap();
    for (batch, origin) in batches {
        d.apply_batch(batch, *origin).unwrap();
    }
    d.checkpoint().unwrap()
}

// Undo covers the open session only: edits saved by an earlier session, the person's or
// an agent's, are the starting point of the next one.
#[test]
fn a_reopened_document_starts_with_nothing_to_undo() {
    let (d, _) = setup();
    let saved = session(&d.checkpoint().unwrap(), &[
        (set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page),
        (set(json!(["title"]), json!("Agent")), Origin::Agent),
    ]);
    let mut d = Document::open(&schema(), &saved, &[]).unwrap();
    let mut view = View::of(&d);
    assert!(!d.can_undo() && !d.can_redo());
    assert!(!undo(&mut d, &mut view));
    apply(&mut d, &mut view, &set(json!(["title"]), json!("Mine")), Origin::Page);
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "Agent", "undo stops where the session opened");
    assert!(!d.can_undo());
    view.check(&d, "after undoing the session");
}

#[test]
fn undo_survives_a_concurrent_text_edit() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &set(json!(["rows", {"id": ROW}, "done"]), json!(true)), Origin::Page);
    let mut title = Field::new(&d, json!(["title"]), "abc");
    apply(&mut d, &mut view, &set(json!(["title"]), json!("Rabc")), Origin::Agent);
    // Branches from before the agent's edit and merges.
    title.edit(&mut d, &mut view, "abcX", 4);
    assert_eq!(value(&d)["title"], "RabcX");
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "Rabc", "the person's merged keystroke goes first");
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "abc", "then the agent's edit");
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["rows"][0]["done"], false);
    view.check(&d, "after undo");
    assert!(redo(&mut d, &mut view) && redo(&mut d, &mut view) && redo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "RabcX");
    view.check(&d, "after redoing concurrent typing");
}

// Failure: mixing a paused UndoManager with revert_to corrupted redo after undoing an
// agent step. Test text replacement and container restoration below an agent's edit.
#[test]
fn agent_and_person_steps_round_trip_repeatedly() {
    for change in [set(json!(["title"]), json!("PERSON")), batch(json!([
        {"type":"remove","path":["rows"],"id":ROW},
    ]))] {
        let (mut d, mut view) = setup();
        let original = value(&d);
        apply(&mut d, &mut view, &set(json!(["title"]), json!("AGENT")), Origin::Agent);
        let agent = value(&d);
        apply(&mut d, &mut view, &change, Origin::Page);
        let person = value(&d);
        for _ in 0..5 {
            for expected in [&agent, &original] {
                assert!(undo(&mut d, &mut view));
                assert_eq!(&value(&d), expected);
                view.check(&d, "undo across the agent step");
            }
            assert!(!d.can_undo());
            for expected in [&agent, &person] {
                assert!(redo(&mut d, &mut view));
                assert_eq!(&value(&d), expected);
                view.check(&d, "redo across the agent step");
            }
            assert!(!d.can_redo());
        }
    }
}

// Failure: the rehearsal used a new peer's counter contribution, so an agent's
// valid batch was rejected even though the same page batch succeeded.
#[test]
fn counter_batches_use_the_live_writer_for_both_origins() {
    let schema = json!({"kind":"object","properties":{"a":{"kind":"boolean"},"z":{"kind":"counter"}}}).to_string();
    let initial = json!({"a":false,"z":9007199254740991i64}).to_string();
    for origin in [Origin::Page, Origin::Agent] {
        let mut d = Document::create(&schema, &initial).unwrap();
        let mut view = View::of(&d);
        apply(&mut d, &mut view, &set(json!(["a"]), json!(true)), Origin::Page);
        apply(&mut d, &mut view, &batch(json!([
            {"type":"increment","path":["z"],"by":-9007199254740991i64},
            {"type":"increment","path":["z"],"by":-9007199254740991i64},
        ])), origin);
        assert_eq!(value(&d)["z"], -9007199254740991i64);
        assert!(undo(&mut d, &mut view));
        assert_eq!(value(&d)["z"], 9007199254740991i64);
        assert!(redo(&mut d, &mut view));
        assert_eq!(value(&d)["z"], -9007199254740991i64);
        view.check(&d, "after counter redo");
    }
}

#[test]
fn undo_works_on_a_trimmed_document() {
    let full = Document::create(&schema(), &fixture()["initial"].to_string()).unwrap().checkpoint().unwrap();
    let loro = loro::LoroDoc::new();
    loro.import(&full).unwrap();
    let shallow = loro.export(loro::ExportMode::shallow_snapshot(&loro.oplog_frontiers())).unwrap();
    let mut d = Document::open(&schema(), &shallow, &[]).unwrap();
    let mut view = View::of(&d);
    apply(&mut d, &mut view, &batch(json!([{"type":"remove","path":["rows"],"id":ROW}])), Origin::Page);
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["rows"][0]["$id"], ROW);
    view.check(&d, "after undo");
}

#[test]
fn history_is_bounded() {
    let (mut d, mut view) = setup();
    apply(&mut d, &mut view, &set(json!(["title"]), json!("Agent")), Origin::Agent);
    for _ in 0..100 {
        apply(&mut d, &mut view, &batch(json!([{"type":"increment","path":["hits"],"by":1}])), Origin::Page);
    }
    for expected in (0..=99).rev() {
        assert!(undo(&mut d, &mut view));
        assert_eq!(value(&d)["hits"], expected);
        view.check(&d, "undo within the retained steps");
    }
    assert!(!d.can_undo());
    assert_eq!(value(&d)["title"], "Agent", "the oldest step was evicted");
    for _ in 0..100 { assert!(redo(&mut d, &mut view)); }
    assert!(!d.can_redo());
    assert_eq!(value(&d)["hits"], 100);
    view.check(&d, "redo every retained step");
}

#[test]
fn noops_and_refusals_preserve_runs_and_redo_but_new_edits_clear_redo() {
    let (mut d, mut view) = setup();
    let increase = batch(json!([{"type":"increment","path":["hits"],"by":1}]));
    apply(&mut d, &mut view, &increase, Origin::Agent);
    apply(&mut d, &mut view, &set(json!(["title"]), json!("abc")), Origin::Page);
    assert!(d.apply_batch(&batch(json!([
        {"type":"increment","path":["hits"],"by":1},
        {"type":"set","path":["missing"],"value":1},
    ])), Origin::Page).is_err());
    apply(&mut d, &mut view, &increase, Origin::Agent);
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["hits"], 0, "the agent run survived the noop and refusal");
    let before = d.snapshot().unwrap();
    apply(&mut d, &mut view, &set(json!(["title"]), json!("abc")), Origin::Agent);
    assert_eq!(d.snapshot().unwrap(), before);
    assert!(redo(&mut d, &mut view));
    assert_eq!(value(&d)["hits"], 2);
    assert!(undo(&mut d, &mut view));
    apply(&mut d, &mut view, &set(json!(["title"]), json!("New")), Origin::Page);
    assert!(!d.can_redo());
    view.check(&d, "new edit after undo");
}

#[test]
fn raw_replica_imports_end_history_but_duplicate_and_refused_imports_do_not() {
    let (mut d, mut view) = setup();
    let base = d.version();
    let mut peer = Document::open(&schema(), &d.checkpoint().unwrap(), &[]).unwrap();
    peer.apply_batch(&set(json!(["title"]), json!("Remote")), Origin::Page).unwrap();
    let bytes = peer.export_since(&base).unwrap();
    apply(&mut d, &mut view, &set(json!(["rows", {"id":ROW}, "done"]), json!(true)), Origin::Page);
    assert!(undo(&mut d, &mut view));
    assert!(d.can_redo());
    view.publish(&d.import(&bytes).unwrap().unwrap());
    assert!(!d.can_undo() && !d.can_redo());
    assert_eq!(value(&d)["title"], "Remote");
    apply(&mut d, &mut view, &set(json!(["title"]), json!("Local")), Origin::Page);
    assert!(d.import(&bytes).unwrap().is_none());
    assert!(d.import(b"invalid").is_err());
    assert!(undo(&mut d, &mut view));
    assert_eq!(value(&d)["title"], "Remote", "undo cannot reach before the import");
    view.check(&d, "undo after duplicate and refused imports");
    assert!(!d.can_undo());
}
