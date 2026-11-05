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
pub(super) fn legacy_fixture() -> (tempfile::TempDir, Arc<Owner>) {
    use loro::{LoroCounter, LoroMap, LoroMovableList, LoroText};
    let (directory, owner) = fixture();
    let path = owner.path().to_owned();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    drop(owner);
    let store = Store::open(&path, Mode::Document).unwrap();
    // An independent released-layout writer, with the fixture's accepted schema.
    let legacy = loro::LoroDoc::new();
    legacy.get_map("meta").insert("layout", 1).unwrap();
    let data = legacy.get_map("data");
    data.insert_container("title", LoroText::new()).unwrap().insert(0, "abc").unwrap();
    data.insert_container("hits", LoroCounter::new()).unwrap();
    let rows = data.insert_container("rows", LoroMap::new()).unwrap();
    let values = rows.insert_container("rows", LoroMap::new()).unwrap();
    let order = rows.insert_container("order", LoroMovableList::new()).unwrap();
    for (index, text) in ["A", "B"].into_iter().enumerate() {
        let id = format!("{:032x}", index + 1);
        let row = values.insert_container(&id, LoroMap::new()).unwrap();
        row.insert_container("text", LoroText::new()).unwrap().insert(0, text).unwrap();
        row.insert("done", false).unwrap();
        order.insert(index, id).unwrap();
    }
    let bytes = legacy.export(loro::ExportMode::Snapshot).unwrap();
    let mut document = pop_core::Document::open(store.app().app.spec(), &bytes, &[]).unwrap();
    let job = store.checkpoint_job(&mut document).unwrap();
    store.write(&job).unwrap();
    store.close().unwrap();
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    (directory, owner)
}
pub(super) fn test_access(owner: &Owner, key: &RoomKey, initialized: bool) -> Arc<epochs::Access> {
    let document = owner.app().document_uuid.as_deref().unwrap();
    let journal = journal::Journal::new(&owner.path().parent().unwrap().join("relay-state"), key, document).unwrap();
    if initialized {
        let store = Store::open(owner.path(), Mode::Snapshot).unwrap();
        let document = store.document().unwrap();
        let checkpoint = document.checkpoint().unwrap();
        let meta = loro::LoroDoc::decode_import_blob_meta(&checkpoint, true).unwrap();
        let root = pop_sharing_protocol::frame::version_pairs(
            meta.partial_end_vv.iter().map(|(&peer, &end)| (peer, end as u32)),
        );
        let receipt = journal.receipt(
            pop_sharing_protocol::lifecycle::EpochState { seed_hash: "c".repeat(64), root: root.clone(), heads: root },
            document.version(),
            None,
        );
        journal.transaction().unwrap().confirm(&receipt).unwrap();
        store.close().unwrap();
    }
    Arc::new(epochs::Access { journal: Arc::new(journal), identity: None })
}

#[test]
fn anonymous_credentials_are_explicit_uuid_invitations_and_never_identity_tokens() {
    let link =
        Invitation { room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(), capability: "b".repeat(64) }.link().unwrap();
    let encoded = joined_credential(&link).unwrap();
    let decoded = Credential::decode(&encoded).unwrap();
    assert_eq!(decoded.runtime().unwrap().capability(), Some("b".repeat(64).as_str()));
    assert!(decoded.runtime().unwrap().origin.starts_with("https://"));
    assert!(joined_credential("https://attacker.example/join#secret").is_err());
    assert_eq!(credential_document(&encoded).as_deref(), Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"));
    let ended = serde_json::to_string(&GuestCredential { link, ended: true }).unwrap();
    assert_eq!(credential_document(&ended), None);
    for invalid in ["identity-token", "{}", r#"{"link":"https://attacker.example/join#secret","ended":false}"#] {
        assert_eq!(credential_document(invalid), None);
    }
}

#[test]
fn pending_recovery_remains_openable_when_the_device_state_directory_moves() {
    let (directory, owner) = fixture();
    let document = owner.app().document_uuid.clone().unwrap();
    let key = RoomKey::identity(&document, "memory-only-test-token").unwrap();
    let access = test_access(&owner, &key, true);
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":7}]}))
                .unwrap(),
            origin: pop_core::Origin::Page,
        },
    )
    .unwrap();
    let original = access.journal.transaction().unwrap().current().unwrap().unwrap();
    let destination = access.journal.recovery_destination(&original.owner_version).unwrap();
    request(&owner, Request::Copy { destination: destination.clone(), preview: None, icon: None }).unwrap();
    let pending = access.journal.receipt(original.epoch, "pending-installed-version".into(), Some(destination));
    access.journal.transaction().unwrap().prepare(&pending).unwrap();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    drop(access);
    let moved = directory.path().join("relocated-state");
    std::fs::rename(directory.path().join("relay-state"), &moved).unwrap();
    let journal = journal::Journal::new(&moved, &key, &document).unwrap();
    let transaction = journal.transaction().unwrap();
    let pending = transaction.pending().unwrap().unwrap();
    let recovered = Store::open(pending.recovery.as_ref().unwrap(), Mode::Snapshot)
        .expect("pending recovery must resolve inside the moved device state directory");
    assert_ne!(recovered.app().document_uuid.as_deref(), Some(document.as_str()));
    assert_eq!(recovered.document().unwrap().reading().unwrap().value["hits"].as_f64(), Some(7.0));
    recovered.close().unwrap();
    transaction.promote(&pending).unwrap();
    let confirmed = transaction.current().unwrap().unwrap();
    let recovered = Store::open(confirmed.recovery.as_ref().unwrap(), Mode::Snapshot).unwrap();
    assert_eq!(recovered.document().unwrap().reading().unwrap().value["hits"].as_f64(), Some(7.0));
    recovered.close().unwrap();
}
