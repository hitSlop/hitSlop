// `replace`: a value from JSON (`slop import`), written as its differences. Failure: rows
// or text were recreated, so an open text field was clobbered, or a refused replace left
// part of its change. Oracle: literal values, identity proven by later edits, and the
// page's view equal to a fresh snapshot after every publication.
mod support;
use hitslop_core::{Applied, Code, Document, Origin};
use serde_json::{Value, json};
use support::{View, app, fixture, type_text, value};

const A: &str = "00000000000000000000000000000001";
const B: &str = "00000000000000000000000000000002";

fn open(name: &str) -> (Document, View) {
    let f = fixture(name);
    let d = Document::create(&app(f["schema"].to_string()), &f["initial"].to_string()).unwrap();
    let view = View::of(&d);
    (d, view)
}
fn replace(path: Value, value: Value) -> String {
    json!({"intents":[{"type":"replace","path":path,"value":value}]}).to_string()
}
fn apply(d: &mut Document, view: &mut View, batch: &str) -> Applied {
    let applied = d.apply_batch(batch, Origin::Agent).unwrap();
    if let Some(publication) = &applied.publication {
        view.publish(publication);
    }
    applied
}
fn refused(d: &mut Document, batch: &str) -> Code {
    let before = d.state().unwrap();
    let code = d.apply_batch(batch, Origin::Agent).unwrap_err().code;
    assert_eq!(d.state().unwrap(), before, "a refused replace changes nothing");
    code
}

#[test]
fn replacing_a_value_with_itself_writes_nothing() {
    for name in ["checklist", "collections", "scalars", "nested"] {
        let (mut d, mut view) = open(name);
        let sequence = d.sequence();
        let same = replace(json!([]), value(&d));
        let applied = apply(&mut d, &mut view, &same);
        assert!(applied.publication.is_none() && applied.ids.is_empty(), "{name}");
        assert_eq!(d.sequence(), sequence);
    }
}

#[test]
fn rows_and_text_keep_their_identity() {
    let (mut d, mut view) = open("checklist");
    let base = d.version();
    let applied = apply(
        &mut d,
        &mut view,
        &replace(
            json!([]),
            json!({
                "title": "New title",
                "hits": 0,
                "rows": [
                    {"$id": B, "text": "B", "done": false},
                    {"$id": A, "text": "A", "done": true},
                    {"text": "C", "done": false},
                ],
            }),
        ),
    );
    assert_eq!(applied.ids.len(), 1, "the new row's minted ID");
    let v = value(&d);
    assert_eq!(
        v["rows"].as_array().unwrap().iter().map(|r| r["$id"].clone()).collect::<Vec<_>>(),
        [json!(B), json!(A), json!(applied.ids[0])]
    );
    assert_eq!((v["title"].clone(), v["rows"][1]["done"].clone()), (json!("New title"), json!(true)));
    view.check(&d, "after the replace");
    // A text field opened before the replace still edits the same text.
    let typed = type_text(&mut d, &base, json!(["rows",{"id":A},"text"]), "A", "AX", 2).unwrap();
    view.publish(&typed.publication.unwrap());
    assert_eq!(value(&d)["rows"][1]["text"], "AX");
    view.check(&d, "after typing in a kept row");
}

#[test]
fn rows_are_removed_inserted_and_reordered_with_the_fewest_moves() {
    let (mut d, mut view) = open("checklist");
    let rows =
        |ids: &[&str]| json!(ids.iter().map(|id| json!({"$id": id, "text": id, "done": false})).collect::<Vec<_>>());
    apply(&mut d, &mut view, &replace(json!(["rows"]), rows(&["a", "b", "c", "d", "e"])));
    let ids = |d: &Document| {
        value(d)["rows"].as_array().unwrap().iter().map(|r| r["$id"].as_str().unwrap().to_owned()).collect::<Vec<_>>()
    };
    assert_eq!(ids(&d), ["a", "b", "c", "d", "e"]);
    let moves = |applied: &Applied| {
        let publication: Value = serde_json::from_str(applied.publication.as_deref().unwrap()).unwrap();
        publication["ops"].as_array().unwrap().iter().filter(|op| op["type"] == "moveRow").count()
    };
    let applied = apply(&mut d, &mut view, &replace(json!(["rows"]), rows(&["b", "c", "d", "e", "a"])));
    assert_eq!((ids(&d), moves(&applied)), (vec!["b".to_owned(), "c".into(), "d".into(), "e".into(), "a".into()], 1));
    let applied = apply(&mut d, &mut view, &replace(json!(["rows"]), rows(&["x", "e", "c", "y", "b"])));
    assert_eq!(ids(&d), ["x", "e", "c", "y", "b"]);
    assert_eq!(moves(&applied), 2, "e, c and b reverse: one stays, two move");
    view.check(&d, "after reordering");
}

#[test]
fn a_counter_takes_the_value() {
    let (mut d, mut view) = open("checklist");
    apply(&mut d, &mut view, &replace(json!(["hits"]), json!(5)));
    assert_eq!(value(&d)["hits"], 5);
    apply(&mut d, &mut view, &json!({"intents":[{"type":"increment","path":["hits"],"by":2}]}).to_string());
    assert_eq!(value(&d)["hits"], 7);
    view.check(&d, "after counting on");
}

