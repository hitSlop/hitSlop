//! Local index/cache behavior, independent from authored apps and platform SDKs.
mod support;
use pop_library::{DocumentEntry, DocumentRole, Library};
use pop_runtime::owner::{Owner, Request};
use pop_sharing_client::local;
use pop_storage::store::Mode;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
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

fn catalog(id: &str, revision: u64, title: &str) -> DocumentEntry {
    DocumentEntry {
        document_id: id.into(),
        title: title.into(),
        template_id: "fixture".into(),
        app_revision: None,
        preview_ref: None,
        modified_at: revision,
        role: DocumentRole::Owner,
        access_generation: 1,
        catalog_revision: revision,
        deleted: false,
    }
}
#[test]
fn local_creation_and_indexing_preserve_identity_and_existing_file_location() {
    let (dir, template) = fixture();
    let local = Library::open(&dir.path().join("library"), "").unwrap();
    let created = local.create(&template, "Local").unwrap();
    let owner = Arc::new(Owner::open(Path::new(&created.path), Mode::Document, Arc::new(|_| {})).unwrap());
    local.register(&owner).unwrap();
    let identity = owner.app().document_uuid.clone().unwrap();
    let indexed = local.adopt(Path::new(&created.path), "Indexed").unwrap();
    assert_eq!(indexed.id, identity);
    assert_eq!(indexed.path, created.path);
    assert!(indexed.local_only);
    assert!(local.connection(&identity).is_none());
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    assert_eq!(pop_sharing_client::local_document(Path::new(&created.path)).unwrap(), Some(identity));
}
#[test]
fn account_projection_uses_current_sdk_metadata_and_hides_on_signout_without_deleting_files() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let local = Library::open(&root, "").unwrap();
    let local_entry = local.create(&template, "Local").unwrap();
    let account = Library::open(&root, "account-a").unwrap();
    let remote = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    account.receive_catalog(&[catalog(remote, 3, "Current")]).unwrap();
    account.receive_catalog(&[catalog(remote, 2, "Stale")]).unwrap();
    assert_eq!(account.entries().unwrap().iter().find(|entry| entry.id == remote).unwrap().title, "Stale");
    assert!(!account.entries().unwrap().iter().find(|entry| entry.id == remote).unwrap().downloaded);
    let other = Library::open(&root, "account-b").unwrap();
    assert!(other.entries().unwrap().iter().all(|entry| entry.id != remote));
    account.detach().unwrap();
    assert!(Path::new(&local_entry.path).exists());
    assert_eq!(Library::open(&root, "").unwrap().entries().unwrap().len(), 1);
}
#[test]
fn registering_an_owner_from_another_account_cannot_adopt_or_connect_it() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let source = dir.path().join("external.pop");
    pop_storage::file::create_document(&template, &source).unwrap();
    let owner = Arc::new(Owner::open(&source, Mode::Document, Arc::new(|_| {})).unwrap());
    let other = Library::open(&root, "account-b").unwrap();
    other.register(&owner).unwrap();
    assert!(other.entries().unwrap().is_empty());
    assert!(other.connection(owner.app().document_uuid.as_deref().unwrap()).is_none());
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
}

fn edit(owner: &Owner, title: &str) {
    local::request(
        owner,
        Request::Apply {
            batch: serde_json::from_value(
                serde_json::json!({"intents":[{"type":"set","path":["title"],"value":title}]}),
            )
            .unwrap(),
            origin: pop_core::Origin::Page,
        },
    )
    .unwrap();
}
#[test]
fn tombstone_preserves_late_accepted_edits_as_account_local_recoveries() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let account = Library::open(&root, "account-a").unwrap();
    let entry = account.create(&template, "Original").unwrap();
    account.receive_catalog(&[catalog(&entry.id, 3, "Cloud")]).unwrap();
    let owner = Arc::new(Owner::open(Path::new(&entry.path), Mode::Document, Arc::new(|_| {})).unwrap());
    account.register(&owner).unwrap();
    edit(&owner, "Before revocation");
    let mut deleted = catalog(&entry.id, 4, "");
    deleted.deleted = true;
    account.receive_catalog(&[deleted]).unwrap();
    edit(&owner, "Accepted while closing");
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    account.unregister(&entry.id).unwrap();
    let entries = account.entries().unwrap();
    let recovered: Vec<_> = entries.iter().filter(|entry| entry.account_bound && entry.local_only).collect();
    assert_eq!(recovered.len(), 2);
    assert!(recovered.iter().any(|entry| {
        let store = pop_storage::store::Store::open(Path::new(&entry.path), Mode::Snapshot).unwrap();
        store.document().unwrap().reading().unwrap().value["title"] == "Accepted while closing"
    }));
    assert!(Library::open(&root, "").unwrap().entries().unwrap().iter().all(|entry| !entry.account_bound));
    assert_eq!(
        Library::open(&root, "account-a")
            .unwrap()
            .entries()
            .unwrap()
            .iter()
            .filter(|entry| entry.account_bound && entry.local_only)
            .count(),
        2
    );
}
#[test]
fn legacy_cache_collision_is_preserved_once_without_replacing_the_canonical_file() {
    let (dir, template) = fixture();
    let library = Library::open(&dir.path().join("library"), "").unwrap();
    let canonical = library.create(&template, "Canonical").unwrap();
    let duplicate = dir.path().join("old.pop");
    std::fs::copy(&canonical.path, &duplicate).unwrap();
    let owner = Owner::open(&duplicate, Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&owner, "Old cache edits");
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    let recovered = library.adopt_local_cache(&duplicate, "Old").unwrap().unwrap();
    assert_ne!(recovered.id, canonical.id);
    assert_eq!(library.adopt_local_cache(&duplicate, "Old").unwrap().unwrap().id, recovered.id);
    assert_eq!(library.entries().unwrap().len(), 2);
    assert_eq!(local::inspect(&duplicate).unwrap().id, canonical.id);
    assert_eq!(local::inspect(Path::new(&canonical.path)).unwrap().id, canonical.id);
}

