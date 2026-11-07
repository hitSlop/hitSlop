use super::*;
pub(super) fn fixture() -> (tempfile::TempDir, Arc<Owner>) {
    static REGISTRY: std::sync::Once = std::sync::Once::new();
    REGISTRY.call_once(|| {
        pop_storage::registry::use_folder(&std::env::temp_dir().join("pop-sharing-test-registry")).unwrap()
    });
    let dir = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/checklist/document");
    let template = dir.path().join("template.pop");
    pop_storage::file::pack(&std::fs::read_to_string(source.join("input.json")).unwrap(), &source, &template, None)
        .unwrap();
    let path = dir.path().join("document.pop");
    pop_storage::file::create_document(&template, &path).unwrap();
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    (dir, owner)
}
pub(super) fn test_journal(owner: &Owner, key: &RoomKey, confirmed: bool) -> Arc<journal::Journal> {
    let document = owner.app().document_uuid.as_deref().unwrap();
    let journal = journal::Journal::new(&owner.path().parent().unwrap().join("relay-state"), key, document).unwrap();
    if confirmed {
        let store = Store::open(owner.path(), Mode::Snapshot).unwrap();
        let receipt = journal.receipt(store.document().unwrap().version());
        journal.transaction().unwrap().confirm(&receipt).unwrap();
        store.close().unwrap();
    }
    Arc::new(journal)
}

#[test]
fn invitations_are_validated_without_treating_the_capability_as_an_identity() {
    let link =
        Invitation { room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(), capability: "b".repeat(64) }.link().unwrap();
    assert_eq!(invitation_document(&link).unwrap(), "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
    assert!(invitation_document("https://attacker.example/join#secret").is_err());
    assert!(invitation_document("identity-token").is_err());
}

#[test]
fn journal_contention_is_retryable_instead_of_a_terminal_refusal() {
    let (_directory, owner) = fixture();
    let document = owner.app().document_uuid.as_deref().unwrap();
    let key = RoomKey::identity(document, "test-token").unwrap();
    let journal = test_journal(&owner, &key, true);
    let _held = journal.transaction().unwrap();
    let error = journal.transaction().err().expect("another transaction holds the journal");
    assert!(matches!(journal::local(error), transport::HttpFailure::Busy(_)));
}

#[test]
fn failed_receipt_confirmation_keeps_the_previous_receipt() {
    let (_directory, owner) = fixture();
    let document = owner.app().document_uuid.as_deref().unwrap();
    let key = RoomKey::identity(document, "test-token").unwrap();
    request(
        &owner,
        Request::Share { share: pop_storage::store::Share { room: document.into(), endpoint: key.origin.clone() } },
    )
    .unwrap();
    let journal = test_journal(&owner, &key, true);
    let previous = journal.transaction().unwrap().current().unwrap().unwrap().owner_version;
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
                .unwrap(),
            origin: pop_core::Origin::Page,
        },
    )
    .unwrap();
    request(&owner, Request::Flush).unwrap();
    let saved = Store::open(owner.path(), Mode::Snapshot).unwrap();
    let heads = saved.document().unwrap().wire_history();
    saved.close().unwrap();
    let version = heads.iter().map(|(peer, end)| (u64::from_str_radix(peer, 16).unwrap(), *end)).collect();
    let connection = owner.sync_connection(Arc::new(|_| {}));
    connection
        .frame(pop_core::sync::Frame::Welcome { permission: pop_core::sync::Permission::Write, version }.encode());
    connection.frame(pop_core::sync::Frame::CaughtUp.encode());
    // State is a serial-owner barrier after the handshake messages.
    request(&owner, Request::State).unwrap();
    let receipt_path = std::fs::read_dir(owner.path().parent().unwrap().join("relay-state"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .unwrap();
    // A directory at the temporary receipt path deterministically refuses the write.
    std::fs::create_dir(receipt_path.with_extension("tmp")).unwrap();
    assert!(journal::confirm(&owner, &connection, &journal).is_err());
    assert_eq!(journal.transaction().unwrap().current().unwrap().unwrap().owner_version, previous);
    connection.detach();
}

#[test]
fn connection_snapshot_observer_does_not_replace_the_ui_listener_or_hold_callback_locks() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let primary: Notice = Arc::new(Mutex::new(None));
    let observer: Notice = Arc::new(Mutex::new(None));
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let slot = Arc::downgrade(&observer);
    *observer.lock().unwrap() = Some(Arc::new(move || {
        if slot.upgrade().unwrap().try_lock().is_ok() {
            observed.fetch_add(1, Ordering::Relaxed);
        }
    }));
    let output = connection_notices(&primary, &observer);
    let observed = calls.clone();
    let slot = Arc::downgrade(&primary);
    *primary.lock().unwrap() = Some(Arc::new(move || {
        if slot.upgrade().unwrap().try_lock().is_ok() {
            observed.fetch_add(1, Ordering::Relaxed);
        }
    }));
    let callback = output.lock().unwrap().clone().unwrap();
    callback();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn inspecting_a_missing_receipt_does_not_create_relay_state() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("not-connected");
    let document = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    assert!(confirmed_saved_version(&missing, document).unwrap().is_none());
    assert!(!missing.exists(), "reading a missing receipt must not create a relay directory or lock");
}

#[test]
fn receipt_inspections_share_reads_but_fence_writers() {
    let (directory, owner) = fixture();
    let document = owner.app().document_uuid.as_deref().unwrap();
    let key = RoomKey::identity(document, "test-token").unwrap();
    let journal = test_journal(&owner, &key, true);
    let first = journal.inspection().unwrap().unwrap();
    let second = journal.inspection().unwrap().unwrap();
    let current = first.current().unwrap().unwrap();
    assert_eq!(second.current().unwrap().unwrap().owner_version, current.owner_version);
    assert!(matches!(journal.transaction(), Err(journal::Error::Busy)));
    drop(second);
    drop(first);
    let writer = journal.transaction().unwrap();
    assert!(matches!(journal.inspection(), Err(journal::Error::Busy)));
    writer.confirm(&journal.receipt("next-frontier".into())).unwrap();
    drop(writer);
    let state = directory.path().join("relay-state");
    assert_eq!(confirmed_saved_version(&state, document).unwrap().as_deref(), Some("next-frontier"));
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
