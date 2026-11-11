use super::*;
use crate::resolution;
use crate::resolution::ResolvedRoom;
use pop_sharing_protocol::http::RoomPhase;
pub(crate) fn fixture_source() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/checklist/document")
}
pub(crate) fn fixture() -> (tempfile::TempDir, Arc<Owner>) {
    let source = fixture_source();
    document(tempfile::tempdir().unwrap(), &std::fs::read_to_string(source.join("input.json")).unwrap(), &source)
}
/// A document packed from `input` with resource paths under `source`, kept in `dir`.
pub(crate) fn document(dir: tempfile::TempDir, input: &str, source: &Path) -> (tempfile::TempDir, Arc<Owner>) {
    crate::test_support::isolate_registry();
    let template = dir.path().join("template.pop");
    pop_storage::file::pack(input, source, &template, None).unwrap();
    let path = dir.path().join("document.pop");
    pop_storage::file::create_document(&template, &path).unwrap();
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {})).unwrap());
    (dir, owner)
}
pub(crate) fn receipt_at(root: &Path) -> receipts::Receipt {
    std::fs::create_dir_all(root).unwrap();
    let db = crate::library::index::open(root).unwrap();
    let db = Mutex::new(db);
    Arc::new(move |version| crate::library::index::confirm(&mut pop_core::lock(&db), "test", "test", &version))
}
pub(crate) fn test_receipt(owner: &Owner, _key: &RoomKey, _confirmed: bool) -> receipts::Receipt {
    receipt_at(&owner.path().parent().unwrap().join("index"))
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
fn attachment_heavy_documents_stage_a_small_seed_without_losing_source_blobs() {
    let (_directory, owner) = fixture();
    let mut ids = Vec::new();
    for byte in [17, 29] {
        let item = request(&owner, |owner: &Owner, options, callback| {
            owner.put_attachment(vec![byte; 9 << 20], options, callback)
        })
        .unwrap();
        ids.push(item.id);
    }
    request(&owner, |owner: &Owner, options, callback| {
        owner.apply(
            serde_json::from_value(
                serde_json::json!({"intents":[{"type":"set","path":["title"],"value":ids.join(" ")}]}),
            )
            .unwrap(),
            pop_core::Origin::Page,
            options,
            callback,
        )
    })
    .unwrap();
    let seed = stage(&owner).expect("valid attachments must not consume the seed budget");
    assert!(std::fs::metadata(&seed.path).unwrap().len() < pop_sharing_protocol::SEED_BYTES as u64);
    let wire = Store::open(&seed.path, Mode::Snapshot).unwrap();
    assert!(wire.attachments().unwrap().is_empty(), "attachments travel through the blob inventory");
    assert_eq!(wire.app().document_uuid, owner.app().document_uuid);
    let version = wire.document().unwrap().version();
    let app = wire.app_digest().unwrap();
    wire.close().unwrap();
    let source = Store::open(&seed.snapshot, Mode::Snapshot).unwrap();
    assert_eq!(source.attachments().unwrap().len(), 2, "the immutable upload snapshot retains its exact blobs");
    assert_eq!(source.document().unwrap().version(), version);
    assert_eq!(source.app_digest().unwrap(), app);
    source.close().unwrap();
    for id in ids {
        assert!(matches!(
            request(&owner, |owner: &Owner, options, callback| owner.read_attachment(id, options, callback)).unwrap(),
            bytes if !bytes.is_empty()
        ));
    }
    request(&owner, |owner: &Owner, options, callback| owner.close(None, None, options, callback)).unwrap();
}

#[test]
fn initial_download_hydrates_without_records_and_failed_download_never_publishes() {
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    let (directory, owner) = fixture();
    let blobs: Vec<Vec<u8>> = [17, 29].into_iter().map(|byte| vec![byte; 9 << 20]).collect();
    let mut inventory = Vec::new();
    for bytes in &blobs {
        let item =
            request(&owner, |owner: &Owner, options, callback| owner.put_attachment(bytes.clone(), options, callback))
                .unwrap();
        inventory.push(pop_sharing_protocol::resources::Resource {
            key: item.id.clone(),
            hash: item.id,
            size: item.bytes,
            media_type: item.media_type,
        });
    }
    request(&owner, |owner: &Owner, options, callback| owner.apply(serde_json::from_value(serde_json::json!({"intents":[{"type":"set","path":["title"],"value":inventory.iter().map(|item|item.hash.clone()).collect::<Vec<_>>().join(" ")}]})).unwrap(), pop_core::Origin::Page, options, callback)).unwrap();
    let seed = stage(&owner).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let key = RoomKey {
        room: owner.app().document_uuid.clone().unwrap(),
        identity: "test-token".into(),
        identity_updates: None,
        origin: format!("http://{}", listener.local_addr().unwrap()),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let refuse = Arc::new(AtomicBool::new(true));
    let serving_stop = stop.clone();
    let serving_refuse = refuse.clone();
    let expected = inventory.clone();
    let served_blobs = blobs.clone();
    let server = std::thread::spawn(move || {
        while !serving_stop.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(2));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let (status, body) = if header.contains("/blobs ") {
                (200, serde_json::to_vec(&expected).unwrap())
            } else if serving_refuse.load(Ordering::Relaxed) {
                (503, Vec::new())
            } else {
                let index = expected
                    .iter()
                    .position(|item| header.contains(&format!("/blob/{} ", item.hash)))
                    .expect("known blob");
                (200, served_blobs[index].clone())
            };
            write!(stream, "HTTP/1.1 {status} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    let destination = directory.path().join("download.pop");
    let state = directory.path().join("download-state");
    let resolved = ResolvedRoom {
        info: RoomInfo {
            document: key.room.clone(),
            phase: RoomPhase::Live,
            seed_hash: Some(seed.hash.clone()),
            seed_size: Some(std::fs::metadata(&seed.path).unwrap().len() as u32),
            app: Some(seed.app.clone()),
            compact: false,
        },
        key,
        seed: resolution::PrivateSeed { path: seed.path.clone() },
    };
    let interrupted = resolved.install_if_current(&destination, receipt_at(&state), || Ok(()));
    assert!(interrupted.is_err(), "a missing blob must refuse initial publication");
    assert!(!destination.exists(), "a failed initial download must not publish a partial document");
    refuse.store(false, Ordering::Relaxed);
    assert!(resolved.install_if_current(&destination, receipt_at(&state), || Err("account detached".into())).is_err());
    assert!(!destination.exists(), "an account changed during hydration must fence publication");
    resolved.install_if_current(&destination, receipt_at(&state), || Ok(())).unwrap();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let downloaded = Store::open(&destination, Mode::Snapshot).unwrap();
    assert_eq!(downloaded.app().document_uuid, owner.app().document_uuid);
    assert_eq!(hex(downloaded.app_digest().unwrap().as_bytes()), seed.app);
    for (item, bytes) in inventory.iter().zip(blobs) {
        assert_eq!(downloaded.attachment(&item.hash).unwrap(), bytes);
    }
    downloaded.close().unwrap();
    request(&owner, |owner: &Owner, options, callback| owner.close(None, None, options, callback)).unwrap();
}
