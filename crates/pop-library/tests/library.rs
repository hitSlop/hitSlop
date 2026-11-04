//! Library storage/owner boundary, independent of authored templates and CloudKit.
mod support;
use pop_core::Origin;
use pop_library::{Issue, Library, snapshot};
use pop_runtime::owner::{Owner, Reply, Request};
use pop_storage::{file, store::Mode};
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
    let template = dir.path().join("template.pop");
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
fn removing_downloads_keeps_cloud_metadata_and_restores_only_on_request() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "A synced document").unwrap();
    let path = std::path::Path::new(&entry.path);
    let before = std::fs::read(path).unwrap();
    assert!(library.remove_download(&entry.id).is_err(), "unsynced data must stay local");
    assert_eq!(std::fs::read(path).unwrap(), before);
    let upload = library.stage(&entry.id).unwrap();
    library
        .acknowledge_asset(&entry.id, &upload.entry.version, Some(&upload.entry.asset_revision), &[1], false, None)
        .unwrap();
    let cloud = dir.path().join("cloud.pop");
    std::fs::copy(upload.asset_path.as_ref().unwrap(), &cloud).unwrap();
    let owner = Owner::open(path, Mode::Document, Arc::new(|_| {})).unwrap();
    assert_eq!(library.remove_download(&entry.id).unwrap_err().issue, Issue::Busy);
    assert!(path.exists());
    close(&owner);
    library.remove_download(&entry.id).unwrap();
    library.remove_download(&entry.id).unwrap();
    assert!(!path.exists());
    let removed = &library.entries().unwrap()[0];
    assert!(!removed.downloaded && !removed.deleted && !removed.dirty);
    drop(library);

    let library = Library::open(&root, "account").unwrap();
    assert!(!library.entries().unwrap()[0].downloaded);
    assert!(
        library
            .receive_removed_metadata(
                &entry.id,
                "Renamed in iCloud",
                &entry.template,
                &upload.entry.version,
                &[2],
                Some(&upload.entry.artwork_version),
                Some(&upload.entry.asset_revision),
                None
            )
            .unwrap()
    );
    assert_eq!(library.entries().unwrap()[0].title, "Renamed in iCloud");
    assert!(!path.exists(), "a metadata update never restores the file");
    library
        .receive_with_artwork(
            &entry.id,
            "Renamed in iCloud",
            &entry.template,
            Some(&cloud),
            &upload.entry.version,
            &[2],
            false,
            Some(&upload.entry.artwork_version),
            Some(&upload.entry.asset_revision),
            None,
        )
        .unwrap();
    let restored = &library.entries().unwrap()[0];
    assert!(restored.downloaded && !restored.dirty);
    assert_eq!(snapshot::inspect(path).unwrap().id, entry.id);

    library.remove_download(&entry.id).unwrap();
    library.delete(&entry.id).unwrap();
    let tombstone = library.stage(&entry.id).unwrap();
    assert!(tombstone.entry.deleted && tombstone.asset_path.is_none());
    assert!(
        !library
            .receive_removed_metadata(
                &entry.id,
                "Stale record",
                &entry.template,
                &upload.entry.version,
                &[3],
                None,
                None,
                None
            )
            .unwrap()
    );
    library
        .receive(&entry.id, &entry.title, &entry.template, Some(&cloud), &upload.entry.version, &[3], false, None)
        .unwrap();
    assert!(library.entries().unwrap()[0].deleted);
    assert!(!path.exists(), "a stale cloud record cannot restore a deleted document");
}

#[test]
fn relay_authority_never_uploads_cloudkit_bytes_even_on_explicit_sync() {
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
            share: pop_storage::store::Share { room: "relay".into(), endpoint: "https://relay.example".into() },
        },
    )
    .unwrap();
    assert!(!library.stage_allowed(&entry.id, false).unwrap());
    assert!(library.stage_sync(&entry.id, false).unwrap().is_none());
    assert!(library.stage_sync(&entry.id, true).unwrap().is_none());
    assert!(library.stage(&entry.id).is_err());
    assert!(!library.stage_allowed(&entry.id, false).unwrap());
    close(&owner);
    // The closed editor can outlive its close; its saved file still answers.
    assert!(!library.stage_allowed(&entry.id, false).unwrap());
    drop(library);
    let reopened = Library::open(&dir.path().join("library"), "account").unwrap();
    assert!(!reopened.stage_allowed(&entry.id, false).unwrap());
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
            None,
        )
        .unwrap();
    left.acknowledge(&entry.id, &seed.entry.version, &[1], false, None).unwrap();
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
        None,
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
            None,
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
    left.acknowledge(&entry.id, &sa.entry.version, &[3], false, None).unwrap();
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
            None,
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
    right.receive(&entry.id, &entry.title, &entry.template, None, "", &[2], true, None).unwrap();
    right.cleanup().unwrap();
    assert!(path.exists(), "an open editor retains its backing file");
    close(&owner);
    right.cleanup().unwrap();
    assert!(!path.exists());
    let rescued = right.entries().unwrap().into_iter().find(|e| e.local_only).unwrap();
    assert_ne!(rescued.id, entry.id);
    assert!(!rescued.dirty);
    let saved = pop_storage::store::Store::open(std::path::Path::new(&rescued.path), Mode::Snapshot).unwrap();
    assert_eq!(saved.document().unwrap().reading().unwrap().value["title"], "Keep my offline edits");
    drop(saved);
    left.receive(
        &entry.id,
        &entry.title,
        &entry.template,
        seed.asset_path.as_deref().map(std::path::Path::new),
        &seed.entry.version,
        &[3],
        false,
        None,
    )
    .unwrap();
    assert!(left.entries().unwrap()[0].deleted);
    assert!(left.stage(&entry.id).unwrap().entry.deleted);

    right.delete(&rescued.id).unwrap();
    assert!(!std::path::Path::new(&rescued.path).exists(), "deleting a local-only rescue must reclaim its file");
    right.delete(&rescued.id).unwrap();
    drop(right);
    let reopened = Library::open(&dir.path().join("right"), "account").unwrap();
    let deleted = reopened.entries().unwrap().into_iter().find(|entry| entry.id == rescued.id).unwrap();
    assert!(deleted.deleted && deleted.local_only && !deleted.dirty);
    reopened.cleanup().unwrap();
    assert!(!std::path::Path::new(&rescued.path).exists());
}