#[test]
fn recovery_completed_before_indexing_is_discovered_after_restart_and_hidden_from_other_accounts() {
    use sha2::{Digest, Sha256};
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let account = Library::open(&root, "account-a").unwrap();
    let source = account.create(&template, "Source").unwrap();
    let scope: String = Sha256::digest(b"account-a").iter().map(|byte| format!("{byte:02x}")).collect();
    let recovered = root.join("accounts").join(scope).join("relay-state/Recovered/complete.pop");
    std::fs::create_dir_all(recovered.parent().unwrap()).unwrap();
    let owner = Owner::open(Path::new(&source.path), Mode::Document, Arc::new(|_| {})).unwrap();
    edit(&owner, "Recovered before crash");
    local::request(&owner, Request::Copy { destination: recovered.clone(), preview: None, icon: None }).unwrap();
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    drop(account);
    let entries = Library::open(&root, "account-a").unwrap().entries().unwrap();
    let recovered = std::fs::canonicalize(recovered).unwrap();
    let copy = entries.iter().find(|entry| Path::new(&entry.path) == recovered).unwrap();
    assert!(copy.account_bound && copy.local_only);
    assert_ne!(copy.id, source.id);
    assert!(Library::open(&root, "account-b").unwrap().entries().unwrap().iter().all(|entry| entry.id != copy.id));
}

#[test]
fn failed_initial_enable_keeps_selected_document_and_path_pending_after_restart() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let account = Library::open(&root, "account-a").unwrap();
    let entry = account.create(&template, "Selected").unwrap();
    assert!(account.enable_sync(&entry.id, "").is_err());
    drop(account);
    let reopened = Library::open(&root, "account-a").unwrap();
    let entries = reopened.entries().unwrap();
    let pending = entries.iter().find(|item| item.id == entry.id).unwrap();
    assert_eq!(pending.path, entry.path);
    assert!(pending.account_bound && !pending.local_only && pending.dirty);
    assert!(pending.problem.is_some());
    assert_eq!(reopened.account_paths().unwrap(), vec![entry.path.clone()]);
    assert_eq!(local::inspect(Path::new(&entry.path)).unwrap().id, entry.id);
    assert!(Library::open(&root, "").unwrap().entries().unwrap().iter().all(|item| item.id != entry.id));
}

#[test]
fn inbox_tracks_first_real_open_and_survives_reopen_and_download_removal() {
    let (dir, template) = fixture();
    let root = dir.path().join("inbox");
    let account = Library::open(&root, "account-a").unwrap();
    let created = account.create(&template, "Created here").unwrap();
    assert!(!created.inbox);
    account.receive_catalog(&[catalog(&created.id, 1, "Created here")]).unwrap();
    assert!(!account.entries().unwrap().iter().find(|entry| entry.id == created.id).unwrap().inbox);
    let incoming_source = dir.path().join("incoming.pop");
    pop_storage::file::create_document(&template, &incoming_source).unwrap();
    let id = local::inspect(&incoming_source).unwrap().id;
    account.receive_catalog(&[catalog(&id, 1, "Incoming")]).unwrap();
    let incoming = account.entries().unwrap().into_iter().find(|entry| entry.id == id).unwrap();
    assert!(incoming.inbox && !incoming.downloaded);
    // A completed download is not itself a window open.
    std::fs::copy(&incoming_source, &incoming.path).unwrap();
    assert!(account.entries().unwrap().iter().find(|entry| entry.id == id).unwrap().inbox);
    let owner = Arc::new(Owner::open(Path::new(&incoming.path), Mode::Document, Arc::new(|_| {})).unwrap());
    account.register(&owner).unwrap();
    assert!(!account.entries().unwrap().iter().find(|entry| entry.id == id).unwrap().inbox);
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    account.unregister(&id).unwrap();
    assert!(
        account.remove_download(&id).is_err(),
        "unconfirmed cache removal still refuses without resetting seen state"
    );
    assert!(!account.entries().unwrap().iter().find(|entry| entry.id == id).unwrap().inbox);
    // Missing-cache eviction is also idempotent, and must not make it new again.
    pop_storage::file::delete_document(Path::new(&incoming.path)).unwrap();
    account.remove_download(&id).unwrap();
    account.receive_catalog(&[catalog(&id, 2, "Incoming renamed")]).unwrap();
    drop(account);
    let reopened = Library::open(&root, "account-a").unwrap();
    let incoming = reopened.entries().unwrap().into_iter().find(|entry| entry.id == id).unwrap();
    assert!(!incoming.inbox && !incoming.downloaded);
    let other = Library::open(&root, "account-b").unwrap();
    other.receive_catalog(&[catalog(&id, 1, "Incoming to another account")]).unwrap();
    assert!(other.entries().unwrap().iter().find(|entry| entry.id == id).unwrap().inbox);
}

