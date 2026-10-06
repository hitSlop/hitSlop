// Failure: remote bytes that local writes would reject (other roots, plain lists,
// replaced containers, deleted fields, wrong types, duplicate or rewritten IDs,
// concurrent move/remove) desynchronize the published projection or panic.
// Oracle: independent patch consumer + a fresh full snapshot (value and issues).
// Gap: publications.rs peers are `Document`s and can only make valid edits.
mod support;
use hitslop_core::Document;
use support::{Edit, View, fixture, next, snapshot};
use loro::{
    Container, ExportMode, LoroDoc, LoroList, LoroMap, LoroMovableList, LoroText,
    ValueOrContainer,
};
use serde_json::{json, Value};

fn peer_of(d: &Document) -> LoroDoc {
    let peer = LoroDoc::new();
    peer.import(&d.checkpoint().unwrap()).unwrap();
    peer
}
/// Imports what `peer` did since it was forked and checks the publication.
fn deliver(d: &mut Document, projected: &mut View, peer: &LoroDoc, from: &loro::VersionVector) {
    let bytes = peer.export(ExportMode::updates(from)).unwrap();
    projected.publish(&d.merge(&bytes).unwrap());
    projected.check(d, "import");
    assert_eq!(serde_json::from_str::<Value>(&d.state().unwrap()).unwrap(), snapshot(d), "maintained state");
}
fn rows(peer: &LoroDoc) -> Option<LoroMovableList> {
    match peer.get_map("data").get("rows") {
        Some(ValueOrContainer::Container(Container::MovableList(l))) => Some(l),
        _ => None,
    }
}
fn row(list: &LoroMovableList, i: usize) -> Option<LoroMap> {
    match list.get(i) {
        Some(ValueOrContainer::Container(Container::Map(m))) => Some(m),
        _ => None,
    }
}

#[test]
fn other_roots_do_not_enter_the_projection() {
    let f = fixture("checklist");
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let mut projected = View::of(&d);
    let peer = peer_of(&d);
    let from = peer.oplog_vv();
    peer.get_map("unrelated").insert("done", true).unwrap();
    peer.get_map("unrelated").insert("title", "elsewhere").unwrap();
    peer.commit();
    deliver(&mut d, &mut projected, &peer, &from);
}

#[test]
fn plain_lists_publish_exactly_on_later_updates() {
    let f = fixture("checklist");
    let mut d = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let mut projected = View::of(&d);
    for target in ["extra", "rows"] {
        let peer = peer_of(&d);
        let from = peer.oplog_vv();
        let list = peer
            .get_map("data")
            .insert_container(target, LoroList::new())
            .unwrap();
        list.push("first").unwrap();
        peer.commit();
        deliver(&mut d, &mut projected, &peer, &from);
        // The next update to that same plain list must not assume a movable list.
        let peer = peer_of(&d);
        let from = peer.oplog_vv();
        match peer.get_map("data").get(target) {
            Some(ValueOrContainer::Container(Container::List(l))) => l.push("second").unwrap(),
            other => panic!("expected plain list, got {other:?}"),
        }
        peer.commit();
        deliver(&mut d, &mut projected, &peer, &from);
    }
}

