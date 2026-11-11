use super::*;
use crate::resolution;
use crate::resolution::ResolvedRoom;
use pop_contracts::relay::PauseKind;
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
pub(crate) fn test_journal(owner: &Owner, key: &RoomKey, confirmed: bool) -> Arc<journal::Journal> {
    let document = owner.app().document_uuid.as_deref().unwrap();
    let journal =
        journal::Journal::new(&owner.path().parent().unwrap().join("relay-state"), &key.origin, document).unwrap();
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
    assert!(matches!(journal::local(error), transport::HttpFailure { kind: PauseKind::Busy, .. }));
}

#[test]
fn failed_receipt_confirmation_keeps_the_previous_receipt() {
    let (_directory, owner) = fixture();
    let document = owner.app().document_uuid.as_deref().unwrap();
    let key = RoomKey::identity(document, "test-token").unwrap();
    request(&owner, |owner: &Owner, options, callback| {
        owner.set_share(pop_storage::store::Share { endpoint: key.origin.clone() }, options, callback)
    })
    .unwrap();
    let journal = test_journal(&owner, &key, true);
    let previous = journal.transaction().unwrap().current().unwrap().unwrap().owner_version;
    request(&owner, |owner: &Owner, options, callback| {
        owner.apply(
            serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
                .unwrap(),
            pop_core::Origin::Page,
            options,
            callback,
        )
    })
    .unwrap();
    request(&owner, Owner::flush).unwrap();
    let saved = Store::open(owner.path(), Mode::Snapshot).unwrap();
    let heads = saved.document().unwrap().wire_history();
    saved.close().unwrap();
    let version = heads.iter().map(|(peer, end)| (u64::from_str_radix(peer, 16).unwrap(), *end)).collect();
    let connection = owner.sync_connection(Arc::new(|_| {}));
    connection.frame(
        pop_contracts::relay::frame::Frame::Welcome {
            permission: pop_contracts::relay::frame::Permission::Write,
            version,
        }
        .encode(),
    );
    connection.frame(pop_contracts::relay::frame::Frame::CaughtUp.encode());
    // A version probe orders after the handshake messages.
    request(&owner, Owner::version).unwrap();
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
    request(&owner, |owner: &Owner, options, callback| owner.close(None, None, options, callback)).unwrap();
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
    let interrupted = resolved.install_if_current(&destination, &state, || Ok(()));
    assert!(interrupted.is_err(), "a missing blob must refuse initial publication");
    assert!(!destination.exists(), "a failed initial download must not publish a partial document");
    assert!(
        journal::Journal::inspect(&state, &resolved.key.origin, resolved.document())
            .unwrap()
            .inspection()
            .unwrap()
            .and_then(|tx| tx.current().unwrap())
            .is_none()
    );
    refuse.store(false, Ordering::Relaxed);
    assert!(resolved.install_if_current(&destination, &state, || Err("account detached".into())).is_err());
    assert!(!destination.exists(), "an account changed during hydration must fence publication");
    resolved.install_if_current(&destination, &state, || Ok(())).unwrap();
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
