//! Library storage/owner boundary, independent of authored templates and CloudKit.
use crate::support;
use hitslop_core::{
    Origin, file,
    library::{Issue, Library, snapshot},
    owner::{Owner, Reply, Request},
    store::Mode,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};

fn fixture() -> (tempfile::TempDir, PathBuf) {
    support::isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/ui.js"), "export default {}").unwrap();
    support::write_app(
        &stage,
        support::App::new(r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#, r#"{"title":"Initial"}"#),
    );
    let template = dir.path().join("template.slop");
    support::pack(&stage, &template).unwrap();
    (dir, template)
}
fn edit(owner: &Owner, intents: serde_json::Value) {
    snapshot::request(
        owner,
        Request::Apply { batch: serde_json::from_value(json!({"intents":intents})).unwrap(), origin: Origin::Window },
    )
    .unwrap();
    snapshot::request(owner, Request::Flush).unwrap();
}
fn close(owner: &Owner) {
    snapshot::request(owner, Request::Close { preview: None, icon: None }).unwrap();
}

#[test]
fn relay_checkpoints_throttle_automatic_staging_but_manual_saves_and_ordinary_documents_do_not() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.create(&template, "Checkpoint").unwrap();
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    assert!(library.stage_sync(&entry.id, false).unwrap().is_some());
    assert!(library.stage_allowed(&entry.id, false).unwrap());
    snapshot::request(
        &owner,
        Request::Share {
            share: hitslop_core::store::Share { room: "relay".into(), endpoint: "https://relay.example".into() },
        },
    )
    .unwrap();
    assert!(!library.stage_allowed(&entry.id, false).unwrap());
    assert!(library.stage_sync(&entry.id, false).unwrap().is_none());
    assert!(library.stage_sync(&entry.id, true).unwrap().is_some());
    assert!(!library.stage_allowed(&entry.id, false).unwrap());
    close(&owner);
    drop(library);
    let reopened = Library::open(&dir.path().join("library"), "account").unwrap();
    assert!(reopened.stage_allowed(&entry.id, false).unwrap());
    assert_eq!(std::fs::read_dir(dir.path().join("library/uploads")).unwrap().count(), 0);
}

#[test]
fn merges_use_the_live_owner_publish_and_acknowledge_the_actual_uploaded_version() {
    let (dir, template) = fixture();
    let left = Library::open(&dir.path().join("left"), "account").unwrap();
    let right = Library::open(&dir.path().join("right"), "account").unwrap();
    let entry = left.create(&template, "A synced document").unwrap();
    let seed = left.stage(&entry.id).unwrap();
    right
        .receive(
            &entry.id,
            &entry.title,
            &entry.template,
            seed.asset_path.as_deref().map(std::path::Path::new),
            &seed.entry.version,
            &[1],
            false,
        )
        .unwrap();
    left.acknowledge(&entry.id, &seed.entry.version, &[1], false).unwrap();
    let a = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    let bpath = right.entries().unwrap()[0].path.clone();
    let b = Arc::new(Owner::open(std::path::Path::new(&bpath), Mode::Document, Arc::new(|_| {})).unwrap());
    left.register(&a).unwrap();
    right.register(&b).unwrap();
    edit(&a, json!([{"type":"set","path":["title"],"value":"Mac edit"}]));
    edit(&b, json!([{"type":"setTheme","values":{"accent":"#123456"}}]));
    let sa = left.stage(&entry.id).unwrap();
    let sb = right.stage(&entry.id).unwrap();
    left.receive(
        &entry.id,
        &entry.title,
        &entry.template,
        sb.asset_path.as_deref().map(std::path::Path::new),
        &sb.entry.version,
        &[2],
        false,
    )
    .unwrap();
    right
        .receive(
            &entry.id,
            &entry.title,
            &entry.template,
            sa.asset_path.as_deref().map(std::path::Path::new),
            &sa.entry.version,
            &[2],
            false,
        )
        .unwrap();
    let Reply::State { reading, .. } = snapshot::request(&a, Request::State).unwrap() else { panic!("state") };
    assert_eq!(reading.value["title"], "Mac edit");
    assert_eq!(reading.theme["accent"], "#123456");
    assert_eq!(
        snapshot::inspect(std::path::Path::new(&entry.path)).unwrap().fingerprint,
        snapshot::inspect(std::path::Path::new(&bpath)).unwrap().fingerprint
    );
    // A late acknowledgment never hides edits accepted after the staged snapshot.
    left.acknowledge(&entry.id, &sa.entry.version, &[3], false).unwrap();
    assert!(left.entries().unwrap()[0].dirty);
    close(&a);
    close(&b);
    drop(left);
    let reopened = Library::open(&dir.path().join("left"), "account").unwrap();
    assert!(reopened.entries().unwrap()[0].dirty);
    assert!(Library::open(&dir.path().join("left"), "different-account").is_err());
}

