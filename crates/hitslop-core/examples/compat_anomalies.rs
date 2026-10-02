//! Writes merged anomalies into a document of the conformance package, through the real
//! store, for the compatibility corpus (`bun run compat:capture`). The anomalies are the
//! kinds a replica merge can produce: a wrong type, an out-of-range value, an unknown
//! field, a reserved record key, duplicate row IDs and an invalid counter contribution.
//! Run with `cargo run -p hitslop-core --features storage --example compat_anomalies -- DOCUMENT`.
use hitslop_core::store::{Mode, Store};
use hitslop_core::Origin;
use loro::{ExportMode, LoroDoc, LoroMap, LoroMovableList};
use std::path::Path;

fn main() {
    let root = std::env::args().nth(1).expect("document path");
    let root = Path::new(&root);
    let read = |file: &str| std::fs::read_to_string(root.join(file)).unwrap();
    let initial = read("initial.json");
    let key = hitslop_core::validate(&read("state.schema.json"), &initial).unwrap();
    let store = Store::open(root, Mode::Document).unwrap();
    let mut doc = store.document(&key, &initial, &read("assets/theme.json")).unwrap();
    doc.apply_batch(
        r#"{"intents":[{"type":"insert","path":["rows"],"id":"dup","value":{"text":"Original","done":false,"tags":[],"notes":{}}}]}"#,
        Origin::Page,
    )
    .unwrap();
    let peer = LoroDoc::new();
    peer.import(&doc.checkpoint().unwrap()).unwrap();
    let from = peer.oplog_vv();
    let data = peer.get_map("data");
    data.insert("done", "invalid").unwrap();
    data.insert("count", 1000).unwrap();
    data.insert("extra", "preserved").unwrap();
    let checkins = match data.get("checkins") {
        Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) => map,
        _ => panic!("checkins"),
    };
    checkins.insert("__proto__", 1).unwrap();
    let hits = match data.get("hits") {
        Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) => map,
        _ => panic!("hits"),
    };
    hits.insert("peer", "not a number").unwrap();
    let rows = match data.get("rows") {
        Some(loro::ValueOrContainer::Container(loro::Container::MovableList(list))) => list,
        _ => panic!("rows"),
    };
    duplicate_row(&rows);
    peer.commit();
    doc.import(&peer.export(ExportMode::updates(&from)).unwrap()).unwrap();
    let job = store.job(&mut doc, false).unwrap().unwrap();
    store.write(&job).unwrap();
    store.close().unwrap();
    println!("{}", doc.snapshot().unwrap());
}

/// A second row claiming the first row's ID, as two replicas inserting it would.
fn duplicate_row(rows: &LoroMovableList) {
    let row = rows.insert_container(rows.len(), LoroMap::new()).unwrap();
    row.insert("$id", "dup").unwrap();
    row.insert_container("text", loro::LoroText::new()).unwrap().insert(0, "Duplicate").unwrap();
    row.insert("done", true).unwrap();
    row.insert_container("tags", LoroMovableList::new()).unwrap();
    row.insert_container("notes", LoroMap::new()).unwrap();
}