#[test]
fn collection_anomalies_survive_publication_and_reopen() {
    let f: Value = fixture("collections");
    let schema = f["schema"].to_string();
    let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let mut projected = View::of(&d);
    // Successive imports exercise updates to already-observed containers as well
    // as replacement containers. None of these values may be repaired on read.
    for step in 0..3 {
        let peer = peer_of(&d);
        let from = peer.oplog_vv();
        let data = peer.get_map("data");
        let cells = match data.get("cells").unwrap() {
            ValueOrContainer::Container(Container::Map(map)) => map,
            other => panic!("expected record, got {other:?}"),
        };
        let done = match data.get("done").unwrap() {
            ValueOrContainer::Container(Container::Map(map)) => map,
            other => panic!("expected record, got {other:?}"),
        };
        done.insert("__proto__", true).unwrap();
        let notes = if step == 1 { loro::LoroValue::Bool(true) } else { loro::LoroValue::Null };
        data.insert("notes", notes).unwrap();
        if step == 0 {
            let pixels = data.insert_container("pixels", LoroList::new()).unwrap();
            pixels.push("valid").unwrap();
            pixels.push(false).unwrap();
            pixels.push_container(LoroMap::new()).unwrap().insert("unexpected", true).unwrap();
        } else {
            let pixels = data.insert_container("pixels", LoroMovableList::new()).unwrap();
            pixels.push("valid").unwrap();
            pixels.push(false).unwrap();
            pixels.push_container(LoroMap::new()).unwrap().insert("unexpected", true).unwrap();
        }
        if step == 1 {
            cells.insert("A1", 42).unwrap();
        } else {
            let cell = cells.insert_container("A1", LoroMap::new()).unwrap();
            cell.insert("input", "kept").unwrap();
            cell.insert("tint", loro::LoroValue::Null).unwrap();
        }
        peer.commit();
        deliver(&mut d, &mut projected, &peer, &from);
        let state = snapshot(&d);
        assert_eq!(state["value"]["done"]["__proto__"], true);
        assert_eq!(state["value"]["pixels"], json!(["valid", false, {"unexpected": true}]));
        assert!(state["value"].as_object().unwrap().contains_key("notes"));
        assert_eq!(state["value"]["notes"], if step == 1 { json!(true) } else { Value::Null });
        let issues = state["issues"].as_array().unwrap();
        for (code, path) in [
            ("invalid_key", json!(["done", "__proto__"])),
            ("type_mismatch", json!(["pixels", {"index": 1}])),
            ("type_mismatch", json!(["pixels", {"index": 2}])),
            ("type_mismatch", json!(["notes"])),
            ("type_mismatch", if step == 1 { json!(["cells", "A1"]) } else { json!(["cells", "A1", "tint"]) }),
        ] {
            assert!(issues.iter().any(|issue| issue["code"] == code && issue["path"] == path), "missing {code} at {path}: {issues:?}");
        }
        let reopened = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
        let fresh = snapshot(&reopened);
        assert_eq!(fresh["value"], state["value"]);
        assert_eq!(fresh["issues"], state["issues"]);
    }
}

/// One raw edit a well-behaved `Document` would refuse or never produce.
fn chaos(rng: &mut u64, peer: &LoroDoc, serial: &mut u64) {
    let data = peer.get_map("data");
    let list = rows(peer);
    let len = list.as_ref().map_or(0, |l| l.len());
    let i = if len == 0 { 0 } else { next(rng) as usize % len };
    *serial += 1;
    match next(rng) % 14 {
        0 => {
            if let Some(r) = list.as_ref().and_then(|l| row(l, i)) {
                r.delete("done").unwrap();
            } else {
                data.delete("title").unwrap();
            }
        }
        1 => {
            if let Some(r) = list.as_ref().and_then(|l| row(l, i)) {
                r.insert("done", "bad").unwrap();
            } else {
                data.insert("title", 5).unwrap();
            }
        }
        2 => {
            let fresh = data.insert_container("rows", LoroMovableList::new()).unwrap();
            for k in 0..2 {
                let r = fresh.insert_container(k, LoroMap::new()).unwrap();
                r.insert("$id", format!("fresh{serial}x{k}")).unwrap();
                r.insert("done", false).unwrap();
                r.insert_container("text", LoroText::new())
                    .unwrap()
                    .insert(0, "r")
                    .unwrap();
            }
        }
        3 => {
            if let Some(r) = list.as_ref().and_then(|l| row(l, i)) {
                r.insert_container("text", LoroText::new())
                    .unwrap()
                    .insert(0, "t")
                    .unwrap();
            } else {
                data.insert_container("title", LoroText::new())
                    .unwrap()
                    .insert(0, "T")
                    .unwrap();
            }
        }
        4 => {
            if let Some(l) = &list {
                l.insert(i.min(l.len()), "plain").unwrap();
            }
        }
        5 => {
            let extra = data.insert_container("extra", LoroList::new()).unwrap();
            extra.push(*serial as i64).unwrap();
        }
        6 => {
            peer.get_map("unrelated").insert("done", true).unwrap();
        }
        7 => {
            if let (Some(l), Some(r)) = (&list, list.as_ref().and_then(|l| row(l, i))) {
                if let Some(ValueOrContainer::Value(id)) = r.get("$id") {
                    let dup = l.insert_container(l.len(), LoroMap::new()).unwrap();
                    dup.insert("$id", id).unwrap();
                    dup.insert("done", true).unwrap();
                    dup.insert_container("text", LoroText::new()).unwrap();
                }
            }
        }
        8 => {
            if let Some(r) = list.as_ref().and_then(|l| row(l, i)) {
                r.insert("$id", if *serial % 2 == 0 { "has space".to_string() } else { format!("renamed{serial}") }).unwrap();
            }
        }
        9 => {
            if let Some(l) = &list {
                if l.len() > 1 {
                    l.mov(i, (i + 1) % l.len()).unwrap();
                }
            }
        }
        10 => {
            if let Some(l) = &list {
                if l.len() > 0 {
                    l.delete(i, 1).unwrap();
                }
            }
        }
        11 => {
            if let Some(r) = list.as_ref().and_then(|l| row(l, i)) {
                if let Some(ValueOrContainer::Container(Container::Text(t))) = r.get("text") {
                    t.insert(0, "z").unwrap();
                }
            }
        }
        12 => {
            if let Some(r) = list.as_ref().and_then(|l| row(l, i)) {
                r.insert("done", *serial % 2 == 0).unwrap();
            }
        }
        _ => {
            if let Some(l) = &list {
                let r = l.insert_container(i.min(l.len()), LoroMap::new()).unwrap();
                r.insert("$id", format!("new{serial}")).unwrap();
                r.insert("done", false).unwrap();
                r.insert_container("text", LoroText::new()).unwrap();
            }
        }
    }
}