#[test]
fn deletion_waits_for_editor_close_and_rescues_unsent_edits_without_resurrection() {
    let (dir, template) = fixture();
    let left = Library::open(&dir.path().join("left"), "account").unwrap();
    let right = Library::open(&dir.path().join("right"), "account").unwrap();
    let entry = left.create(&template, "Shared").unwrap();
    let seed = left.stage(&entry.id).unwrap();
    right
        .receive(
            &entry.id,
            &entry.title,
            &entry.template,
            seed.asset_path.as_deref().map(std::path::Path::new),
            &seed.entry.version,
            &[1],
            false,
        )
        .unwrap();
    let path = PathBuf::from(&right.entries().unwrap()[0].path);
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    right.register(&owner).unwrap();
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Keep my offline edits"}]));
    left.delete(&entry.id).unwrap();
    let tombstone = left.stage(&entry.id).unwrap();
    assert!(tombstone.entry.deleted);
    assert!(tombstone.asset_path.is_none());
    right.receive(&entry.id, &entry.title, &entry.template, None, "", &[2], true).unwrap();
    right.cleanup().unwrap();
    assert!(path.exists(), "an open editor retains its backing file");
    close(&owner);
    right.cleanup().unwrap();
    assert!(!path.exists());
    let rescued = right.entries().unwrap().into_iter().find(|e| e.local_only).unwrap();
    assert_ne!(rescued.id, entry.id);
    assert!(!rescued.dirty);
    let saved = hitslop_core::store::Store::open(std::path::Path::new(&rescued.path), Mode::Snapshot).unwrap();
    assert_eq!(saved.document().unwrap().reading().unwrap().value["title"], "Keep my offline edits");
    left.receive(
        &entry.id,
        &entry.title,
        &entry.template,
        seed.asset_path.as_deref().map(std::path::Path::new),
        &seed.entry.version,
        &[3],
        false,
    )
    .unwrap();
    assert!(left.entries().unwrap()[0].deleted);
    assert!(left.stage(&entry.id).unwrap().entry.deleted);
}

#[test]
fn closed_cli_writer_is_not_bypassed_and_oversize_uploads_keep_the_document() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.create(&template, "Shared").unwrap();
    let owner = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
    assert_eq!(library.stage(&entry.id).err().unwrap().issue, Issue::Busy);
    close(&owner);
    assert!(library.stage(&entry.id).is_ok());
    let huge = dir.path().join("too-large.slop");
    std::fs::copy(&entry.path, &huge).unwrap();
    std::fs::OpenOptions::new().write(true).open(&huge).unwrap().set_len((16 << 20) + 1).unwrap();
    assert_eq!(
        library.receive(&entry.id, "Shared", "fixture", Some(&huge), "", &[], false).unwrap_err().issue,
        Issue::TooLarge
    );
    assert!(file::open(std::path::Path::new(&entry.path), true).is_ok());
}

