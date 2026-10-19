use super::*;
fn fixture() -> (tempfile::TempDir, Arc<Owner>) {
    static REGISTRY: std::sync::Once = std::sync::Once::new();
    REGISTRY.call_once(|| {
        hitslop_core::registry::use_folder(&std::env::temp_dir().join("hitslop-sharing-test-registry")).unwrap()
    });
    let dir = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/checklist/document");
    let template = dir.path().join("template.slop");
    hitslop_core::file::pack(&std::fs::read_to_string(source.join("input.json")).unwrap(), &source, &template, None)
        .unwrap();
    let path = dir.path().join("document.slop");
    hitslop_core::file::create_document(&template, &path).unwrap();
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    (dir, owner)
}
#[test]
fn staged_creation_retries_the_same_seed_after_edits_and_keeps_creator_authority() {
    let (dir, owner) = fixture();
    let before = owner.app().document_uuid.clone();
    let pending = Client::prepare(&owner, &dir.path().join("seeds")).unwrap();
    assert!(owner.share().unwrap().is_none());
    let staged = Client::stage(&owner, &pending).unwrap();
    let key = Credential::decode(&staged).unwrap();
    let seed = std::fs::read(&key.seed).unwrap();
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
                .unwrap(),
            origin: hitslop_core::Origin::Window,
        },
    )
    .unwrap();
    let retried = Client::stage(&owner, &staged).unwrap();
    assert_eq!(retried, staged);
    assert_eq!(std::fs::read(&key.seed).unwrap(), seed);
    assert_eq!(owner.app().document_uuid, before);
    assert!(Client::restore(owner.clone(), &staged).unwrap().status().link.is_none());
    let link = key.invitation.link().unwrap();
    assert_eq!(accept_invitation(Some(&staged), &link).unwrap(), staged);
    let mut other = key.invitation.clone();
    other.room = "b".repeat(32);
    assert!(accept_invitation(Some(&staged), &other.link().unwrap()).is_err());
    let recipient = joined_credential(&link).unwrap();
    assert!(Credential::decode(&recipient).unwrap().admin.is_empty());
    std::fs::remove_file(key.seed.with_extension("staging")).unwrap();
    assert!(Client::stage(&owner, &staged).unwrap_err().contains("original device"));
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn untrusted_invitations_are_refused_before_installing_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("missing.slop");
    assert!(install(&target, "https://attacker.example/join#secret").is_err());
    assert!(!target.exists());
}
