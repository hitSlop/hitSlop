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
}