#[test]
fn unsupported_download_is_visible_without_replacing_local_state_or_blocking_other_documents() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.create(&template, "Existing").unwrap();
    let seed = library.stage(&entry.id).unwrap();
    let raised = dir.path().join("newer.slop");
    std::fs::copy(seed.asset_path.as_ref().unwrap(), &raised).unwrap();
    let database = rusqlite::Connection::open(&raised).unwrap();
    database.execute_batch("PRAGMA user_version=999;").unwrap();
    drop(database);
    assert_eq!(
        library
            .receive(&entry.id, "Existing", "fixture", Some(&raised), &seed.entry.version, &[], false)
            .unwrap_err()
            .issue,
        Issue::RequiresUpdate
    );
    library.block(&entry.id, &entry.title, &entry.template, "Update hitSlop to sync this document").unwrap();
    let blocked = library.entries().unwrap().into_iter().find(|e| e.id == entry.id).unwrap();
    assert!(blocked.problem.is_some());
    assert!(!blocked.dirty);
    assert!(file::open(std::path::Path::new(&entry.path), true).is_ok());
    let other = library.create(&template, "Other").unwrap();
    assert!(library.stage(&other.id).is_ok());
    // A placeholder for a not-yet-installed document can be retried after upgrading.
    let receiver = Library::open(&dir.path().join("receiver"), "account").unwrap();
    receiver.block(&entry.id, &entry.title, &entry.template, "Update required").unwrap();
    assert_eq!(receiver.entries().unwrap().len(), 1);
    receiver
        .receive(
            &entry.id,
            &entry.title,
            &entry.template,
            seed.asset_path.as_deref().map(std::path::Path::new),
            &seed.entry.version,
            &[],
            false,
        )
        .unwrap();
    assert!(receiver.entries().unwrap()[0].problem.is_none());
}

fn image(color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&color.repeat(512 * 512)).unwrap();
    }
    bytes
}
fn deliver(library: &Library, upload: &hitslop_core::library::Upload) {
    let e = &upload.entry;
    library
        .receive_with_artwork(
            &e.id,
            &e.title,
            &e.template,
            upload.asset_path.as_deref().map(std::path::Path::new),
            &e.version,
            &[1],
            e.deleted,
            Some(&e.artwork_version),
            Some(&e.asset_revision),
        )
        .unwrap();
}

#[test]
fn snapshot_delivery_preserves_the_live_owners_relay_binding() {
    let (dir, template) = fixture();
    let left = Library::open(&dir.path().join("left"), "account").unwrap();
    let right = Library::open(&dir.path().join("right"), "account").unwrap();
    let entry = left.create(&template, "Shared document").unwrap();
    deliver(&right, &left.stage(&entry.id).unwrap());
    let path = PathBuf::from(&right.entries().unwrap()[0].path);
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    right.register(&owner).unwrap();
    snapshot::request(
        &owner,
        Request::Share {
            share: hitslop_core::store::Share {
                room: "local-relay-room".into(),
                endpoint: "https://relay.example.test".into(),
            },
        },
    )
    .unwrap();
    let before = owner.share().unwrap().unwrap();
    // The downloaded file still carries the CloudKit retention placeholder. Neither
    // an open-owner merge nor a later closed-owner merge may replace local membership.
    let upload = left.stage(&entry.id).unwrap();
    deliver(&right, &upload);
    assert_eq!(owner.share().unwrap().unwrap(), before);
    close(&owner);
    deliver(&right, &upload);
    let reopened = Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap();
    assert_eq!(reopened.share().unwrap().unwrap(), before);
    close(&reopened);
}