#[test]
fn seeded_chaos_peer_imports_publish_exactly() {
    let f = fixture("checklist");
    let mut rng = 0xc4a05u64;
    let mut serial = 0u64;
    for _round in 0..support::workload("HITSLOP_CHAOS_ROUNDS", 300) {
        let mut d =
            Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
        let mut projected = View::of(&d);
        for step in 0..support::workload("HITSLOP_CHAOS_STEPS", 40) {
            if step % 5 == 4 {
                // Two peers from the same base: concurrent structural edits.
                let (a, b) = (peer_of(&d), peer_of(&d));
                let from = a.oplog_vv();
                if let (Some(la), Some(lb)) = (rows(&a), rows(&b)) {
                    if la.len() > 1 {
                        let i = next(&mut rng) as usize % la.len();
                        la.mov(i, (i + 1) % la.len()).unwrap();
                        lb.delete(i, 1).unwrap();
                    }
                }
                a.commit();
                b.commit();
                deliver(&mut d, &mut projected, &a, &from);
                deliver(&mut d, &mut projected, &b, &from);
            } else {
                let peer = peer_of(&d);
                let from = peer.oplog_vv();
                for _ in 0..1 + next(&mut rng) % 3 {
                    chaos(&mut rng, &peer, &mut serial);
                }
                peer.commit();
                deliver(&mut d, &mut projected, &peer, &from);
            }
            // Local edits still go through validation; a rejection publishes nothing.
            let view = snapshot(&d);
            if let Some(first) = view["value"]["rows"].as_array().and_then(|r| r.first()) {
                let op = json!({"intents":[{"type":"set","path":["rows",{"id":first["$id"]},"done"],"value":true}]});
                if let Ok(reply) = d.apply(&op.to_string()) {
                    projected.publish(&reply);
                    projected.check(&d, "local");
                }
            }
        }
    }
}

// Failure: a duplicated `$id` inside a nested list (a row's tags) must publish that
// list exactly, be flagged, and leave later local and remote edits exact.
#[test]
fn nested_duplicate_row_ids_publish_exactly_and_are_flagged() {
    let f: Value = fixture("nested");
    let schema = f["schema"].to_string();
    let mut d = Document::create(&schema, &f["initial"].to_string()).unwrap();
    let mut projected = View::of(&d);
    let tags = |peer: &LoroDoc| match row(&rows(peer).unwrap(), 0).unwrap().get("tags").unwrap() {
        ValueOrContainer::Container(Container::MovableList(list)) => list,
        other => panic!("expected tags list, got {other:?}"),
    };
    let peer = peer_of(&d);
    let from = peer.oplog_vv();
    let list = tags(&peer);
    let dup = list.insert_container(list.len(), LoroMap::new()).unwrap();
    dup.insert("$id", "tag1").unwrap();
    dup.insert_container("label", LoroText::new()).unwrap().insert(0, "copy").unwrap();
    dup.insert("on", true).unwrap();
    peer.commit();
    deliver(&mut d, &mut projected, &peer, &from);
    let state = snapshot(&d);
    let issues = state["issues"].as_array().unwrap();
    assert!(issues.iter().any(|issue| issue["code"] == "duplicate_id"
        && issue["path"].as_array().is_some_and(|path| path.contains(&json!("tags")))), "{issues:?}");
    // A local edit elsewhere and a remote edit inside the anomalous list stay exact.
    projected.publish(&d.apply(&json!({"intents":[
        {"type":"set","path":["rows",{"id":"row2"},"meta","pinned"],"value":false}
    ]}).to_string()).unwrap());
    projected.check(&d, "local");
    let peer = peer_of(&d);
    let from = peer.oplog_vv();
    row(&tags(&peer), 1).unwrap().insert("on", false).unwrap();
    peer.commit();
    deliver(&mut d, &mut projected, &peer, &from);
    let reopened = Document::open(&schema, &d.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(snapshot(&reopened)["value"], snapshot(&d)["value"]);
    assert_eq!(snapshot(&reopened)["issues"], snapshot(&d)["issues"]);
}