#[test]
fn cache_migration_marks_only_reliably_unseen_remote_references_as_inbox() {
    let (dir, template) = fixture();
    let root = dir.path().join("migration");
    let account = Library::open(&root, "account-a").unwrap();
    let local = account.create(&template, "Local").unwrap();
    let unseen = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let removed = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    account.receive_catalog(&[catalog(unseen, 1, "Unseen"), catalog(removed, 1, "Previously downloaded")]).unwrap();
    let downloaded_source = dir.path().join("downloaded.pop");
    pop_storage::file::create_document(&template, &downloaded_source).unwrap();
    let downloaded = local::inspect(&downloaded_source).unwrap().id;
    account.receive_catalog(&[catalog(&downloaded, 1, "Downloaded")]).unwrap();
    let downloaded_path = account.entries().unwrap().into_iter().find(|entry| entry.id == downloaded).unwrap().path;
    std::fs::copy(downloaded_source, downloaded_path).unwrap();
    drop(account);
    let db = rusqlite::Connection::open(root.join("cloud-index.sqlite")).unwrap();
    db.execute(
        "UPDATE documents SET version='prior-saved-version',file_stamp='prior-file-stamp' WHERE id=?1",
        [removed],
    )
    .unwrap();
    // Recreate the pre-feature device cache; document files and cloud formats stay unchanged.
    db.execute_batch("ALTER TABLE documents DROP COLUMN opened;ALTER TABLE documents DROP COLUMN sharing_enabled;")
        .unwrap();
    drop(db);
    let migrated = Library::open(&root, "account-a").unwrap();
    let entries = migrated.entries().unwrap();
    assert!(entries.iter().find(|entry| entry.id == unseen).unwrap().inbox);
    for id in [local.id.as_str(), removed, downloaded.as_str()] {
        assert!(!entries.iter().find(|entry| entry.id == id).unwrap().inbox);
    }
    assert!(entries.iter().all(|entry| entry.sharing_enabled.is_none()), "migration never infers owner sharing state");
}