#[test]
fn local_only_deletion_preserves_busy_files_and_cleans_legacy_tombstones() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Shared").unwrap();
    let seed = library.stage(&entry.id).unwrap();
    library.acknowledge(&entry.id, &seed.entry.version, &[1], false, None).unwrap();
    let owner = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Keep my offline edits"}]));
    close(&owner);
    library.receive(&entry.id, &entry.title, &entry.template, None, "", &[2], true, None).unwrap();
    library.cleanup().unwrap();
    let rescued = library.entries().unwrap().into_iter().find(|entry| entry.local_only).unwrap();
    let path = std::path::Path::new(&rescued.path);
    let owner = Owner::open(path, Mode::Document, Arc::new(|_| {})).unwrap();
    let before = std::fs::read(path).unwrap();
    assert_eq!(library.delete(&rescued.id).unwrap_err().issue, Issue::Busy);
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(!library.entries().unwrap().into_iter().find(|entry| entry.id == rescued.id).unwrap().deleted);

    // Older builds hid local-only rows without ever confirming a cloud tombstone.
    rusqlite::Connection::open(root.join("library.sqlite"))
        .unwrap()
        .execute("UPDATE documents SET deleted=1 WHERE id=?1", [&rescued.id])
        .unwrap();
    assert_eq!(library.cleanup().unwrap_err().issue, Issue::Busy);
    assert_eq!(std::fs::read(path).unwrap(), before);
    close(&owner);
    library.cleanup().unwrap();
    assert!(!path.exists());
    let deleted = library.entries().unwrap().into_iter().find(|entry| entry.id == rescued.id).unwrap();
    assert!(deleted.deleted && !deleted.dirty);
    library.cleanup().unwrap();
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
    let huge = dir.path().join("too-large.pop");
    std::fs::copy(&entry.path, &huge).unwrap();
    std::fs::OpenOptions::new().write(true).open(&huge).unwrap().set_len((16 << 20) + 1).unwrap();
    assert_eq!(
        library.receive(&entry.id, "Shared", "fixture", Some(&huge), "", &[], false, None).unwrap_err().issue,
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
    let raised = dir.path().join("newer.pop");
    std::fs::copy(seed.asset_path.as_ref().unwrap(), &raised).unwrap();
    let database = rusqlite::Connection::open(&raised).unwrap();
    database.execute_batch("PRAGMA user_version=999;").unwrap();
    drop(database);
    assert_eq!(
        library
            .receive(&entry.id, "Existing", "fixture", Some(&raised), &seed.entry.version, &[], false, None)
            .unwrap_err()
            .issue,
        Issue::RequiresUpdate
    );
    library.block(&entry.id, &entry.title, &entry.template, "Update pop to sync this document").unwrap();
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
            None,
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
fn deliver(library: &Library, upload: &pop_library::Upload) {
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
            None,
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
            share: pop_storage::store::Share {
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
    let entry = left.create(&template, "  A named pop  ").unwrap();
    assert_eq!(entry.title, "A named pop");
    let seed = left.stage(&entry.id).unwrap();
    deliver(&right, &seed);
    left.acknowledge_asset(&entry.id, &seed.entry.version, Some(&seed.entry.asset_revision), &[1], false, None)
        .unwrap();
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
    left.acknowledge_asset(&entry.id, &first.entry.version, Some(&first.entry.asset_revision), &[2], false, None)
        .unwrap();
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
    library.acknowledge(&entry.id, &entry.version, &[], false, None).unwrap();
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
            None,
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
    let local = pop_storage::store::Store::open(std::path::Path::new(&entry.path), Mode::Snapshot).unwrap();
    let reading = local.document().unwrap().reading().unwrap();
    assert_eq!(reading.value["title"], "An edit from the other build");
    assert!(serde_json::to_string(&reading.theme).unwrap().contains("#123456"));
    assert_eq!(file::inspect(std::path::Path::new(&entry.path)).unwrap().app_digest, local_digest);

    // The same state contract must survive an authority cutoff as well as an
    // ordinary history merge, while each device retains its own UI bundle.
    let source_version = snapshot::inspect(&right_path).unwrap().version;
    let candidate = dir.path().join("compatible-build-cutoff.pop");
    snapshot::request(
        &owner,
        Request::PrepareEpoch {
            destination: pop_storage::store::EpochLocation::Path(candidate.clone()),
            expected_version: source_version.clone(),
            required_heads: None,
        },
    )
    .unwrap();
    let fresh = snapshot::inspect(&candidate).unwrap();
    let authority = pop_library::CatalogAuthority::Checkpoint {
        proof: pop_library::CatalogCheckpointProof {
            transition: "d".repeat(32),
            source_version: source_version.clone(),
            version: fresh.version.clone(),
        },
    };
    left.receive_catalog_snapshot(
        &entry.id,
        &entry.title,
        &entry.template,
        Some(&candidate),
        &fresh.version,
        b"compatible-build-cutoff",
        false,
        None,
        None,
        authority,
        Some(&next_catalog_stamp(&left, &entry.id)),
    )
    .expect("a proven cutoff accepts a compatible state contract across UI revisions");
    let installed = pop_storage::store::Store::open(std::path::Path::new(&entry.path), Mode::Snapshot).unwrap();
    let installed_reading = installed.document().unwrap().reading().unwrap();
    assert_eq!(installed_reading.value, reading.value);
    assert_eq!(installed_reading.theme, reading.theme);
    assert_eq!(installed.document().unwrap().version(), fresh.version);
    assert_eq!(file::inspect(std::path::Path::new(&entry.path)).unwrap().app_digest, local_digest);
    assert_eq!(snapshot::inspect(&right_path).unwrap().version, source_version);
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
        let incoming = dir.path().join(format!("{kind}.pop"));
        std::fs::copy(&source, &incoming).unwrap();
        let conn = rusqlite::Connection::open(&incoming).unwrap();
        conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false).unwrap();
        if kind == "marker" {
            conn.execute("UPDATE app SET runtime_abi=?", [(pop_core::RUNTIME_ABI + 1) as i64]).unwrap();
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
        let result = library.receive(
            &entry.id,
            &entry.title,
            &entry.template,
            Some(&incoming),
            &seed.entry.version,
            &[],
            false,
            None,
        );
        assert!(result.is_err(), "{kind} must refuse");
        if kind == "marker" {
            assert_eq!(result.unwrap_err().issue, Issue::RequiresUpdate);
        }
        assert_eq!(std::fs::read(&entry.path).unwrap(), before, "{kind} must not write the local file");
    }
}

#[test]
fn adopt_preserves_identity_work_and_source_and_schedules_upload() {
    let (dir, template) = fixture();
    let source = dir.path().join("local.pop");
    file::create_document(&template, &source).unwrap();
    let owner = Owner::open(&source, Mode::Document, Arc::new(|_| {})).unwrap();
    edit(
        &owner,
        json!([{"type":"set","path":["title"],"value":"My saved work"}, {"type":"setTheme","values":{"accent":"#123456"}}]),
    );
    close(&owner);
    let before = std::fs::read(&source).unwrap();
    let info = snapshot::inspect(&source).unwrap();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.adopt(&source, "  My pop  ").unwrap();
    assert_eq!(entry.id, info.id);
    assert_eq!(entry.title, "My pop");
    assert!(entry.dirty);
    assert_eq!(library.entries().unwrap()[0].id, info.id);
    assert_eq!(snapshot::inspect(std::path::Path::new(&entry.path)).unwrap().fingerprint, info.fingerprint);
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let adopted = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
    let share = adopted.share().unwrap().unwrap();
    assert_eq!(share.room, info.id);
    assert_eq!(share.endpoint, pop_storage::LIBRARY_SHARE_ENDPOINT);
    close(&adopted);
    assert!(library.stage(&entry.id).unwrap().asset_path.is_some());
    assert!(library.adopt(&source, "Again").is_err());
    assert_eq!(library.entries().unwrap().len(), 1);
}

#[test]
fn adopt_metadata_failure_rolls_back_the_copy_and_allows_retry() {
    let (dir, template) = fixture();
    let source = dir.path().join("local.pop");
    file::create_document(&template, &source).unwrap();
    let before = std::fs::read(&source).unwrap();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let db = rusqlite::Connection::open(root.join("library.sqlite")).unwrap();
    // Fail after the row is inserted, at the real metadata write boundary.
    db.execute_batch(
        "CREATE TRIGGER refuse_artwork BEFORE UPDATE OF artwork_version ON documents
        BEGIN SELECT RAISE(ABORT, 'metadata unavailable'); END;",
    )
    .unwrap();
    assert!(library.adopt(&source, "Retry me").is_err());
    assert!(library.entries().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(root.join("documents")).unwrap().count(), 0);
    assert_eq!(std::fs::read(&source).unwrap(), before);
    db.execute_batch("DROP TRIGGER refuse_artwork").unwrap();
    let entry = library.adopt(&source, "Retry me").unwrap();
    assert_eq!(library.entries().unwrap()[0].id, entry.id);
}

#[test]
fn adopt_refuses_busy_shared_invalid_and_template_files_without_writing() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let source = dir.path().join("local.pop");
    file::create_document(&template, &source).unwrap();
    let owner = Owner::open(&source, Mode::Document, Arc::new(|_| {})).unwrap();
    assert_eq!(library.adopt(&source, "Busy").unwrap_err().issue, Issue::Busy);
    snapshot::request(
        &owner,
        Request::Share {
            share: pop_storage::store::Share {
                room: "relay-room".into(),
                endpoint: "https://relay.example.test".into(),
            },
        },
    )
    .unwrap();
    close(&owner);
    let before = std::fs::read(&source).unwrap();
    assert!(library.adopt(&source, "Shared").is_err());
    assert_eq!(std::fs::read(&source).unwrap(), before);
    assert!(library.adopt(&template, "Template").is_err());
    let invalid = dir.path().join("invalid.pop");
    std::fs::write(&invalid, b"invalid").unwrap();
    assert!(library.adopt(&invalid, "Invalid").is_err());
    assert!(library.adopt(&source, " ").is_err());
    let oversize = dir.path().join("oversize.pop");
    std::fs::File::create(&oversize).unwrap().set_len(snapshot::MAX_FILE_BYTES + 1).unwrap();
    assert_eq!(library.adopt(&oversize, "Oversize").unwrap_err().issue, Issue::TooLarge);
    assert!(library.entries().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(dir.path().join("library/documents")).unwrap().count(), 0);
}