#[test]
fn artwork_only_updates_sync_and_a_late_receipt_cannot_hide_newer_artwork() {
    let (dir, template) = fixture();
    let left = Library::open(&dir.path().join("left"), "account").unwrap();
    let right = Library::open(&dir.path().join("right"), "account").unwrap();
    let entry = left.create(&template, "  A named slop  ").unwrap();
    assert_eq!(entry.title, "A named slop");
    let seed = left.stage(&entry.id).unwrap();
    deliver(&right, &seed);
    left.acknowledge_asset(&entry.id, &seed.entry.version, Some(&seed.entry.asset_revision), &[1], false).unwrap();
    let a = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    let bpath = PathBuf::from(&right.entries().unwrap()[0].path);
    let b = Arc::new(Owner::open(&bpath, Mode::Document, Arc::new(|_| {})).unwrap());
    left.register(&a).unwrap();
    right.register(&b).unwrap();
    let red = image([255, 0, 0, 255]);
    left.publish_artwork(&a, &entry.version, Some(red.clone()), Some(red)).unwrap();
    let first = left.stage(&entry.id).unwrap();
    assert_eq!(first.entry.version, seed.entry.version, "artwork does not edit the CRDT");
    assert_ne!(first.entry.asset_revision, seed.entry.asset_revision);
    deliver(&right, &first);
    assert!(!right.entries().unwrap()[0].dirty);
    let blue = image([0, 0, 255, 255]);
    left.publish_artwork(&a, &entry.version, Some(blue.clone()), Some(blue)).unwrap();
    let second = left.stage(&entry.id).unwrap();
    left.acknowledge_asset(&entry.id, &first.entry.version, Some(&first.entry.asset_revision), &[2], false).unwrap();
    assert!(left.entries().unwrap()[0].dirty);
    deliver(&right, &second);
    assert_eq!(
        file::artwork(std::path::Path::new(&entry.path), &[file::Artwork::Icon]).unwrap(),
        file::artwork(&bpath, &[file::Artwork::Icon]).unwrap()
    );
    assert_eq!(right.entries().unwrap()[0].asset_revision, second.entry.asset_revision);
    assert!(!right.entries().unwrap()[0].dirty);
    // A completed image cannot overwrite edits accepted after the capture source.
    edit(&b, json!([{"type":"set","path":["title"],"value":"Newer local edits"}]));
    let before = file::artwork(&bpath, &[file::Artwork::Icon]).unwrap();
    assert!(right.publish_artwork(&b, &entry.version, Some(image([0, 255, 0, 255])), None).is_err());
    deliver(&right, &first);
    assert_eq!(file::artwork(&bpath, &[file::Artwork::Icon]).unwrap(), before);
    let merged = right.stage(&entry.id).unwrap();
    assert!(merged.entry.artwork_version.is_empty(), "old artwork is only a fallback after new edits");
    deliver(&left, &merged);
    assert!(!left.entries().unwrap()[0].dirty, "different stale images never cause a sync loop");
    close(&a);
    close(&b);
}
#[test]
fn empty_names_and_invalid_artwork_do_not_create_or_damage_documents() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    assert!(library.create(&template, "  ").is_err());
    assert!(library.entries().unwrap().is_empty());
    let entry = library.create(&template, "Checklist").unwrap();
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    assert!(library.publish_artwork(&owner, &entry.version, Some(vec![1, 2, 3]), None).is_err());
    close(&owner);
    assert_eq!(snapshot::inspect(std::path::Path::new(&entry.path)).unwrap().version, entry.version);
}

#[test]
fn a_legacy_data_receipt_does_not_hide_the_first_artwork_refresh() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Existing preview document").unwrap();
    library.acknowledge(&entry.id, &entry.version, &[], false).unwrap();
    // The previous preview tracked only the confirmed Loro version.
    rusqlite::Connection::open(root.join("library.sqlite"))
        .unwrap()
        .execute("UPDATE documents SET confirmed_asset='',artwork_version=''", [])
        .unwrap();
    assert!(!library.entries().unwrap()[0].dirty);
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.publish_artwork(&owner, &entry.version, Some(image([80, 90, 100, 255])), None).unwrap();
    assert!(library.entries().unwrap()[0].dirty);
    close(&owner);
}