#[test]
fn library_root_relocation_preserves_managed_files_accounts_and_external_paths() {
    let (dir, template) = fixture();
    let old = dir.path().join("before");
    let moved = dir.path().join("after");
    let library = Library::open(&old, "account-a").unwrap();
    let local_entry = library.create(&template, "Managed local").unwrap();
    let external = dir.path().join("external.pop");
    pop_storage::file::create_document(&template, &external).unwrap();
    let external = std::fs::canonicalize(external).unwrap();
    let external_entry = library.adopt(&external, "External Mac document").unwrap();
    let incoming = dir.path().join("incoming-relocation.pop");
    pop_storage::file::create_document(&template, &incoming).unwrap();
    let remote_id = local::inspect(&incoming).unwrap().id;
    library.receive_catalog(&[catalog(&remote_id, 1, "Downloaded")]).unwrap();
    let remote_path = library.entries().unwrap().into_iter().find(|entry| entry.id == remote_id).unwrap().path;
    local::install(&incoming, Path::new(&remote_path), &remote_id).unwrap();
    let owner = Arc::new(Owner::open(Path::new(&remote_path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    library.unregister(&remote_id).unwrap();
    let other = Library::open(&old, "account-b").unwrap();
    let missing_source = dir.path().join("future-download.pop");
    pop_storage::file::create_document(&template, &missing_source).unwrap();
    let missing = local::inspect(&missing_source).unwrap().id;
    other.receive_catalog(&[catalog(&missing, 1, "Not downloaded yet")]).unwrap();
    drop(other);
    drop(library);
    drop(owner);
    std::fs::rename(&old, &moved).unwrap();
    let moved = std::fs::canonicalize(moved).unwrap();
    let library = Library::open(&moved, "account-a").unwrap();
    let entries = library.entries().unwrap();
    assert!(
        entries.iter().find(|entry| entry.id == local_entry.id).unwrap().downloaded,
        "managed local paths follow the moved library root"
    );
    let remote = entries.iter().find(|entry| entry.id == remote_id).unwrap();
    assert!(remote.downloaded && !remote.inbox);
    assert!(Path::new(&remote.path).starts_with(&moved));
    assert_eq!(entries.iter().find(|entry| entry.id == external_entry.id).unwrap().path, external.to_string_lossy());
    let owner = Arc::new(Owner::open(Path::new(&remote.path), Mode::Document, Arc::new(|_| {})).unwrap());
    library.register(&owner).unwrap();
    assert_eq!(library.adopt(Path::new(&remote.path), "Reopened").unwrap().id, remote_id);
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    library.unregister(&remote_id).unwrap();
    let other = Library::open(&moved, "account-b").unwrap();
    let pending = other.entries().unwrap().into_iter().find(|entry| entry.id == missing).unwrap();
    assert!(Path::new(&pending.path).starts_with(&moved));
    assert!(!pending.downloaded && pending.inbox);
    local::install(&missing_source, Path::new(&pending.path), &missing).unwrap();
    let next_owner = Arc::new(Owner::open(Path::new(&pending.path), Mode::Document, Arc::new(|_| {})).unwrap());
    other.register(&next_owner).unwrap();
    local::request(&next_owner, Request::Close { preview: None, icon: None }).unwrap();
    other.unregister(&missing).unwrap();
    assert!(other.entries().unwrap().iter().find(|entry| entry.id == missing).unwrap().downloaded);
    assert!(library.account_paths().unwrap().iter().all(|path| Path::new(path).starts_with(&moved)));
    assert!(other.entries().unwrap().iter().all(|entry| entry.id != remote_id));
}

#[test]
fn ios_container_relocation_reuses_legacy_sources_without_new_recovered_copies() {
    let (dir, template) = fixture();
    let parent = dir.path().join("Containers/Data/Application");
    let old = parent.join("0E20CAF2-615D-4D0D-8D28-000B84114F4C");
    let current = parent.join("A1AE72BC-BD1B-47B7-9D74-803FCEB299FF");
    let tail = "Library/Application Support/pop/CloudDocuments";
    let old_root = old.join(tail);
    let library = Library::open(&old_root, "account-a").unwrap();
    let source = old.join("Documents/Previously adopted.pop");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    pop_storage::file::create_document(&template, &source).unwrap();
    let original = library.adopt_local_cache(&source, "Original").unwrap().unwrap();
    let owner = Arc::new(Owner::open(&source, Mode::Document, Arc::new(|_| {})).unwrap());
    let extra = old_root.join("documents/already-recovered.pop");
    local::request(&owner, Request::Copy { destination: extra.clone(), preview: None, icon: None }).unwrap();
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    let recovered = library.adopt(&extra, "Recovered edits already present").unwrap();
    drop(library);
    drop(owner);
    std::fs::rename(&old, &current).unwrap();
    let root = std::fs::canonicalize(current.join(tail)).unwrap();
    let source = std::fs::canonicalize(current.join("Documents/Previously adopted.pop")).unwrap();
    // An earlier affected update may already have produced a second current-prefix
    // mapping to an independent recovery. Both actual documents must be retained.
    let db = rusqlite::Connection::open(root.join("cloud-index.sqlite")).unwrap();
    db.execute(
        "INSERT INTO legacy_sources(path,id,fingerprint) VALUES(?1,?2,'affected-update')",
        rusqlite::params![source.to_string_lossy(), recovered.id],
    )
    .unwrap();
    drop(db);
    let bytes = std::fs::read(&source).unwrap();
    let library = Library::open(&root, "account-a").unwrap();
    let entries = library.entries().unwrap();
    let indexed = entries.iter().find(|entry| entry.id == original.id).unwrap();
    assert_eq!(Path::new(&indexed.path), source, "old iOS absolute app-container paths rebase before file discovery");
    assert!(indexed.downloaded);
    assert!(
        entries.iter().any(|entry| entry.id == recovered.id && entry.downloaded),
        "existing extra recovery is never deleted"
    );
    for _ in 0..2 {
        assert_eq!(library.adopt_local_cache(&source, "Original").unwrap().unwrap().id, original.id);
    }
    assert_eq!(library.entries().unwrap().len(), 2, "relocation must not manufacture further recovery copies");
    assert_eq!(std::fs::read(source).unwrap(), bytes, "path repair never rewrites document bytes");
}