#[cfg(target_os = "macos")]
#[test]
fn adopt_cleanup_keeps_a_busy_or_changed_original() {
    let (dir, template) = fixture();
    let source = dir.path().join("local.pop");
    file::create_document(&template, &source).unwrap();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.adopt(&source, "Keep both if changed").unwrap();
    let saved = snapshot::inspect(std::path::Path::new(&entry.path)).unwrap();
    let owner = Owner::open(&source, Mode::Document, Arc::new(|_| {})).unwrap();
    assert_eq!(library.trash_adopted(&source, &entry.id).unwrap_err().issue, Issue::Busy);
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Later local work"}]));
    close(&owner);
    assert!(library.trash_adopted(&source, &entry.id).is_err());
    assert!(source.exists());
    assert_ne!(snapshot::inspect(&source).unwrap().fingerprint, saved.fingerprint);
    assert_eq!(snapshot::inspect(std::path::Path::new(&entry.path)).unwrap().fingerprint, saved.fingerprint);
}

#[test]
fn complete_snapshots_include_incoming_history_even_when_import_is_a_noop() {
    let (dir, template) = fixture();
    let left = Library::open(&dir.path().join("left"), "account").unwrap();
    let right = Library::open(&dir.path().join("right"), "account").unwrap();
    let entry = left.create(&template, "History").unwrap();
    let seed = left.stage(&entry.id).unwrap();
    deliver(&right, &seed);
    let path = PathBuf::from(&right.entries().unwrap()[0].path);
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    right.register(&owner).unwrap();
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Incoming edits"}]));
    let incoming = right.stage(&entry.id).unwrap();
    for _ in 0..2 {
        deliver(&left, &incoming);
        let saved = pop_storage::store::Store::open(std::path::Path::new(&entry.path), Mode::Snapshot).unwrap();
        let remote = pop_storage::store::Store::open(
            std::path::Path::new(incoming.asset_path.as_ref().unwrap()),
            Mode::Snapshot,
        )
        .unwrap();
        let local_doc = loro::LoroDoc::new();
        local_doc.import(&saved.document().unwrap().checkpoint().unwrap()).unwrap();
        let remote_doc = loro::LoroDoc::new();
        remote_doc.import(&remote.document().unwrap().checkpoint().unwrap()).unwrap();
        let local_vv = local_doc.oplog_vv();
        assert!(remote_doc.state_frontiers().iter().all(|id| local_vv.includes_id(id)));
    }
    close(&owner);
}

#[test]
fn history_guard_refuses_a_receipt_until_incoming_operations_are_present() {
    let (dir, template) = fixture();
    let left = Library::open(&dir.path().join("left"), "account").unwrap();
    let right = Library::open(&dir.path().join("right"), "account").unwrap();
    let entry = left.create(&template, "History guard").unwrap();
    let seed = left.stage(&entry.id).unwrap();
    deliver(&right, &seed);
    let path = PathBuf::from(&right.entries().unwrap()[0].path);
    let remote = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    right.register(&remote).unwrap();
    edit(&remote, json!([{"type":"set","path":["title"],"value":"Missing here"}]));
    let incoming = right.stage(&entry.id).unwrap();
    let source = std::path::Path::new(incoming.asset_path.as_ref().unwrap());
    let local = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    left.register(&local).unwrap();
    assert_eq!(
        snapshot::require_history(&local, source).unwrap_err(),
        "Incoming document history is incomplete; will retry"
    );
    deliver(&left, &incoming);
    snapshot::require_history(&local, source).unwrap();
    close(&local);
    close(&remote);
}

#[test]
fn retries_and_change_tokens_survive_reopening_without_confirming_work() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Retry").unwrap();
    library
        .retry(&pop_library::Retry {
            id: entry.id.clone(),
            title: entry.title.clone(),
            template: entry.template.clone(),
            message: "Offline".into(),
            retry_at: 123.0,
            incoming: true,
        })
        .unwrap();
    library.save_change_token(&[1, 2]).unwrap();
    drop(library);
    let library = Library::open(&root, "account").unwrap();
    assert_eq!(library.retries().unwrap()[0].retry_at, 123.0);
    assert_eq!(library.change_token().unwrap(), Some(vec![1, 2]));
    assert!(library.entries().unwrap()[0].dirty);
    library.clear_retry(&entry.id).unwrap();
    assert!(library.retries().unwrap().is_empty());
}

#[test]
fn missing_cloud_deletion_is_pending_across_reopen_and_never_resets_live_records() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Deleted").unwrap();
    library.acknowledge(&entry.id, &entry.version, &[1, 2], false, None).unwrap();
    assert!(!library.prepare_deletion_reupload(&entry.id).unwrap());
    let live = library.stage(&entry.id).unwrap();
    assert_eq!(live.fields, Some(vec![1, 2]));
    library.release_upload(std::path::Path::new(live.asset_path.as_deref().unwrap())).unwrap();
    library.delete(&entry.id).unwrap();
    library.acknowledge(&entry.id, "", &[1, 2], true, None).unwrap();
    library.block(&entry.id, "Deleted", "fixture", "old error").unwrap();
    assert!(library.prepare_deletion_reupload(&entry.id).unwrap());
    drop(library);
    let library = Library::open(&root, "account").unwrap();
    let upload = library.stage(&entry.id).unwrap();
    assert!(upload.entry.deleted && upload.entry.dirty);
    assert!(upload.fields.is_none() && upload.asset_path.is_none());
    assert!(upload.entry.problem.is_none());
    library.acknowledge(&entry.id, "", &[3], true, None).unwrap();
    assert!(!library.entries().unwrap()[0].dirty);
}

#[test]
fn whole_file_snapshot_merges_through_owner_preserving_identity_theme_and_blobs() {
    use pop_storage::store::Share;
    fn document_fixture() -> (tempfile::TempDir, PathBuf) {
        let (dir, template) = fixture();
        let path = dir.path().join("document.pop");
        file::create_document(&template, &path).unwrap();
        (dir, path)
    }
    let open = |path: &std::path::Path| Owner::open(path, Mode::Document, Arc::new(|_| {})).unwrap();
    let call = snapshot::request;
    let set = |title: &str| Request::Apply {
        batch: serde_json::from_value(json!({"intents":[{"type":"set","path":["title"],"value":title}]})).unwrap(),
        origin: Origin::Page,
    };
    let theme = |color: &str, origin| Request::Apply {
        batch: serde_json::from_value(json!({"intents":[{"type":"setTheme","values":{"accent":color}}]})).unwrap(),
        origin,
    };
    let state = |owner: &Owner| {
        let Reply::State { reading, .. } = call(owner, Request::State).unwrap() else { panic!("state") };
        serde_json::to_value(reading).unwrap()
    };
    let (dir, path) = document_fixture();
    let left = open(&path);
    call(
        &left,
        Request::Share { share: Share { room: "snapshot-room".into(), endpoint: "https://relay.example".into() } },
    )
    .unwrap();
    let seed = dir.path().join("seed.pop");
    call(&left, Request::Backup { destination: seed.clone() }).unwrap();
    let id = snapshot::inspect(&seed).unwrap().id;
    let replica = dir.path().join("replica.pop");
    snapshot::install(&seed, &replica, &id).unwrap();
    assert!(snapshot::install(&seed, &replica, &id).is_err(), "install never overwrites a replica");
    let right = open(&replica);
    call(&left, set("Offline title")).unwrap();
    call(&right, theme("#123456", Origin::Window)).unwrap();
    let Reply::Attachment { item } = call(&right, Request::PutAttachment { bytes: b"snapshot blob".to_vec() }).unwrap()
    else {
        panic!("blob")
    };
    let incoming = dir.path().join("incoming.pop");
    call(&right, Request::Backup { destination: incoming.clone() }).unwrap();
    snapshot::merge(&left, &incoming).unwrap();
    let union = snapshot::inspect(&path).unwrap();
    snapshot::merge(&left, &incoming).unwrap();
    assert_eq!(snapshot::inspect(&path).unwrap().version, union.version, "repeated delivery is idempotent");
    assert_eq!(state(&left)["value"]["title"], "Offline title");
    assert_eq!(state(&left)["theme"]["accent"], "#123456");
    assert!(
        matches!(call(&left, Request::ReadAttachment { id: item.id }).unwrap(), Reply::Bytes { bytes: Some(bytes) } if bytes == b"snapshot blob")
    );
    let merged = dir.path().join("merged.pop");
    call(&left, Request::Backup { destination: merged.clone() }).unwrap();
    snapshot::merge(&right, &merged).unwrap();
    assert_eq!(snapshot::inspect(&replica).unwrap().fingerprint, union.fingerprint);
    close(&left);
    close(&right);
    let left = open(&path);
    assert_eq!(snapshot::inspect(&path).unwrap().id, id);
    assert_eq!(snapshot::inspect(&path).unwrap().fingerprint, union.fingerprint);
    let (_other_dir, unrelated) = document_fixture();
    assert!(snapshot::merge(&left, &unrelated).unwrap_err().contains("different document identity"));
    close(&left);
}