#[test]
fn records_optionals_and_scalar_lists_reconcile() {
    let (mut d, mut view) = open("collections");
    let mut target = value(&d);
    target["done"] = json!({"mon": true});
    target["widths"] = json!({"B": 120});
    target["cells"] = json!({"A1": {"input": "x", "tint": "red"}});
    target["pixels"] = json!(["#fff", "#000"]);
    target["notes"] = json!("Some notes");
    target["habits"][0]["checkins"] = json!({"2026-10-01": 1});
    apply(&mut d, &mut view, &replace(json!([]), target.clone()));
    assert_eq!(value(&d), target);
    view.check(&d, "after setting");
    // Absent optionals and record entries are removed; an object entry is reconciled.
    target["cells"] = json!({"A1": {"input": "y"}});
    target["widths"] = json!({});
    target.as_object_mut().unwrap().remove("notes");
    apply(&mut d, &mut view, &replace(json!([]), target.clone()));
    assert_eq!(value(&d), target);
    view.check(&d, "after removing");

    let (mut d, mut view) = open("scalars");
    let mut target = value(&d);
    target["memo"] = json!("note");
    target["box"] = json!({"items": [{"$id": "i1", "done": false}]});
    target["photo"] = json!({"id": "p", "name": "photo.png"});
    apply(&mut d, &mut view, &replace(json!([]), target.clone()));
    assert_eq!(value(&d), target);
    target["box"]["items"][0]["done"] = json!(true);
    target.as_object_mut().unwrap().remove("memo");
    apply(&mut d, &mut view, &replace(json!([]), target.clone()));
    assert_eq!(value(&d), target);
    view.check(&d, "after reconciling an optional object's rows");
}

#[test]
fn a_path_replaces_one_part() {
    let (mut d, mut view) = open("checklist");
    let before = value(&d);
    apply(&mut d, &mut view, &replace(json!(["rows", {"id": A}]), json!({"text": "Row A", "done": true})));
    apply(&mut d, &mut view, &replace(json!(["title"]), json!("Only the title")));
    let v = value(&d);
    assert_eq!(v["rows"][0], json!({"$id": A, "text": "Row A", "done": true}));
    assert_eq!((v["title"].clone(), v["rows"][1].clone()), (json!("Only the title"), before["rows"][1].clone()));
    let (mut d, mut view) = open("collections");
    apply(&mut d, &mut view, &replace(json!(["cells", "B2"]), json!({"input": "new"})));
    apply(&mut d, &mut view, &replace(json!(["pixels", {"index": 1}]), json!("#123")));
    let v = value(&d);
    assert_eq!((v["cells"]["B2"].clone(), v["pixels"][1].clone()), (json!({"input": "new"}), json!("#123")));
    view.check(&d, "after replacing parts");
}

#[test]
fn invalid_values_are_refused_whole() {
    let (mut d, _) = open("checklist");
    let mut target = value(&d);
    target["title"] = json!(5);
    assert_eq!(refused(&mut d, &replace(json!([]), target)), Code::TypeMismatch);
    let mut target = value(&d);
    target["rows"][1]["$id"] = json!(A);
    assert_eq!(refused(&mut d, &replace(json!([]), target)), Code::DuplicateId);
    let mut target = value(&d);
    target["extra"] = json!(1);
    assert_eq!(refused(&mut d, &replace(json!([]), target)), Code::TypeMismatch);
    assert_eq!(
        refused(&mut d, &replace(json!(["rows", {"id": A}]), json!({"$id": B, "text": "x", "done": false}))),
        Code::InvalidId
    );
    assert_eq!(
        refused(&mut d, &replace(json!(["rows", {"id": "missing"}]), json!({"text": "x", "done": false}))),
        Code::PathNotFound
    );
    // All or nothing with the rest of the batch.
    let batch = json!({"intents":[
        {"type":"set","path":["title"],"value":"Changed"},
        {"type":"replace","path":["rows"],"value":[{"$id": A, "text": 1, "done": false}]},
    ]});
    assert_eq!(refused(&mut d, &batch.to_string()), Code::TypeMismatch);
}

#[test]
fn an_import_is_one_undo_step() {
    let (mut d, mut view) = open("checklist");
    let before = value(&d);
    let mut target = before.clone();
    target["title"] = json!("Imported");
    target["rows"] = json!([{"text": "Only row", "done": true}]);
    target["hits"] = json!(4);
    apply(&mut d, &mut view, &replace(json!([]), target));
    let imported = value(&d);
    view.publish(&d.undo().unwrap().publication.unwrap());
    assert_eq!(value(&d), before);
    view.check(&d, "after undoing the import");
    view.publish(&d.redo().unwrap().publication.unwrap());
    assert_eq!(value(&d), imported, "redo keeps the imported row's ID");
    view.check(&d, "after redoing the import");
}

// A batch can mutate an earlier field before a later replace is refused. The rollback
// must preserve both existing undo and redo.
#[test]
fn a_late_refused_import_preserves_history() {
    let schema = json!({"kind":"object","properties":{"a":{"kind":"boolean"},"z":{"kind":"counter"}}}).to_string();
    let seed = Document::create(&app(&schema), &json!({"a":false,"z":9007199254740991i64}).to_string()).unwrap();
    let mut d = Document::open(&app(&schema), &seed.checkpoint().unwrap(), &[]).unwrap();
    let mut view = View::of(&d);
    apply(&mut d, &mut view, &replace(json!(["a"]), json!(true)));
    for a in [false, true] {
        let late = json!({"intents":[{"type":"set","path":["a"],"value":a},{"type":"replace","path":[],"value":{"a":a,"z":"many"}}]});
        assert_eq!(refused(&mut d, &late.to_string()), Code::TypeMismatch);
        if !a {
            assert!(d.can_undo(), "a refused import keeps undo");
            view.publish(&d.undo().unwrap().publication.unwrap());
        } else {
            assert!(d.can_redo(), "a refused import keeps redo");
            view.publish(&d.redo().unwrap().publication.unwrap());
        }
        view.check(&d, "history after refused import");
    }
    assert_eq!(value(&d), json!({"a":true,"z":9007199254740991i64}));
}
