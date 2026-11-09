//! Local index/cache behavior, independent from authored apps and platform SDKs.
mod support;
use pop_library::{DocumentRole, Library};
use pop_runtime::owner::{Owner, Request};
use pop_sharing_client::LibraryUpdate;
use pop_sharing_client::local;
use pop_sharing_protocol::library::AccountEntry;
use pop_storage::store::Mode;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
fn fixture() -> (tempfile::TempDir, PathBuf) {
    support::fixture()
}

#[test]
fn firestore_index_migration_preserves_files_inbox_and_pending_enable() {
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account-a").unwrap();
    let created = library.create(&template, "Keep this file").unwrap();
    library.apply(LibraryUpdate::Upsert(catalog(&created.id, 42, "Cloud title"))).unwrap();
    drop(library);
    let db = rusqlite::Connection::open(root.join("index.sqlite")).unwrap();
    db.execute_batch(
        "ALTER TABLE documents RENAME COLUMN listed TO catalog_seen;
        CREATE TABLE catalog_writes(account TEXT,id TEXT,revision INTEGER,payload TEXT);
        CREATE TABLE catalog_clock(account TEXT,revision INTEGER);
        CREATE TABLE catalog_tombstones(account TEXT,id TEXT,payload TEXT);
        UPDATE documents SET pending_enable=1,opened=0 WHERE account='account-a';
        PRAGMA user_version=1;",
    )
    .unwrap();
    drop(db);
    let migrated = Library::open(&root, "account-a").unwrap();
    let entry = migrated.entries().unwrap().into_iter().find(|entry| entry.id == created.id).unwrap();
    assert_eq!(entry.revision, 0);
    assert!(entry.inbox);
    assert!(Path::new(&entry.path).is_file());
    let db = rusqlite::Connection::open(root.join("index.sqlite")).unwrap();
    assert_eq!(
        db.query_row("SELECT pending_enable FROM documents WHERE account='account-a'", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name IN ('catalog_writes','catalog_clock','catalog_tombstones')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

fn catalog(id: &str, revision: u64, title: &str) -> AccountEntry {
    AccountEntry {
        document: id.into(),
        title: title.into(),
        template_id: "fixture".into(),
        role: DocumentRole::Owner,
        revision,
        sharing: false,
    }
}

#[test]
fn remote_membership_can_leave_and_rejoin_live_without_reconnecting() {
    let (dir, _) = fixture();
    let library = Library::open(&dir.path().join("library"), "account-a").unwrap();
    let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    library.apply(LibraryUpdate::Upsert(catalog(id, 1, "Joined"))).unwrap();
    library.apply(LibraryUpdate::Remove(id.into())).unwrap();
    assert!(library.entries().unwrap().iter().find(|entry| entry.id == id).unwrap().deleted);
    library.apply(LibraryUpdate::Upsert(catalog(id, 2, "Rejoined elsewhere"))).unwrap();
    let entry = library.entries().unwrap().into_iter().find(|entry| entry.id == id).unwrap();
    assert!(!entry.deleted);
    assert_eq!(entry.title, "Rejoined elsewhere");
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
    account.apply(LibraryUpdate::Upsert(catalog(remote, 3, "Current"))).unwrap();
    account.apply(LibraryUpdate::Upsert(catalog(remote, 2, "Stale"))).unwrap();
    assert_eq!(account.entries().unwrap().iter().find(|entry| entry.id == remote).unwrap().title, "Current");
    assert!(!account.entries().unwrap().iter().find(|entry| entry.id == remote).unwrap().downloaded);
    let other = Library::open(&root, "account-b").unwrap();
    assert!(other.entries().unwrap().iter().all(|entry| entry.id != remote));
    account.detach().unwrap();
    assert!(Path::new(&local_entry.path).exists());
    assert_eq!(Library::open(&root, "").unwrap().entries().unwrap().len(), 1);
}

#[test]
fn malformed_snapshot_cannot_remove_existing_documents() {
    let (dir, _) = fixture();
    let account = Library::open(&dir.path().join("library"), "account-a").unwrap();
    let valid = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    account.apply(LibraryUpdate::Upsert(catalog(valid, 1, "Valid"))).unwrap();
    assert!(account.apply(LibraryUpdate::Snapshot(vec![catalog("invalid", 2, "Invalid")])).is_err());
    assert!(account.entries().unwrap().iter().any(|entry| entry.id == valid && !entry.deleted));
}
#[test]
fn complete_snapshots_remove_listed_documents_but_preserve_local_files() {
    let (dir, template) = fixture();
    let account = Library::open(&dir.path().join("library"), "account-a").unwrap();
    let local = account.create(&template, "Local").unwrap();
    let remote = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    account.apply(LibraryUpdate::Snapshot(vec![catalog(remote, 1, "Remote")])).unwrap();
    account.apply(LibraryUpdate::Snapshot(vec![])).unwrap();
    let entries = account.entries().unwrap();
    assert!(entries.iter().any(|e| e.id == remote && e.deleted));
    assert!(entries.iter().any(|e| e.id == local.id && !e.deleted));
    assert!(Path::new(&local.path).exists());
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
fn reading_entries_does_not_index_a_new_recovery_file() {
    use sha2::{Digest, Sha256};
    let (dir, template) = fixture();
    let root = dir.path().join("library");
    let library = Library::open(&root, "account-a").unwrap();
    let source = library.create(&template, "Source").unwrap();
    let scope: String = Sha256::digest(b"account-a").iter().map(|byte| format!("{byte:02x}")).collect();
    let recovered = root.join("accounts").join(scope).join("relay-state/Recovered/complete.pop");
    std::fs::create_dir_all(recovered.parent().unwrap()).unwrap();
    let owner = Owner::open(Path::new(&source.path), Mode::Document, Arc::new(|_| {})).unwrap();
    local::request(&owner, Request::Copy { destination: recovered, preview: None, icon: None }).unwrap();
    local::request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    let db = rusqlite::Connection::open(root.join("index.sqlite")).unwrap();
    let count = || db.query_row("SELECT count(*) FROM documents", [], |row| row.get::<_, i64>(0)).unwrap();
    let before = count();
    let _ = library.entries().unwrap();
    assert_eq!(count(), before, "reading a snapshot must not index an unrelated recovery file");
    library.reconcile().unwrap();
    assert_eq!(count(), before + 1, "explicit reconciliation indexes the completed recovery");
    assert_eq!(library.entries().unwrap().len(), 2);
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
    account.apply(LibraryUpdate::Upsert(catalog(&created.id, 1, "Created here"))).unwrap();
    assert!(!account.entries().unwrap().iter().find(|entry| entry.id == created.id).unwrap().inbox);
    let incoming_source = dir.path().join("incoming.pop");
    pop_storage::file::create_document(&template, &incoming_source).unwrap();
    let id = local::inspect(&incoming_source).unwrap().id;
    account.apply(LibraryUpdate::Upsert(catalog(&id, 1, "Incoming"))).unwrap();
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
    account.apply(LibraryUpdate::Upsert(catalog(&id, 2, "Incoming renamed"))).unwrap();
    drop(account);
    let reopened = Library::open(&root, "account-a").unwrap();
    let incoming = reopened.entries().unwrap().into_iter().find(|entry| entry.id == id).unwrap();
    assert!(!incoming.inbox && !incoming.downloaded);
    let other = Library::open(&root, "account-b").unwrap();
    other.apply(LibraryUpdate::Upsert(catalog(&id, 1, "Incoming to another account"))).unwrap();
    assert!(other.entries().unwrap().iter().find(|entry| entry.id == id).unwrap().inbox);
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
    library.apply(LibraryUpdate::Upsert(catalog(&remote_id, 1, "Downloaded"))).unwrap();
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
    other.apply(LibraryUpdate::Upsert(catalog(&missing, 1, "Not downloaded yet"))).unwrap();
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