fn frozen_frontier(path: &std::path::Path, token: &str, epoch: &str) -> String {
    let store = pop_storage::store::Store::open(path, Mode::Snapshot).unwrap();
    let document = loro::LoroDoc::new();
    document.import(&store.document().unwrap().checkpoint().unwrap()).unwrap();
    let mut heads: Vec<_> =
        document.oplog_vv().iter().map(|(peer, end)| (format!("{peer:016x}"), *end as u32)).collect();
    heads.sort_by(|a, b| a.0.cmp(&b.0));
    serde_json::to_string(&pop_sharing_protocol::lifecycle::Handoff {
        token: token.into(),
        seed_hash: epoch.into(),
        root: heads.clone(),
        heads,
    })
    .unwrap()
}

#[test]
fn catalog_authority_fences_legacy_uploads_and_persists_handoff_receipts() {
    use pop_library::{CatalogAuthority as A, CatalogReceipt};
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Authority").unwrap();
    let transition = "a".repeat(32);
    let epoch = "b".repeat(64);
    let room = "reserved-room".to_string();
    let preparing =
        A::Preparing { transition: transition.clone(), room: room.clone(), seed_hash: None, prior_checkpoint: None };
    library.prepare_authority(&entry.id, preparing.clone()).unwrap();
    assert!(library.stage_sync(&entry.id, true).unwrap().is_none());
    assert!(library.stage(&entry.id).is_err());
    let seed = library.stage_preparing(&entry.id, &transition).unwrap();
    library.release_upload(std::path::Path::new(seed.asset_path.as_ref().unwrap())).unwrap();
    library
        .accept_authority(
            &entry.id,
            "Authority",
            "fixture",
            preparing,
            b"prepare-tag",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    assert!(library.pending_authority(&entry.id).unwrap().is_none());
    let relay = A::Relay { transition: transition.clone(), room: room.clone(), seed_hash: epoch.clone() };
    library.prepare_authority(&entry.id, relay.clone()).unwrap();
    library
        .accept_authority(
            &entry.id,
            "Authority",
            "fixture",
            relay.clone(),
            b"relay-tag",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    assert!(!library.stage_allowed(&entry.id, true).unwrap());
    assert!(
        library.admit_authority(&entry.id, &A::default(), None).is_err(),
        "a queued old CloudKit writer cannot regain byte authority"
    );
    let freeze = frozen_frontier(std::path::Path::new(&entry.path), &"c".repeat(32), &epoch);
    let handoff = A::Handoff { transition: transition.clone(), room: room.clone(), freeze: freeze.clone() };
    library.prepare_authority(&entry.id, handoff.clone()).unwrap();
    library
        .accept_authority(
            &entry.id,
            "Authority",
            "fixture",
            handoff,
            b"freeze-tag",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    let final_file = library.stage_handoff(&entry.id, &transition).unwrap();
    let receipt = CatalogReceipt {
        transition: transition.clone(),
        room: room.clone(),
        freeze,
        snapshot_version: final_file.entry.version,
    };
    let snapshot = A::Snapshot { receipt: Some(receipt.clone()) };
    library.prepare_authority(&entry.id, snapshot.clone()).unwrap();
    library
        .accept_authority(
            &entry.id,
            "Authority",
            "fixture",
            snapshot.clone(),
            b"snapshot-cas-tag",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    library.release_upload(std::path::Path::new(final_file.asset_path.as_ref().unwrap())).unwrap();
    drop(library);
    let reopened = Library::open(&root, "account").unwrap();
    assert_eq!(reopened.authority(&entry.id).unwrap(), snapshot);
    assert!(reopened.pending_authority(&entry.id).unwrap().is_none());
    assert!(
        !reopened.stage_allowed(&entry.id, true).unwrap(),
        "the final receipt is immutable until Finalize acknowledges it"
    );
    assert_eq!(pop_library::CatalogAuthority::decode(2, Some(&snapshot.encode().unwrap())).unwrap(), snapshot);
    assert!(
        reopened.stage_handoff(&entry.id, &"e".repeat(32)).is_err(),
        "restaging cannot substitute a different frozen transition"
    );
    assert!(reopened.prepare_authority(&entry.id, A::default()).is_err());
    let proof = pop_library::CatalogCheckpointProof {
        transition: transition.clone(),
        source_version: receipt.snapshot_version.clone(),
        version: receipt.snapshot_version.clone(),
    };
    let retired = A::Checkpoint { proof: proof.clone() };
    reopened.prepare_authority(&entry.id, retired.clone()).unwrap();
    reopened
        .accept_authority(
            &entry.id,
            "Authority",
            "fixture",
            retired.clone(),
            b"retired-tag",
            Some(&next_catalog_stamp(&reopened, &entry.id)),
        )
        .unwrap();
    assert!(reopened.stage_allowed(&entry.id, true).unwrap());
    assert!(reopened.admit_authority(&entry.id, &A::default(), None).is_err());
    let next = A::Preparing {
        transition: "d".repeat(32),
        room: "next-room".into(),
        seed_hash: None,
        prior_checkpoint: Some(proof),
    };
    reopened.prepare_authority(&entry.id, next.clone()).unwrap();
    reopened
        .accept_authority(
            &entry.id,
            "Authority",
            "fixture",
            next,
            b"next-preparing",
            Some(&next_catalog_stamp(&reopened, &entry.id)),
        )
        .unwrap();
    assert!(reopened.prepare_authority(&entry.id, A::default()).is_err());
    reopened.prepare_authority(&entry.id, retired.clone()).unwrap();
    reopened
        .accept_authority(
            &entry.id,
            "Authority",
            "fixture",
            retired.clone(),
            b"cancel-tag",
            Some(&next_catalog_stamp(&reopened, &entry.id)),
        )
        .unwrap();
    assert!(reopened.pending_authority(&entry.id).unwrap().is_none());
    drop(reopened);
    assert_eq!(Library::open(&root, "account").unwrap().authority(&entry.id).unwrap(), retired);
}

#[test]
fn a_final_handoff_receipt_can_be_restaged_after_local_edits_without_changing_its_freeze() {
    use pop_library::{CatalogAuthority as A, CatalogReceipt};
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.create(&template, "Finalizing").unwrap();
    let freeze = frozen_frontier(std::path::Path::new(&entry.path), &"c".repeat(32), &"b".repeat(64));
    let receipt = CatalogReceipt {
        transition: "a".repeat(32),
        room: "room".into(),
        freeze,
        snapshot_version: entry.version.clone(),
    };
    library
        .accept_authority(
            &entry.id,
            "Finalizing",
            "fixture",
            A::Snapshot { receipt: Some(receipt.clone()) },
            b"receipt",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    let owner = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Edited while finalizing"}]));
    close(&owner);
    let upload = library.stage_handoff(&entry.id, &receipt.transition).unwrap();
    assert_ne!(upload.entry.version, receipt.snapshot_version);
    let mut refreshed = receipt.clone();
    refreshed.snapshot_version = upload.entry.version.clone();
    library.prepare_authority(&entry.id, A::Snapshot { receipt: Some(refreshed.clone()) }).unwrap();
    let mut wrong_room = refreshed.clone();
    wrong_room.room = "another-room".into();
    assert!(library.prepare_authority(&entry.id, A::Snapshot { receipt: Some(wrong_room) }).is_err());
    library
        .accept_authority(
            &entry.id,
            "Finalizing",
            "fixture",
            A::Snapshot { receipt: Some(refreshed.clone()) },
            b"new-receipt",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    assert_eq!(refreshed.freeze, receipt.freeze);
    let saved =
        pop_storage::store::Store::open(std::path::Path::new(upload.asset_path.as_ref().unwrap()), Mode::Snapshot)
            .unwrap();
    assert_eq!(saved.document().unwrap().reading().unwrap().value["title"], "Edited while finalizing");
    library.release_upload(std::path::Path::new(upload.asset_path.as_ref().unwrap())).unwrap();
}

#[test]
fn resuming_preparing_reconciles_its_retained_payload_before_seed_activation() {
    use pop_library::{CatalogAuthority as A, CatalogCheckpointProof};
    use pop_storage::store::{EpochLocation, Store};
    for cutoff in [false, true] {
        let (dir, template) = fixture();
        let library = Library::open(&dir.path().join("library"), "account").unwrap();
        let entry = library.create(&template, "Second device").unwrap();
        let initial = library.stage(&entry.id).unwrap();
        library.acknowledge(&entry.id, &initial.entry.version, b"initial", false, None).unwrap();
        let remote_path = std::path::Path::new(initial.asset_path.as_ref().unwrap());
        let remote = Owner::open(remote_path, Mode::Document, Arc::new(|_| {})).unwrap();
        edit(&remote, json!([{"type":"set","path":["title"],"value":"First device private edit"}]));
        close(&remote);
        let local = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
        edit(&local, json!([{"type":"setTheme","values":{"accent":"#123456"}}]));
        close(&local);
        let source = if cutoff {
            let source = dir.path().join("private-checkpoint.pop");
            let store = Store::open(remote_path, Mode::Document).unwrap();
            store
                .stage_epoch(
                    &EpochLocation::Path(source.clone()),
                    &store.document().unwrap().clean_checkpoint().unwrap(),
                )
                .unwrap();
            store.close().unwrap();
            source
        } else {
            remote_path.to_path_buf()
        };
        let info = snapshot::inspect(&source).unwrap();
        let authority = A::Preparing {
            transition: "a".repeat(32),
            room: "reserved-room".into(),
            seed_hash: None,
            prior_checkpoint: cutoff.then(|| CatalogCheckpointProof {
                transition: "d".repeat(32),
                source_version: initial.entry.version.clone(),
                version: info.version.clone(),
            }),
        };
        library
            .receive_catalog_snapshot(
                &entry.id,
                &entry.title,
                &entry.template,
                Some(&source),
                &info.version,
                b"preparing",
                false,
                None,
                None,
                authority.clone(),
                Some(&next_catalog_stamp(&library, &entry.id)),
            )
            .unwrap();
        assert_eq!(library.authority(&entry.id).unwrap(), authority);
        let saved = Store::open(std::path::Path::new(&entry.path), Mode::Snapshot).unwrap();
        let reading = saved.document().unwrap().reading().unwrap();
        let value = &reading.value;
        assert_eq!(value["title"], "First device private edit");
        if cutoff {
            let entries = library.entries().unwrap();
            let recovery = entries.iter().find(|copy| copy.id != entry.id).unwrap();
            let recovered = Store::open(std::path::Path::new(&recovery.path), Mode::Snapshot).unwrap();
            assert_eq!(recovered.document().unwrap().reading().unwrap().theme["accent"], "#123456");
        } else {
            assert_eq!(reading.theme["accent"], "#123456");
        }
        library.release_upload(remote_path).unwrap();
    }
}

#[test]
fn relay_catalog_discovery_needs_no_cloudkit_document_asset() {
    use pop_library::CatalogAuthority as A;
    support::isolate_registry();
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path(), "account").unwrap();
    let id = "00000000-0000-4000-8000-000000000001";
    let authority = A::Relay { transition: "a".repeat(32), room: "creator-room".into(), seed_hash: "b".repeat(64) };
    library
        .accept_authority(
            id,
            "Live on another device",
            "fixture",
            authority.clone(),
            b"catalog-tag",
            Some(&next_catalog_stamp(&library, id)),
        )
        .unwrap();
    let entry = &library.entries().unwrap()[0];
    assert!(!entry.downloaded);
    assert!(!entry.dirty);
    assert!(!std::path::Path::new(&entry.path).exists());
    assert_eq!(library.authority(id).unwrap(), authority);
    assert!(library.stage_sync(id, true).unwrap().is_none());
}

#[test]
fn a_device_can_receive_the_latest_checkpoint_after_missing_complete_sharing_cycles() {
    use pop_library::{CatalogAuthority as A, CatalogCheckpointProof, CatalogStamp};
    use pop_storage::store::{EpochLocation, Store};
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    for phase in 0..3 {
        let entry = library.create(&template, "Offline copy").unwrap();
        let original = library.stage(&entry.id).unwrap();
        let old = match phase {
            0 => A::Preparing {
                transition: "a".repeat(32),
                room: "old-room".into(),
                seed_hash: None,
                prior_checkpoint: None,
            },
            1 => A::Relay { transition: "a".repeat(32), room: "old-room".into(), seed_hash: "b".repeat(64) },
            _ => A::Handoff {
                transition: "a".repeat(32),
                room: "old-room".into(),
                freeze: frozen_frontier(std::path::Path::new(&entry.path), &"c".repeat(32), &"b".repeat(64)),
            },
        };
        let old_stamp = CatalogStamp { revision: 1, change_tag: "old-record".into() };
        library
            .accept_authority(&entry.id, "Offline copy", "fixture", old.clone(), b"old-record", Some(&old_stamp))
            .unwrap();
        let canonical = dir.path().join(format!("canonical-{phase}.pop"));
        let store = Store::open(std::path::Path::new(&entry.path), Mode::Document).unwrap();
        let remote = pop_core::Document::create(store.app().app.spec(), r#"{"title":"Latest canonical"}"#).unwrap();
        store.stage_epoch(&EpochLocation::Path(canonical.clone()), &remote.clean_checkpoint().unwrap()).unwrap();
        store.close().unwrap();
        let info = snapshot::inspect(&canonical).unwrap();
        let latest = A::Checkpoint {
            proof: CatalogCheckpointProof {
                transition: "d".repeat(32),
                source_version: "intervening-cycle-source".into(),
                version: info.version.clone(),
            },
        };
        let stamp = CatalogStamp { revision: 9, change_tag: "latest-record".into() };
        let invalid_stamp = CatalogStamp { revision: 10, change_tag: "invalid-payload".into() };
        assert!(
            library
                .receive_catalog_snapshot(
                    &entry.id,
                    "Invalid",
                    "fixture",
                    original.asset_path.as_deref().map(std::path::Path::new),
                    &info.version,
                    b"invalid",
                    false,
                    None,
                    None,
                    latest.clone(),
                    Some(&invalid_stamp)
                )
                .is_err()
        );
        assert_eq!(
            library.catalog_stamp(&entry.id).unwrap(),
            Some(old_stamp.clone()),
            "invalid bytes cannot advance the authority fence"
        );
        if phase == 0 {
            // A valid newer record can be admitted while canonical installation is
            // blocked by a live owner. Its fence must survive the failed attempt.
            let blocker = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
            assert!(
                library
                    .receive_catalog_snapshot(
                        &entry.id,
                        "Latest copy",
                        "fixture",
                        Some(&canonical),
                        &info.version,
                        b"latest-record",
                        false,
                        None,
                        None,
                        latest.clone(),
                        Some(&stamp)
                    )
                    .is_err()
            );
            assert_eq!(library.catalog_stamp(&entry.id).unwrap(), Some(stamp.clone()));
            assert!(
                library
                    .receive_catalog_snapshot(
                        &entry.id,
                        "Old copy",
                        "fixture",
                        original.asset_path.as_deref().map(std::path::Path::new),
                        &original.entry.version,
                        b"old-record",
                        false,
                        None,
                        None,
                        A::default(),
                        Some(&old_stamp)
                    )
                    .is_err()
            );
            assert_eq!(snapshot::inspect(std::path::Path::new(&entry.path)).unwrap().version, original.entry.version);
            close(&blocker);
        }
        library
            .receive_catalog_snapshot(
                &entry.id,
                "Latest copy",
                "fixture",
                Some(&canonical),
                &info.version,
                b"latest-record",
                false,
                None,
                None,
                latest.clone(),
                Some(&stamp),
            )
            .unwrap();
        assert_eq!(library.authority(&entry.id).unwrap(), latest);
        let before = std::fs::read(&entry.path).unwrap();
        let metadata = || {
            library
                .entries()
                .unwrap()
                .into_iter()
                .map(|entry| (entry.id, entry.title, entry.version, entry.dirty, entry.deleted))
                .collect::<Vec<_>>()
        };
        let entries = metadata();
        assert!(
            library.accept_authority(&entry.id, "Old copy", "fixture", old, b"old-record", Some(&old_stamp)).is_err()
        );
        for deleted in [false, true] {
            assert!(
                library
                    .receive_catalog_snapshot(
                        &entry.id,
                        "Old copy",
                        "fixture",
                        original.asset_path.as_deref().map(std::path::Path::new),
                        &original.entry.version,
                        b"old-record",
                        deleted,
                        None,
                        None,
                        A::default(),
                        Some(&old_stamp)
                    )
                    .is_err()
            );
        }
        assert!(
            library.acknowledge(&entry.id, &original.entry.version, b"old-record", true, Some(&old_stamp)).is_err()
        );
        assert!(
            library
                .receive_removed_metadata(
                    &entry.id,
                    "Old copy",
                    "fixture",
                    &original.entry.version,
                    b"old-record",
                    None,
                    None,
                    Some(&old_stamp)
                )
                .is_err()
        );
        let conflicting = CatalogStamp { revision: stamp.revision, change_tag: "different-record".into() };
        assert!(
            library
                .accept_authority(
                    &entry.id,
                    "Conflicting copy",
                    "fixture",
                    latest.clone(),
                    b"different",
                    Some(&conflicting)
                )
                .is_err()
        );
        assert_eq!(std::fs::read(&entry.path).unwrap(), before);
        assert_eq!(metadata(), entries);
        // The same immutable record can finish/retry its installation after restart.
        let reopened = Library::open(&root, "account").unwrap();
        reopened
            .receive_catalog_snapshot(
                &entry.id,
                "Latest copy",
                "fixture",
                Some(&canonical),
                &info.version,
                b"latest-record",
                false,
                None,
                None,
                latest,
                Some(&stamp),
            )
            .unwrap();
        assert_eq!(reopened.catalog_stamp(&entry.id).unwrap(), Some(stamp));
        let saved = Store::open(std::path::Path::new(&entry.path), Mode::Snapshot).unwrap();
        assert_eq!(saved.document().unwrap().reading().unwrap().value["title"], "Latest canonical");
        library.release_upload(std::path::Path::new(original.asset_path.as_ref().unwrap())).unwrap();
    }
}

#[test]
fn frozen_handoff_refuses_a_snapshot_missing_the_cutoff_without_removing_local_data() {
    use pop_library::CatalogAuthority as A;
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "account").unwrap();
    let entry = library.create(&template, "Frozen").unwrap();
    let epoch = "b".repeat(64);
    let transition = "a".repeat(32);
    let room = "room".to_string();
    library
        .accept_authority(
            &entry.id,
            "Frozen",
            "fixture",
            A::Relay { transition: transition.clone(), room: room.clone(), seed_hash: epoch.clone() },
            b"relay-tag",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    let mut freeze: pop_sharing_protocol::lifecycle::Handoff =
        serde_json::from_str(&frozen_frontier(std::path::Path::new(&entry.path), &"c".repeat(32), &epoch)).unwrap();
    freeze.heads[0].1 += 1;
    let frozen = A::Handoff { transition: transition.clone(), room, freeze: serde_json::to_string(&freeze).unwrap() };
    library.prepare_authority(&entry.id, frozen).unwrap();
    let before = std::fs::read(&entry.path).unwrap();
    assert!(library.stage_handoff(&entry.id, &transition).is_err());
    assert_eq!(std::fs::read(&entry.path).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path().join("library/uploads")).unwrap().count(), 0);
}

#[test]
fn catalog_format_one_keeps_its_snapshot_acceptance_and_unknown_formats_refuse() {
    use pop_library::{CatalogAuthority as A, Issue};
    assert_eq!(A::decode(1, None).unwrap(), A::default());
    assert_eq!(A::decode(3, None).unwrap_err().issue, Issue::RequiresUpdate);
    assert_eq!(A::decode(2, None).unwrap_err().issue, Issue::RequiresUpdate);
    let malformed =
        A::Preparing { transition: "wrong".into(), room: "room".into(), seed_hash: None, prior_checkpoint: None };
    assert!(malformed.encode().is_err());
    assert!(A::decode(2, Some(&"x".repeat(32 * 1024 + 1))).is_err());
}

#[test]
fn a_receipted_epoch_preserves_divergent_edits_as_a_findable_private_recovery() {
    use pop_library::{CatalogAuthority as A, CatalogReceipt};
    use pop_storage::store::{EpochLocation, Store};
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Shared notes").unwrap();
    let seed = library.stage(&entry.id).unwrap();
    library.acknowledge(&entry.id, &seed.entry.version, b"snapshot-tag", false, None).unwrap();
    library.release_upload(std::path::Path::new(seed.asset_path.as_ref().unwrap())).unwrap();
    let transition = "a".repeat(32);
    let epoch = "b".repeat(64);
    let room = "creator-room".to_string();
    library
        .accept_authority(
            &entry.id,
            "Shared notes",
            "fixture",
            A::Relay { transition: transition.clone(), room: room.clone(), seed_hash: epoch.clone() },
            b"relay-tag",
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    let owner = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
    let Reply::Attachment { item } =
        snapshot::request(&owner, Request::PutAttachment { bytes: b"private local photo".to_vec() }).unwrap()
    else {
        panic!("attachment");
    };
    let local_title = format!("Private local edit attachment/{}", item.id);
    edit(&owner, json!([{"type":"set","path":["title"],"value":local_title}]));
    close(&owner);
    let canonical = dir.path().join("canonical.pop");
    let store = Store::open(std::path::Path::new(&entry.path), Mode::Document).unwrap();
    let remote = pop_core::Document::create(store.app().app.spec(), r#"{"title":"Confirmed canonical"}"#).unwrap();
    store.stage_epoch(&EpochLocation::Path(canonical.clone()), &remote.clean_checkpoint().unwrap()).unwrap();
    store.close().unwrap();
    let info = snapshot::inspect(&canonical).unwrap();
    let freeze = frozen_frontier(&canonical, &"c".repeat(32), &epoch);
    let receipt = CatalogReceipt { transition, room, freeze, snapshot_version: info.version.clone() };
    let authority = A::Snapshot { receipt: Some(receipt) };
    library
        .receive_catalog_snapshot(
            &entry.id,
            "Shared notes",
            "fixture",
            Some(&canonical),
            &info.version,
            b"final-snapshot-tag",
            false,
            None,
            None,
            authority.clone(),
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    let entries = library.entries().unwrap();
    let recovery = entries.iter().find(|copy| copy.id != entry.id).unwrap();
    assert!(recovery.title.starts_with("Recovered edits"));
    assert!(!recovery.local_only && recovery.downloaded);
    let recovered = Store::open(std::path::Path::new(&recovery.path), Mode::Snapshot).unwrap();
    assert_eq!(recovered.document().unwrap().reading().unwrap().value["title"], local_title);
    assert_eq!(recovered.attachment(&item.id).unwrap(), b"private local photo");
    assert!(recovered.share().unwrap().is_none());
    let canonical_store = Store::open(std::path::Path::new(&entry.path), Mode::Snapshot).unwrap();
    assert_eq!(canonical_store.document().unwrap().reading().unwrap().value["title"], "Confirmed canonical");
    assert_eq!(canonical_store.share().unwrap().unwrap().endpoint, pop_storage::LIBRARY_SHARE_ENDPOINT);
    let recovery_id = recovery.id.clone();
    let private_upload = library.stage(&recovery_id).unwrap();
    library.release_upload(std::path::Path::new(private_upload.asset_path.as_ref().unwrap())).unwrap();
    library
        .receive_catalog_snapshot(
            &entry.id,
            "Shared notes",
            "fixture",
            Some(&canonical),
            &info.version,
            b"retry-snapshot-tag",
            false,
            None,
            None,
            authority,
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .unwrap();
    assert_eq!(library.entries().unwrap().len(), 2, "receipt retries do not create duplicate recoveries");
    drop(library);
    let reopened = Library::open(&root, "account").unwrap();
    assert_eq!(reopened.entries().unwrap().len(), 2);
    assert!(reopened.entries().unwrap().iter().any(|entry| entry.id == recovery_id && entry.downloaded));
}

fn grow_private_history(owner: &Owner) {
    let mut seed = 29u64;
    for _ in 0..180 {
        let text: String = (0..32 * 1024)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                char::from(b'a' + (seed % 26) as u8)
            })
            .collect();
        snapshot::request(
            owner,
            Request::Apply {
                batch: serde_json::from_value(json!({"intents":[{"type":"set","path":["title"],"value":text}]}))
                    .unwrap(),
                origin: Origin::Page,
            },
        )
        .unwrap();
    }
    snapshot::request(owner, Request::Flush).unwrap();
}

#[test]
fn private_checkpoint_conflict_restart_and_racing_edits_preserve_the_latest_state() {
    use pop_library::CatalogAuthority as A;
    use pop_storage::store::Store;
    let (dir, template) = fixture();
    let root = dir.path().join("private");
    let library = Library::open(&root, "account").unwrap();
    let remote = Library::open(&dir.path().join("remote"), "account").unwrap();
    let entry = library.create(&template, "Private history").unwrap();
    let seed = library.stage(&entry.id).unwrap();
    deliver(&remote, &seed);
    library.acknowledge(&entry.id, &seed.entry.version, b"base-tag", false, None).unwrap();
    library.release_upload(std::path::Path::new(seed.asset_path.as_ref().unwrap())).unwrap();
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    grow_private_history(&owner);
    let Reply::State { reading: before, .. } = snapshot::request(&owner, Request::State).unwrap() else {
        panic!("state");
    };
    let first = library.stage_sync(&entry.id, false).unwrap().unwrap();
    assert!(matches!(first.authority, A::Checkpoint { .. }));
    let Reply::State { reading: after, .. } = snapshot::request(&owner, Request::State).unwrap() else {
        panic!("state");
    };
    assert_eq!(after.version, before.version, "preparing never replaces the live source or its undo");
    assert!(std::fs::metadata(first.asset_path.as_ref().unwrap()).unwrap().len() < 128 * 1024);
    let remote_path = remote.entries().unwrap()[0].path.clone();
    let other = Owner::open(std::path::Path::new(&remote_path), Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&other, json!([{"type":"setTheme","values":{"accent":"#123456"}}]));
    close(&other);
    let conflict = remote.stage(&entry.id).unwrap();
    library
        .receive_with_artwork(
            &entry.id,
            &entry.title,
            &entry.template,
            conflict.asset_path.as_deref().map(std::path::Path::new),
            &conflict.entry.version,
            b"newer-server-tag",
            false,
            Some(&conflict.entry.artwork_version),
            Some(&conflict.entry.asset_revision),
            None,
        )
        .unwrap();
    assert!(!library.has_pending_checkpoint(&entry.id).unwrap());
    assert!(
        !std::path::Path::new(first.asset_path.as_ref().unwrap()).exists(),
        "the reconciled loser is retired only after merge/flush"
    );
    let Reply::State { reading: merged, .. } = snapshot::request(&owner, Request::State).unwrap() else {
        panic!("state");
    };
    assert_eq!(merged.value, before.value);
    assert_eq!(merged.theme["accent"], "#123456");
    let candidate = library.stage(&entry.id).unwrap();
    assert!(matches!(candidate.authority, A::Checkpoint { .. }));
    let durable_cloud = dir.path().join("cloud-cas.pop");
    std::fs::copy(candidate.asset_path.as_ref().unwrap(), &durable_cloud).unwrap();
    library.release_upload(std::path::Path::new(candidate.asset_path.as_ref().unwrap())).unwrap();
    assert!(
        std::path::Path::new(candidate.asset_path.as_ref().unwrap()).exists(),
        "an unacknowledged candidate survives release/restart"
    );
    close(&owner);
    drop(owner);
    drop(library);
    let library = Library::open(&root, "account").unwrap();
    assert!(library.has_pending_checkpoint(&entry.id).unwrap());
    assert!(std::path::Path::new(candidate.asset_path.as_ref().unwrap()).exists());
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    let Reply::Attachment { item } =
        snapshot::request(&owner, Request::PutAttachment { bytes: b"raced private photo".to_vec() }).unwrap()
    else {
        panic!("attachment");
    };
    let raced = format!("Raced after CAS attachment/{}", item.id);
    edit(&owner, json!([{"type":"set","path":["title"],"value":raced}]));
    let receive = || {
        library.receive_catalog_snapshot(
            &entry.id,
            &entry.title,
            &entry.template,
            Some(&durable_cloud),
            &candidate.entry.version,
            b"cas-receipt",
            false,
            Some(&candidate.entry.artwork_version),
            Some(&candidate.entry.asset_revision),
            candidate.authority.clone(),
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
    };
    receive().unwrap();
    assert!(!library.has_pending_checkpoint(&entry.id).unwrap());
    let Reply::State { reading: installed, .. } = snapshot::request(&owner, Request::State).unwrap() else {
        panic!("state");
    };
    assert_eq!(installed.value, merged.value);
    assert_eq!(installed.theme, merged.theme);
    assert_eq!(owner.app().document_uuid.as_deref(), Some(entry.id.as_str()));
    let entries = library.entries().unwrap();
    let recovery = entries.iter().find(|copy| copy.id != entry.id).unwrap();
    let recovered = Store::open(std::path::Path::new(&recovery.path), Mode::Snapshot).unwrap();
    assert_eq!(recovered.document().unwrap().reading().unwrap().value["title"], raced);
    assert_eq!(recovered.attachment(&item.id).unwrap(), b"raced private photo");
    receive().unwrap();
    assert_eq!(library.entries().unwrap().len(), 2, "lost acknowledgments never duplicate recovery");
    close(&owner);
}

#[test]
fn private_cloudkit_staging_never_upgrades_a_published_layout_one_document() {
    use pop_library::CatalogAuthority as A;
    use pop_storage::store::Store;
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("legacy"), "account").unwrap();
    let entry = library.create(&template, "Legacy").unwrap();
    let legacy = loro::LoroDoc::new();
    legacy.get_map("meta").insert("layout", 1).unwrap();
    legacy.get_map("data").insert("title", "Initial").unwrap();
    let snapshot = legacy.export(loro::ExportMode::Snapshot).unwrap();
    rusqlite::Connection::open(&entry.path).unwrap().execute("UPDATE history SET bytes=?1", [snapshot]).unwrap();
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    grow_private_history(&owner);
    let staged = library.stage(&entry.id).unwrap();
    assert!(matches!(staged.authority, A::Checkpoint { .. }));
    assert!(library.has_pending_checkpoint(&entry.id).unwrap());
    let saved = Store::open(std::path::Path::new(staged.asset_path.as_ref().unwrap()), Mode::Snapshot).unwrap();
    assert!(!saved.document().unwrap().supports_deep_compaction());
    let Reply::State { reading, .. } = snapshot::request(&owner, Request::State).unwrap() else {
        panic!("state");
    };
    let staged_doc = saved.document().unwrap();
    assert_eq!(staged_doc.version(), reading.version);
    assert_eq!(staged_doc.reading().unwrap().value, reading.value);
    assert_eq!(saved.app().document_uuid.as_deref(), Some(entry.id.as_str()));
    close(&owner);
}

#[test]
fn an_unpublished_checkpoint_reservation_recovers_after_restart_and_edit() {
    let (dir, template) = fixture();
    let root = dir.path().join("reservation");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Reservation").unwrap();
    let source = snapshot::inspect(std::path::Path::new(&entry.path)).unwrap().version;
    drop(library);
    // Persist exactly the crash boundary before PrepareEpoch has produced any bytes.
    let db = rusqlite::Connection::open(root.join("library.sqlite")).unwrap();
    let transition = "e".repeat(32);
    db.execute(
        "INSERT INTO catalog_candidates(document_id,transition,source_version,filename) VALUES(?1,?2,?3,?4)",
        rusqlite::params![entry.id, transition, source, format!("{}-{transition}.pop", entry.id)],
    )
    .unwrap();
    drop(db);
    let owner = Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Edited after interruption"}]));
    close(&owner);
    let library = Library::open(&root, "account").unwrap();
    let upload = library.stage(&entry.id).expect("an unpublishable reservation must refresh its source");
    let candidate =
        pop_storage::store::Store::open(std::path::Path::new(upload.asset_path.as_ref().unwrap()), Mode::Snapshot)
            .unwrap();
    assert_eq!(candidate.document().unwrap().reading().unwrap().value["title"], "Edited after interruption");
    assert!(matches!(upload.authority, pop_library::CatalogAuthority::Checkpoint { .. }));
}

#[test]
fn a_cutoff_retry_preserves_both_completed_recovery_and_later_local_edits() {
    use pop_library::{CatalogAuthority as A, CatalogCheckpointProof};
    use pop_storage::store::{EpochLocation, Store};
    let (dir, template) = fixture();
    let root = dir.path().join("retry-recovery");
    let library = Library::open(&root, "account").unwrap();
    let entry = library.create(&template, "Recovery retry").unwrap();
    let transition = "f".repeat(32);
    let path = std::path::Path::new(&entry.path);
    let owner = Owner::open(path, Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&owner, json!([{"type":"set","path":["title"],"value":"First local edit"}]));
    close(&owner);
    let store = Store::open(path, Mode::Document).unwrap();
    let first = store.document().unwrap();
    let recovery_name = format!("{}-{transition}.pop", entry.id);
    store
        .ensure_recovery(
            &EpochLocation::Path(root.join("recoveries").join(&recovery_name)),
            &first.clean_checkpoint().unwrap(),
        )
        .unwrap();
    let canonical = dir.path().join("canonical-retry.pop");
    let remote = pop_core::Document::create(store.app().app.spec(), r#"{"title":"Canonical"}"#).unwrap();
    store.stage_epoch(&EpochLocation::Path(canonical.clone()), &remote.clean_checkpoint().unwrap()).unwrap();
    store.close().unwrap();
    drop(library);
    // A completed recovery is durable, but canonical adoption did not happen.
    let db = rusqlite::Connection::open(root.join("library.sqlite")).unwrap();
    db.execute(
        "INSERT INTO catalog_recoveries(document_id,transition,filename) VALUES(?1,?2,?3)",
        rusqlite::params![entry.id, transition, recovery_name],
    )
    .unwrap();
    drop(db);
    let library = Library::open(&root, "account").unwrap();
    let owner = Owner::open(path, Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Later local edit"}]));
    close(&owner);
    let info = snapshot::inspect(&canonical).unwrap();
    let authority = A::Checkpoint {
        proof: CatalogCheckpointProof {
            transition,
            source_version: info.version.clone(),
            version: info.version.clone(),
        },
    };
    library
        .receive_catalog_snapshot(
            &entry.id,
            &entry.title,
            &entry.template,
            Some(&canonical),
            &info.version,
            b"cutoff-retry-tag",
            false,
            None,
            None,
            authority,
            Some(&next_catalog_stamp(&library, &entry.id)),
        )
        .expect("retry must reserve a distinct recovery for the changed source");
    let mut titles = library
        .entries()
        .unwrap()
        .iter()
        .filter(|copy| copy.id != entry.id)
        .map(|copy| {
            Store::open(std::path::Path::new(&copy.path), Mode::Snapshot)
                .unwrap()
                .document()
                .unwrap()
                .reading()
                .unwrap()
                .value["title"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    titles.sort();
    assert_eq!(titles, ["First local edit", "Later local edit"]);
    assert_eq!(
        Store::open(path, Mode::Snapshot).unwrap().document().unwrap().reading().unwrap().value["title"],
        "Canonical"
    );
}

fn next_catalog_stamp(library: &Library, id: &str) -> pop_library::CatalogStamp {
    let revision = library.catalog_stamp(id).unwrap().map_or(1, |stamp| stamp.revision + 1);
    pop_library::CatalogStamp { revision, change_tag: format!("record-{revision}") }
}

fn private_large_blobs(owner: &Owner) -> Vec<String> {
    (1..=3)
        .map(|byte| {
            let Reply::Attachment { item } =
                snapshot::request(owner, Request::PutAttachment { bytes: vec![byte; 6 * 1024 * 1024] }).unwrap()
            else {
                panic!("attachment");
            };
            item.id
        })
        .collect()
}

#[test]
fn private_checkpoint_discards_hidden_blob_pressure_before_cloudkit_upload() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("hidden-pressure"), "account").unwrap();
    let entry = library.create(&template, "Hidden blobs").unwrap();
    let seed = library.stage(&entry.id).unwrap();
    library.release_upload(std::path::Path::new(seed.asset_path.as_ref().unwrap())).unwrap();
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    let blobs = private_large_blobs(&owner);
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Only current state"}]));
    assert!(std::fs::metadata(&entry.path).unwrap().len() > snapshot::MAX_FILE_BYTES);
    let upload = library.stage(&entry.id).expect("hidden blobs must not permanently exceed CloudKit capacity");
    assert!(matches!(upload.authority, pop_library::CatalogAuthority::Checkpoint { .. }));
    assert!(std::fs::metadata(upload.asset_path.as_ref().unwrap()).unwrap().len() < snapshot::MAX_FILE_BYTES);
    let candidate =
        pop_storage::store::Store::open(std::path::Path::new(upload.asset_path.as_ref().unwrap()), Mode::Snapshot)
            .unwrap();
    assert_eq!(candidate.document().unwrap().reading().unwrap().value["title"], "Only current state");
    for blob in blobs {
        assert!(candidate.attachment(&blob).is_err());
    }
    close(&owner);
}

#[test]
fn an_unissued_oversized_checkpoint_can_refresh_after_visible_blobs_are_removed() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("oversized-candidate"), "account").unwrap();
    let entry = library.create(&template, "Oversized candidate").unwrap();
    let seed = library.stage(&entry.id).unwrap();
    library.release_upload(std::path::Path::new(seed.asset_path.as_ref().unwrap())).unwrap();
    let owner = Arc::new(Owner::open(std::path::Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    grow_private_history(&owner);
    let blobs = private_large_blobs(&owner);
    let references = blobs.iter().map(|id| format!("attachment/{id}")).collect::<Vec<_>>().join(" ");
    edit(&owner, json!([{"type":"set","path":["title"],"value":references}]));
    assert_eq!(library.stage(&entry.id).err().unwrap().issue, Issue::TooLarge);
    assert!(library.has_pending_checkpoint(&entry.id).unwrap());
    edit(&owner, json!([{"type":"set","path":["title"],"value":"Now small enough"}]));
    let upload =
        library.stage(&entry.id).expect("a candidate never issued for upload must not fence a smaller current state");
    let candidate =
        pop_storage::store::Store::open(std::path::Path::new(upload.asset_path.as_ref().unwrap()), Mode::Snapshot)
            .unwrap();
    assert_eq!(candidate.document().unwrap().reading().unwrap().value["title"], "Now small enough");
    assert!(std::fs::metadata(upload.asset_path.as_ref().unwrap()).unwrap().len() < snapshot::MAX_FILE_BYTES);
    close(&owner);
}