#[test]
fn ui_only_builds_merge_data_and_theme_without_replacing_the_local_app() {
    let (dir, template) = fixture();
    let left = Library::open(&dir.path().join("left"), "account").unwrap();
    let right = Library::open(&dir.path().join("right"), "account").unwrap();
    let entry = left.create(&template, "Compatible builds").unwrap();
    let seed = left.stage(&entry.id).unwrap();
    right
        .receive(
            &entry.id,
            &entry.title,
            &entry.template,
            seed.asset_path.as_deref().map(std::path::Path::new),
            &seed.entry.version,
            &[],
            false,
        )
        .unwrap();
    let right_path = PathBuf::from(&right.entries().unwrap()[0].path);
    let conn = rusqlite::Connection::open(&right_path).unwrap();
    conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false).unwrap();
    conn.execute("UPDATE app SET title='A refreshed UI'", []).unwrap();
    conn.execute(
        "UPDATE assets SET bytes=?1, size=?2, encoding='identity' WHERE key='ui.js'",
        rusqlite::params![
            b"export default { refreshed: true }".as_slice(),
            b"export default { refreshed: true }".len() as i64
        ],
    )
    .unwrap();
    drop(conn);
    let local_digest = file::inspect(std::path::Path::new(&entry.path)).unwrap().app_digest;
    assert_ne!(file::inspect(&right_path).unwrap().app_digest, local_digest);
    let owner = Arc::new(Owner::open(&right_path, Mode::Document, Arc::new(|_| {})).unwrap());
    right.register(&owner).unwrap();
    edit(
        &owner,
        json!([{"type":"set","path":["title"],"value":"An edit from the other build"}, {"type":"setTheme","values":{"accent":"#123456"}}]),
    );
    let version = snapshot::inspect(&right_path).unwrap().version;
    right.publish_artwork(&owner, &version, Some(image([30, 150, 90, 255])), None).unwrap();
    let upload = right.stage(&entry.id).unwrap();
    deliver(&left, &upload);
    assert!(
        !left.entries().unwrap()[0].dirty,
        "a compatible UI build must not echo a different artwork revision forever"
    );
    let local = hitslop_core::store::Store::open(std::path::Path::new(&entry.path), Mode::Snapshot).unwrap();
    let reading = local.document().unwrap().reading().unwrap();
    assert_eq!(reading.value["title"], "An edit from the other build");
    assert!(serde_json::to_string(&reading.theme).unwrap().contains("#123456"));
    assert_eq!(file::inspect(std::path::Path::new(&entry.path)).unwrap().app_digest, local_digest);
    close(&owner);
}

#[test]
fn incompatible_schemas_theme_tokens_and_newer_markers_preserve_local_edits() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.create(&template, "Keep local").unwrap();
    let seed = library.stage(&entry.id).unwrap();
    let source = PathBuf::from(seed.asset_path.unwrap());
    for kind in ["schema", "theme", "defaults", "marker"] {
        let incoming = dir.path().join(format!("{kind}.slop"));
        std::fs::copy(&source, &incoming).unwrap();
        let conn = rusqlite::Connection::open(&incoming).unwrap();
        conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false).unwrap();
        if kind == "marker" {
            conn.execute("UPDATE app SET runtime_abi=?", [(hitslop_core::RUNTIME_ABI + 1) as i64]).unwrap();
        } else {
            let definition: String = conn.query_row("SELECT definition_json FROM app", [], |row| row.get(0)).unwrap();
            let mut definition: serde_json::Value = serde_json::from_str(&definition).unwrap();
            if kind == "schema" {
                definition["document"]["properties"]["title"]["maxLength"] = json!(128);
            } else if kind == "defaults" {
                definition["theme"][0]["color"] = json!("#ffffff");
            } else {
                definition["theme"].as_array_mut().unwrap().push(json!({"token":"anotherToken","color":"#ffffff"}));
            }
            conn.execute("UPDATE app SET definition_json=?", [definition.to_string()]).unwrap();
        }
        drop(conn);
        let before = std::fs::read(&entry.path).unwrap();
        let result =
            library.receive(&entry.id, &entry.title, &entry.template, Some(&incoming), &seed.entry.version, &[], false);
        assert!(result.is_err(), "{kind} must refuse");
        if kind == "marker" {
            assert_eq!(result.unwrap_err().issue, Issue::RequiresUpdate);
        }
        assert_eq!(std::fs::read(&entry.path).unwrap(), before, "{kind} must not write the local file");
    }
}
