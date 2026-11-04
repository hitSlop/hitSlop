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
