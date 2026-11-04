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
            pop_sharing_protocol::lifecycle::EpochState {
                seed_hash: "c".repeat(64),
                root: root.clone(),
                heads: root,
                handoff: None,
            },
            document.version(),
            None,
        );
        journal.transaction().unwrap().confirm(&receipt).unwrap();
        store.close().unwrap();
    }
    Arc::new(epochs::Access { journal: Arc::new(journal), identity: None })
}
#[test]
fn creation_retries_back_up_current_bytes_and_credentials_only_keep_link_and_ended() {
    let (_dir, owner) = fixture();
    let SeedBackup { path, hash: first_hash, .. } = stage(&owner).unwrap();
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
                .unwrap(),
            origin: pop_core::Origin::Window,
        },
    )
    .unwrap();
    let SeedBackup { path: retry, hash: next_hash, .. } = stage(&owner).unwrap();
    assert_ne!(path, retry);
    assert_ne!(first_hash, next_hash);
    let link = Invitation { room: "a".repeat(32), capability: "b".repeat(64) }.link().unwrap();
    let key = joined_credential(&link).unwrap();
    let json: serde_json::Value = serde_json::from_str(&key).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 2);
    assert_eq!(json["link"], link);
    assert_eq!(accept_invitation(Some(&key), &link).unwrap(), key);
    assert!(Client::restore(owner.clone(), &key, &_dir.path().join("state"), None).unwrap().status().link.is_some());
    assert!(Credential::decode(r#"{"invitation":{},"ended":false}"#).is_err());
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(path.parent().unwrap());
    let _ = std::fs::remove_file(&retry);
    let _ = std::fs::remove_dir(retry.parent().unwrap());
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn creator_resolution_validates_a_seed_without_a_local_owner_and_refuses_corrupt_bytes() {
    use pop_sharing_protocol::lifecycle::EpochState;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    for corrupt in [false, true] {
        let (dir, owner) = fixture();
        let document = owner.app().document_uuid.clone().unwrap();
        let SeedBackup { path, hash, version, app, .. } = stage(&owner).unwrap();
        let mut seed = std::fs::read(&path).unwrap();
        let info = RoomInfo {
            document: document.clone(),
            phase: RoomPhase::Live,
            seed_hash: Some(hash.clone()),
            seed_size: Some(seed.len() as u32),
            app: Some(app),
            capability: Some("b".repeat(64)),
            compact: false,
        };
        let epoch = EpochState { seed_hash: hash, root: version.clone(), heads: version, handoff: None };
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
        drop(owner);
        if corrupt {
            seed[0] ^= 1;
        }
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", server.local_addr().unwrap());
        server.set_nonblocking(true).unwrap();
        let serving = thread::spawn(move || {
            for body in [
                serde_json::to_vec(&Some(AccountRoom { room: "a".repeat(32) })).unwrap(),
                serde_json::to_vec(&info).unwrap(),
                serde_json::to_vec(&epoch).unwrap(),
                seed,
            ] {
                let deadline = Instant::now() + std::time::Duration::from_secs(5);
                let (mut stream, _) = loop {
                    match server.accept() {
                        Ok(peer) => break peer,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                            thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(error) => panic!("creator resolution did not reach its next request: {error}"),
                    }
                };
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                let header = String::from_utf8(header).unwrap();
                assert!(header.to_ascii_lowercase().contains("authorization: bearer identity-token\r\n"));
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                    .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let resolved = resolution::resolve_at(&document, "identity-token", &origin);
        serving.join().unwrap();
        let destination = dir.path().join("resolved.pop");
        if corrupt {
            assert!(resolved.is_err());
            assert!(!destination.exists());
        } else {
            let resolved = resolved.unwrap().unwrap();
            assert_eq!(resolved.document(), document);
            resolved.install(&destination, &dir.path().join("relay-state")).unwrap();
            let installed = Store::open(&destination, Mode::Snapshot).unwrap();
            assert_eq!(installed.app().document_uuid.as_deref(), Some(document.as_str()));
            assert_eq!(installed.share().unwrap().unwrap().room, "a".repeat(32));
            installed.close().unwrap();
            assert!(
                resolved.install(&destination, &dir.path().join("relay-state")).is_err(),
                "resolution cannot overwrite existing local state"
            );
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
}

#[test]
fn untrusted_invitations_are_refused_before_installing_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("missing.pop");
    assert!(install(&target, "https://attacker.example/join#secret", &dir.path().join("relay-state")).is_err());
    assert!(!target.exists());
}

#[test]
fn downloaded_seed_installs_a_new_replica_and_rebinds_its_room() {
    for predecessor in [None, Some("a".repeat(32))] {
        let (dir, owner) = fixture();
        if let Some(room) = predecessor {
            request(
                &owner,
                Request::Share { share: Share { room, endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into() } },
            )
            .unwrap();
        }
        let seed_path = dir.path().join("seed.pop");
        request(&owner, Request::Backup { destination: seed_path.clone() }).unwrap();
        let expected = snapshot::inspect(&seed_path).unwrap();
        let bytes = std::fs::read(&seed_path).unwrap();
        let key = RoomKey {
            room: "c".repeat(32),
            capability: "b".repeat(64),
            origin: pop_sharing_protocol::TRUSTED_ORIGIN.into(),
        };
        let mut info = RoomInfo {
            document: "00000000-0000-4000-8000-000000000000".into(),
            phase: RoomPhase::Live,
            seed_hash: Some(hex(&Sha256::digest(&bytes))),
            seed_size: Some(bytes.len() as u32),
            app: Some(hex(&expected.app_digest)),
            capability: None,
            compact: false,
        };
        let target = dir.path().join("guest/received.pop");
        assert!(install_seed(&target, &key, &info, &bytes).is_err(), "A different document identity is refused");
        assert!(!target.exists());
        assert_eq!(
            std::fs::read_dir(target.parent().unwrap()).unwrap().count(),
            0,
            "Failed installation is cleaned up"
        );

        info.document = expected.id.clone();
        install_seed(&target, &key, &info, &bytes).expect("A guest can install its first local copy");
        let replica = Owner::open(&target, Mode::Document, Arc::new(|_| {})).unwrap();
        assert_eq!(replica.app().document_uuid.as_deref(), Some(expected.id.as_str()));
        assert_eq!(replica.share().unwrap(), Some(Share { room: key.room, endpoint: key.origin }));
        request(&replica, Request::Close { preview: None, icon: None }).unwrap();
        let installed = snapshot::inspect(&target).unwrap();
        assert_eq!(installed.fingerprint, expected.fingerprint, "Installing preserves saved state and attachments");
        assert_eq!(installed.app_digest, expected.app_digest);
        assert_eq!(std::fs::read_dir(target.parent().unwrap()).unwrap().count(), 1, "Only the installed copy remains");
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    }
}

fn stalled_connection(
    owner: &Arc<Owner>,
) -> (transport::Connection, std::sync::mpsc::SyncSender<()>, std::thread::JoinHandle<()>) {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
        time::Duration,
    };
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let (entered, waiting) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let serving = thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{").unwrap();
        entered.send(()).unwrap();
        // Sender drop releases the peer too if setup or an assertion fails.
        let _ = gate.recv();
    });
    let key = RoomKey { room: "a".repeat(32), capability: "b".repeat(64), origin };
    let access = test_access(owner, &key, false);
    let connection = transport::Connection::start(owner.clone(), key, Arc::default(), access);
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    (connection, release, serving)
}

#[test]
fn stopping_a_connection_cancels_a_stalled_http_body() {
    use std::{sync::mpsc, thread, time::Duration};
    let (_dir, owner) = fixture();
    let (connection, release, serving) = stalled_connection(&owner);
    let (finished, stopped) = mpsc::sync_channel(1);
    let stopping = thread::spawn(move || {
        connection.stop();
        let _ = finished.send(());
    });
    let prompt = stopped.recv_timeout(Duration::from_secs(1)).is_ok();
    let _ = release.send(());
    serving.join().unwrap();
    stopping.join().unwrap();
    request(&owner, Request::Flush).unwrap();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    assert!(prompt, "stopping sharing must cancel an in-flight HTTP body before joining");
}

#[test]
fn stopping_bootstrap_preserves_another_owner_connection() {
    use pop_core::sync::{Frame, Permission, Status};
    use pop_runtime::owner::SyncEvent;
    use std::{sync::mpsc, time::Duration};
    let (_dir, owner) = fixture();
    request(
        &owner,
        Request::Share { share: Share { room: "a".repeat(32), endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into() } },
    )
    .unwrap();
    let (old, release, serving) = stalled_connection(&owner);
    let (send, events) = mpsc::channel();
    let replacement = owner.sync_connection(Arc::new(move |event| {
        let _ = send.send(event);
    }));
    let version = loop {
        if let SyncEvent::Send { bytes } = events.recv_timeout(Duration::from_secs(5)).unwrap()
            && let Frame::Hello { version, .. } = Frame::decode(&bytes).unwrap()
        {
            break version;
        }
    };
    old.stop();
    let _ = release.send(());
    serving.join().unwrap();
    assert!(
        replacement.frame(Frame::Welcome { permission: Permission::Write, version }.encode()),
        "stopping an unrelated bootstrap must preserve the replacement connection"
    );
    assert!(replacement.frame(Frame::CaughtUp.encode()));
    loop {
        if matches!(events.recv_timeout(Duration::from_secs(5)).unwrap(), SyncEvent::Status { status: Status::Synced })
        {
            break;
        }
    }
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
}

#[test]
fn opening_offline_catches_up_when_the_seed_service_returns_without_a_ui_retry() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::Duration,
    };
    let (dir, owner) = fixture();
    let baseline = dir.path().join("remote.pop");
    request(
        &owner,
        Request::Share { share: Share { room: "a".repeat(32), endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into() } },
    )
    .unwrap();
    request(&owner, Request::Backup { destination: baseline.clone() }).unwrap();
    let remote = Owner::open(&baseline, Mode::Document, Arc::new(|_| {})).unwrap();
    request(
        &remote,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":7}]}))
                .unwrap(),
            origin: pop_core::Origin::Agent,
        },
    )
    .unwrap();
    request(&remote, Request::Close { preview: None, icon: None }).unwrap();
    let seed = std::fs::read(&baseline).unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    server.set_nonblocking(true).unwrap();
    let online = Arc::new(AtomicBool::new(false));
    let attempted = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let flags = (online.clone(), attempted.clone(), running.clone());
    let accepted = Store::open(&baseline, Mode::Snapshot).unwrap();
    let checkpoint = accepted.document().unwrap().checkpoint().unwrap();
    let meta = loro::LoroDoc::decode_import_blob_meta(&checkpoint, true).unwrap();
    let root =
        pop_sharing_protocol::frame::version_pairs(meta.partial_end_vv.iter().map(|(&peer, &end)| (peer, end as u32)));
    accepted.close().unwrap();
    let size = seed.len() as u32;
    let seed_hash = hex(&Sha256::digest(&seed));
    let epoch = serde_json::to_vec(&pop_sharing_protocol::lifecycle::EpochState {
        seed_hash: seed_hash.clone(),
        root: root.clone(),
        heads: root,
        handoff: None,
    })
    .unwrap();
    let metadata = serde_json::to_vec(&RoomInfo {
        document: owner.app().document_uuid.clone().unwrap(),
        phase: RoomPhase::Live,
        seed_hash: Some(seed_hash),
        seed_size: Some(size),
        app: Some(hex(&Store::open(owner.path(), Mode::Snapshot).unwrap().app_digest().unwrap())),
        capability: None,
        compact: false,
    })
    .unwrap();
    let serving = thread::spawn(move || {
        while flags.2.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = server.accept() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            let mut request = vec![];
            while !request.ends_with(b"\r\n\r\n") && request.len() < 4096 {
                let mut byte = [0];
                if stream.read_exact(&mut byte).is_err() {
                    break;
                }
                request.push(byte[0]);
            }
            flags.1.store(true, Ordering::Relaxed);
            let is_seed = String::from_utf8_lossy(&request)
                .split_whitespace()
                .nth(1)
                .is_some_and(|path| path.split('?').next().unwrap_or_default().ends_with("/seed"));
            let body = if flags.0.load(Ordering::Relaxed) {
                if is_seed {
                    seed.as_slice()
                } else if String::from_utf8_lossy(&request).contains("/epoch ") {
                    epoch.as_slice()
                } else {
                    metadata.as_slice()
                }
            } else {
                &[]
            };
            let status = if body.is_empty() { 503 } else { 200 };
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Reply\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
        }
    });
    let key = RoomKey { room: "a".repeat(32), origin: format!("http://{address}"), capability: "b".repeat(64) };
    let access = test_access(&owner, &key, false);
    let connection = transport::Connection::start(owner.clone(), key, Arc::new(Mutex::new(None)), access);
    let result = || {
        let until = Instant::now() + Duration::from_secs(5);
        while !attempted.load(Ordering::Relaxed) && Instant::now() < until {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(attempted.load(Ordering::Relaxed), "Opening must try initial seed catch-up in the transport");
        let value = || {
            let Reply::State { reading, .. } = request(&owner, Request::State).unwrap() else {
                panic!("state");
            };
            reading.value["hits"].as_i64().unwrap()
        };
        assert_eq!(value(), 0);
        online.store(true, Ordering::Relaxed);
        let until = Instant::now() + Duration::from_secs(8);
        while value() != 7 && Instant::now() < until {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(value(), 7, "The accepted owner state must catch up without a UI action: {:?}", connection.status());
    };
    // Keep transport/server cleanup deterministic even when the assertion fails.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(result));
    connection.stop();
    running.store(false, Ordering::Relaxed);
    serving.join().unwrap();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    if let Err(error) = outcome {
        std::panic::resume_unwind(error);
    }
}

#[test]
fn account_deletion_authenticates_and_accepts_idempotent_success() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    assert!(delete_account("").is_err());
    assert!(delete_account(&"x".repeat(pop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES + 1)).is_err());
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        for status in [200, 200, 401, 500] {
            let (mut stream, _) = server.accept().unwrap();
            let mut request = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8(request).unwrap().to_lowercase();
            assert!(text.starts_with("delete /v1/account http/1.1\r\n"));
            assert!(text.contains("authorization: bearer local-fixture\r\n"));
            write!(stream, "HTTP/1.1 {status} Reply\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnull").unwrap();
        }
    });
    assert!(delete_account_at(&origin, "local-fixture").is_ok());
    assert!(delete_account_at(&origin, "local-fixture").is_ok());
    assert!(delete_account_at(&origin, "local-fixture").unwrap_err().contains("Sign in"));
    assert!(delete_account_at(&origin, "local-fixture").unwrap_err().contains("500"));
    thread.join().unwrap();
}

#[test]
fn rejoin_reuses_local_identity_only_after_the_previous_room_ended() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::{AtomicBool, AtomicU16, Ordering},
        time::Duration,
    };
    let (dir, owner) = fixture();
    let old = Invitation { room: "a".repeat(32), capability: "b".repeat(64) }.link().unwrap();
    let next = Invitation { room: "c".repeat(32), capability: "d".repeat(64) }.link().unwrap();
    let saved = joined_credential(&old).unwrap();
    let old_share = Share { room: "a".repeat(32), endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into() };
    request(&owner, Request::Share { share: old_share.clone() }).unwrap();
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":3}]}))
                .unwrap(),
            origin: pop_core::Origin::Window,
        },
    )
    .unwrap();
    let document = owner.app().document_uuid.clone().unwrap();
    let info = RoomInfo {
        document: document.clone(),
        phase: RoomPhase::Live,
        seed_hash: None,
        seed_size: None,
        app: None,
        capability: None,
        compact: false,
    };
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let unrelated = RoomInfo { document: "00000000-0000-4000-8000-000000000099".into(), ..info.clone() };
    let metadata = serde_json::to_vec(&info).unwrap();
    let wrong = serde_json::to_vec(&unrelated).unwrap();
    let old_status = Arc::new(AtomicU16::new(200));
    let wrong_document = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let flags = (old_status.clone(), wrong_document.clone(), running.clone());
    let thread = std::thread::spawn(move || {
        while flags.2.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = server.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            let mut request = vec![];
            while !request.ends_with(b"\r\n\r\n") && request.len() < 4096 {
                let mut byte = [0];
                if stream.read_exact(&mut byte).is_err() {
                    break;
                }
                request.push(byte[0]);
            }
            let previous = String::from_utf8_lossy(&request).starts_with(&format!("GET /v1/rooms/{} ", "a".repeat(32)));
            let status = if previous { flags.0.load(Ordering::Relaxed) } else { 200 };
            let body = if !previous && flags.1.load(Ordering::Relaxed) { &wrong } else { &metadata };
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
        }
    });
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert!(join_at(&owner, Some(&saved), &next, &origin).unwrap_err().contains("another live link"));
        old_status.store(503, Ordering::Relaxed);
        assert!(join_at(&owner, Some(&saved), &next, &origin).is_err());
        old_status.store(410, Ordering::Relaxed);
        wrong_document.store(true, Ordering::Relaxed);
        assert!(join_at(&owner, Some(&saved), &next, &origin).is_err());
        assert_eq!(owner.share().unwrap(), Some(old_share.clone()));
        wrong_document.store(false, Ordering::Relaxed);
        let accepted = join_at(&owner, Some(&saved), &next, &origin).unwrap();
        assert_eq!(owner.share().unwrap().unwrap().room, "c".repeat(32));
        assert_eq!(owner.app().document_uuid.as_deref(), Some(document.as_str()));
        assert!(Client::restore(owner.clone(), &accepted, &owner.path().parent().unwrap().join("state"), None).is_ok());
        // A replica restored on another device may have membership but no local key.
        request(&owner, Request::Share { share: old_share }).unwrap();
        assert!(join_at(&owner, None, &next, &origin).is_ok());
        assert!(join_at(&owner, None, &next, &origin).is_ok());
    }));
    running.store(false, Ordering::Relaxed);
    thread.join().unwrap();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    if let Err(error) = outcome {
        std::panic::resume_unwind(error);
    }
    let reopened = Store::open(&dir.path().join("document.pop"), Mode::Snapshot).unwrap();
    assert_eq!(reopened.app().document_uuid.as_deref(), Some(document.as_str()));
    assert_eq!(reopened.document().unwrap().reading().unwrap().value["hits"], 3);
    assert_eq!(reopened.share().unwrap().unwrap().room, "c".repeat(32));
}

/// The HTTP boundary preserves the binary base and both seed-format version headers.
#[test]
fn compact_posts_verified_bytes_and_preserves_conflict_and_size_refusals() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    for code in [204, 409, 413] {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", server.local_addr().unwrap());
        let serving = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap().to_lowercase();
            let length: usize =
                header.lines().find_map(|l| l.strip_prefix("content-length: ")).unwrap().parse().unwrap();
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            write!(stream, "HTTP/1.1 {code} Reply\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            (header, body)
        });
        let key = RoomKey { room: "a".repeat(32), capability: "b".repeat(64), origin };
        let base = pop_core::sync::Base {
            frontier: [(1, 42)].into_iter().collect(),
            start: [(1, 3)].into_iter().collect(),
            bytes: b"opaque updates".to_vec(),
        };
        let result = transport::compact(&key, &base);
        match (code, result) {
            (204, result) => assert!(result.is_ok()),
            (409, Err(transport::HttpFailure::Conflict)) => {}
            (413, Err(transport::HttpFailure::Paused(pause))) => {
                assert_eq!(pause.kind, pop_core::sync::PauseKind::Outgrown);
            }
            (code, _) => panic!("{code} was classified wrongly"),
        }
        let (header, bytes) = serving.join().unwrap();
        assert_eq!(bytes, base.bytes);
        assert!(header.starts_with(&format!("post /v1/rooms/{}/compact ", key.room)));
        for expected in [
            format!("authorization: bearer {}", key.capability),
            format!("x-sharing-sha256: {}", hex(&Sha256::digest(&base.bytes))),
            "x-sharing-version: [[\"0000000000000001\",42]]".into(),
            "x-sharing-root: [[\"0000000000000001\",3]]".into(),
            "content-type: application/octet-stream".into(),
        ] {
            assert!(header.contains(&expected), "{header}");
        }
    }
}

/// A conflict on create or open reads as a retry, not as an opaque status.
#[test]
fn conflicts_tell_the_person_to_retry() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let serving = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
        }
        write!(stream, "HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let key = RoomKey { room: "a".repeat(32), capability: "b".repeat(64), origin };
    let error = transport::http(&key, "GET", "", Some(&key.capability), &[], &[]).unwrap_err();
    serving.join().unwrap();
    assert_eq!(error, "Sharing was busy. Try again.");
}

/// A refused blob transfer reaches the connection as the same typed pause as every other relay call.
#[test]
fn a_blob_download_refused_for_size_pauses_files_full() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let (_dir, owner) = fixture();
    let hash = "c".repeat(64);
    let listing = serde_json::to_vec(&serde_json::json!([
        {"key": hash, "hash": hash, "size": 4, "media_type": "application/octet-stream"}
    ]))
    .unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let serving = std::thread::spawn(move || {
        let mut paths = vec![];
        for _ in 0..2 {
            let (mut stream, _) = server.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let line = String::from_utf8(header).unwrap().lines().next().unwrap().to_owned();
            let (code, body) = if line.contains("/blobs ") { (200, listing.as_slice()) } else { (413, &[][..]) };
            write!(stream, "HTTP/1.1 {code} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                .unwrap();
            stream.write_all(body).unwrap();
            paths.push(line);
        }
        paths
    });
    let key = RoomKey { room: "a".repeat(32), capability: "b".repeat(64), origin };
    let result = resources::download_blobs(&owner, &key, &mut Vec::new());
    let paths = serving.join().unwrap();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    assert!(paths[1].starts_with(&format!("GET /v1/rooms/{}/blob/{hash} ", key.room)), "{paths:?}");
    match result {
        Err(transport::HttpFailure::Paused(pause)) => {
            assert_eq!(pause.kind, pop_core::sync::PauseKind::FilesFull);
        }
        _ => panic!("a refused blob download must pause as files full"),
    }
}

#[test]
fn creator_handoff_keeps_frozen_remote_edits_before_revoking_the_room() {
    use pop_sharing_protocol::{
        frame::{Frame, Record, Span},
        lifecycle::{EpochState, Handoff},
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    let (dir, owner) = fixture();
    let seed = stage(&owner).unwrap();
    let saved = Store::open(&seed.path, Mode::Snapshot).unwrap();
    let mut remote = saved.document().unwrap();
    remote
        .apply_batch(
            serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":7}]}))
                .unwrap(),
            pop_core::Origin::Window,
        )
        .unwrap();
    let heads = remote.wire_history();
    let history = Frame::Records(vec![Record {
        spans: heads
            .iter()
            .filter(|(_, end)| *end > 0)
            .map(|(peer, end)| Span { peer: u64::from_str_radix(peer, 16).unwrap(), start: 0, end: *end })
            .collect(),
        bytes: remote.checkpoint().unwrap(),
    }])
    .encode();
    saved.close().unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let key = RoomKey {
        origin: format!("http://{}", server.local_addr().unwrap()),
        room: "a".repeat(32),
        capability: "b".repeat(64),
    };
    let document = owner.app().document_uuid.clone().unwrap();
    let info = RoomInfo {
        document: document.clone(),
        phase: RoomPhase::Live,
        seed_hash: Some(seed.hash.clone()),
        seed_size: Some(std::fs::metadata(&seed.path).unwrap().len() as u32),
        app: Some(seed.app.clone()),
        capability: Some(key.capability.clone()),
        compact: false,
    };
    let initial = EpochState { seed_hash: seed.hash.clone(), root: seed.version.clone(), heads, handoff: None };
    let bytes = std::fs::read(&seed.path).unwrap();
    let serving = thread::spawn(move || {
        let mut epoch = initial;
        let mut ended = false;
        let mut ended_reads = 0;
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let (mut stream, _) = loop {
                match server.accept() {
                    Ok(peer) => break peer,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                        thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(error) => panic!("handoff did not finish: {error}"),
                }
            };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut b = [0];
                stream.read_exact(&mut b).unwrap();
                header.push(b[0]);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(header.to_ascii_lowercase().contains("authorization: bearer identity-token\r\n"));
            let length = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let route = header.lines().next().unwrap().split_whitespace().nth(1).unwrap();
            let finalized = route.ends_with("/handoff/finalize");
            if ended {
                assert!(route.ends_with("/epoch"));
                write!(stream, "HTTP/1.1 410 Gone\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                ended_reads += 1;
                if ended_reads == 2 {
                    return;
                }
                continue;
            }
            let (response, extra) = if route.ends_with("/handoff/begin") {
                let begin: pop_sharing_protocol::lifecycle::BeginHandoff = serde_json::from_slice(&body).unwrap();
                assert_eq!(begin.expected.heads, epoch.heads);
                let handoff = Handoff {
                    token: begin.token,
                    seed_hash: epoch.seed_hash.clone(),
                    root: epoch.root.clone(),
                    heads: epoch.heads.clone(),
                };
                epoch.handoff = Some(handoff.clone());
                (serde_json::to_vec(&handoff).unwrap(), "")
            } else if route.ends_with("/epoch") {
                (serde_json::to_vec(&epoch).unwrap(), "")
            } else if route.contains("/seed?") {
                (bytes.clone(), "")
            } else if route.ends_with("/blobs") {
                (b"[]".to_vec(), "")
            } else if route.contains("/handoff/history") {
                (history.clone(), "X-Sharing-Done: 1\r\nX-Sharing-Next: 1\r\n")
            } else if finalized {
                (vec![], "")
            } else {
                (serde_json::to_vec(&info).unwrap(), "")
            };
            let status = if finalized { "503 Unavailable" } else { "200 OK" };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                response.len()
            )
            .unwrap();
            stream.write_all(&response).unwrap();
            // The relay committed Ended, but its successful acknowledgement was
            // lost. Retrying must probe that result, never publish another token.
            if finalized {
                ended = true;
            }
        }
    });
    let credential = Credential {
        link: Invitation { room: key.room.clone(), capability: key.capability.clone() }.link().unwrap(),
        ended: false,
    };
    // Loopback origin remains private test transport configuration, never a native credential field.
    let access = test_access(&owner, &key, false);
    let client = Client {
        owner: owner.clone(),
        key: Mutex::new(credential),
        connection: Mutex::new(None),
        creator: std::sync::atomic::AtomicBool::new(true),
        notice: Arc::new(Mutex::new(None)),
        access,
    };
    // Native credentials pin production origin. Use the same public lifecycle over a
    // test-scoped endpoint by recording it in the device journal's namespace.
    let frozen = client.freeze_at("identity-token", &key).unwrap();
    assert!(owner.share().unwrap().is_none() || owner.share().unwrap().unwrap().room == key.room);
    assert!(client.finalize_at("identity-token", &frozen, None, &key).is_err());
    assert_eq!(
        owner.share().unwrap(),
        Some(Share { room: key.room.clone(), endpoint: key.origin.clone() }),
        "an uncertain finalize must retain the frozen lineage"
    );
    let canonical = owner.path().to_owned();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    let resumed_owner = Arc::new(Owner::open(&canonical, Mode::Document, Arc::new(|_| {})).unwrap());
    let resumed = Client {
        owner: resumed_owner.clone(),
        key: Mutex::new(client.key.lock().unwrap().clone()),
        connection: Mutex::new(None),
        creator: std::sync::atomic::AtomicBool::new(true),
        notice: Arc::new(Mutex::new(None)),
        access: test_access(&resumed_owner, &key, false),
    };
    let retry_freeze = resumed.freeze_at("identity-token", &key).unwrap();
    assert_eq!(retry_freeze, frozen);
    resumed.finalize_at("identity-token", &retry_freeze, None, &key).unwrap();
    serving.join().unwrap();
    assert!(resumed_owner.share().unwrap().is_none());
    request(&resumed_owner, Request::Close { preview: None, icon: None }).unwrap();
    let reopened = Store::open(&canonical, Mode::Snapshot).unwrap();
    assert_eq!(reopened.document().unwrap().reading().unwrap().value["hits"], 7);
    reopened.close().unwrap();
    let _ = std::fs::remove_file(&seed.path);
    let _ = std::fs::remove_dir(seed.path.parent().unwrap());
    drop(dir);
}

#[test]
fn fresh_publication_retains_unsent_state_and_referenced_files_across_reopen() {
    use pop_sharing_protocol::lifecycle::{EpochBase, EpochState, PublishEpoch};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    let (directory, owner) = fixture();
    let original = Store::open(owner.path(), Mode::Snapshot).unwrap();
    let heads = original.document().unwrap().wire_history();
    original.close().unwrap();
    let Reply::Attachment { item } =
        request(&owner, Request::PutAttachment { bytes: b"newly referenced file".to_vec() }).unwrap()
    else {
        panic!("missing stored file")
    };
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[
                {"type":"increment","path":["hits"],"by":9},
                {"type":"set","path":["title"],"value":format!("attachment/{}", item.id)}
            ]}))
            .unwrap(),
            origin: pop_core::Origin::Window,
        },
    )
    .unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let key = RoomKey {
        origin: format!("http://{}", server.local_addr().unwrap()),
        room: "a".repeat(32),
        capability: "b".repeat(64),
    };
    let expected = EpochBase { seed_hash: "c".repeat(64), heads };
    let server_expected = expected.clone();
    let candidate_path = directory.path().join("server-candidate.pop");
    let server_file = item.clone();
    let serving = thread::spawn(move || {
        let deadline = Instant::now() + std::time::Duration::from_secs(8);
        let mut candidate_hash = None;
        let mut published: Option<EpochState> = None;
        let mut uploaded_file = false;
        loop {
            let (mut stream, _) = loop {
                match server.accept() {
                    Ok(peer) => break peer,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                        thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(error) => panic!("fresh publication did not finish: {error}"),
                }
            };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut b = [0];
                stream.read_exact(&mut b).unwrap();
                header.push(b[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let length = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let first = header.lines().next().unwrap();
            let route = first.split_whitespace().nth(1).unwrap();
            let read_published = route.ends_with("/epoch") && first.starts_with("GET ");
            let response = if first.starts_with("PUT ") && route.contains("/blobs/") {
                assert!(route.ends_with(&server_file.id));
                assert_eq!(body, b"newly referenced file");
                uploaded_file = true;
                vec![]
            } else if first.starts_with("PUT ") {
                let hash = hex(&Sha256::digest(&body));
                assert!(route.ends_with(&hash));
                std::fs::write(&candidate_path, body).unwrap();
                candidate_hash = Some(hash);
                vec![]
            } else if first.starts_with("POST ") {
                let publication: PublishEpoch = serde_json::from_slice(&body).unwrap();
                publication.validate().unwrap();
                assert_eq!(publication.expected, server_expected);
                assert_eq!(Some(&publication.seed_hash), candidate_hash.as_ref());
                let candidate = Store::open(&candidate_path, Mode::Snapshot).unwrap();
                assert_eq!(publication.root, candidate.document().unwrap().wire_history());
                assert_eq!(candidate.attachment(&server_file.id).unwrap(), b"newly referenced file");
                assert!(uploaded_file);
                published = Some(EpochState {
                    seed_hash: publication.seed_hash,
                    root: publication.root.clone(),
                    heads: publication.root,
                    handoff: None,
                });
                candidate.close().unwrap();
                vec![]
            } else {
                serde_json::to_vec(published.as_ref().unwrap()).unwrap()
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len())
                .unwrap();
            stream.write_all(&response).unwrap();
            if read_published {
                return;
            }
        }
    });
    epochs::publish(&owner, &key, &test_access(&owner, &key, false), expected).unwrap();
    serving.join().unwrap();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    let reopened = Store::open(owner.path(), Mode::Snapshot).unwrap();
    assert_eq!(reopened.document().unwrap().reading().unwrap().value["hits"], 9);
    assert_eq!(reopened.attachment(&item.id).unwrap(), b"newly referenced file");
    reopened.close().unwrap();
}

#[test]
fn legacy_initial_sharing_stage_preserves_saved_identity_and_state() {
    let (_directory, owner) = legacy_fixture();
    let bytes = b"legacy undo attachment";
    let Reply::Attachment { item } = request(&owner, Request::PutAttachment { bytes: bytes.to_vec() }).unwrap() else {
        panic!("attachment")
    };
    let reference = format!("attachment/{}", item.id);
    let title = |value: &str| Request::Apply {
        batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"set","path":["title"],"value":value}]}))
            .unwrap(),
        origin: pop_core::Origin::Window,
    };
    request(&owner, title(&reference)).unwrap();
    request(&owner, Request::Flush).unwrap();
    request(&owner, title("abc")).unwrap();
    request(&owner, Request::Flush).unwrap();
    let before = Store::open(owner.path(), Mode::Snapshot).unwrap();
    let version = before.document().unwrap().version();
    let value = before.document().unwrap().reading().unwrap().value;
    before.close().unwrap();
    let seed = stage(&owner).unwrap();
    let staged = Store::open(&seed.path, Mode::Snapshot).unwrap();
    let document = staged.document().unwrap();
    assert!(!document.supports_deep_compaction());
    assert_eq!(document.version(), version);
    assert_eq!(document.reading().unwrap().value, value);
    staged.close().unwrap();
    request(
        &owner,
        Request::AdoptEpoch { source: seed.path.clone().into(), expected_version: version, recovery_destination: None },
    )
    .unwrap();
    request(&owner, Request::Undo { redo: false }).unwrap();
    request(&owner, Request::Flush).unwrap();
    let Reply::State { reading, .. } = request(&owner, Request::State).unwrap() else { panic!("state") };
    assert_eq!(reading.value["title"], reference, "same-version adoption preserves session undo");
    let Reply::Bytes { bytes: restored } = request(&owner, Request::ReadAttachment { id: item.id }).unwrap() else {
        panic!("attachment bytes")
    };
    assert_eq!(restored.as_deref(), Some(bytes.as_slice()), "undo-restored attachment references must remain readable");
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    let _ = std::fs::remove_file(&seed.path);
    let _ = std::fs::remove_dir(seed.path.parent().unwrap());
}

#[test]
fn handoff_finalization_refuses_live_edits_newer_than_the_cloud_receipt() {
    use pop_sharing_protocol::lifecycle::{EpochState, Handoff};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        thread,
    };
    let (directory, owner) = fixture();
    let relay = Share { room: "a".repeat(32), endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into() };
    request(&owner, Request::Share { share: relay.clone() }).unwrap();
    request(&owner, Request::Flush).unwrap();
    let saved = Store::open(owner.path(), Mode::Snapshot).unwrap();
    let document = saved.document().unwrap();
    let version = document.version();
    let frozen = Handoff {
        token: "d".repeat(32),
        seed_hash: "c".repeat(64),
        root: document.wire_history(),
        heads: document.wire_history(),
    };
    saved.close().unwrap();
    let freeze = serde_json::to_string(&frozen).unwrap();
    let receipt = pop_library::CatalogReceipt {
        transition: "e".repeat(32),
        room: relay.room.clone(),
        freeze: freeze.clone(),
        snapshot_version: version.clone(),
    };
    // A real failed save keeps accepted edits live while the durable snapshot is
    // still exactly the one covered by the earlier CloudKit receipt.
    let lock = rusqlite::Connection::open(owner.path()).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
                .unwrap(),
            origin: pop_core::Origin::Window,
        },
    )
    .unwrap();
    assert!(request(&owner, Request::Flush).is_err());
    lock.execute_batch("ROLLBACK").unwrap();
    let disk = Store::open(owner.path(), Mode::Snapshot).unwrap();
    assert_eq!(disk.document().unwrap().version(), version);
    disk.close().unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let key = RoomKey {
        origin: format!("http://{}", server.local_addr().unwrap()),
        room: relay.room.clone(),
        capability: "b".repeat(64),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let revoked = Arc::new(AtomicUsize::new(0));
    let (server_stop, server_revoked) = (stop.clone(), revoked.clone());
    struct StopOnDrop(Arc<AtomicBool>);
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let _cleanup = StopOnDrop(stop.clone());
    let serving = thread::spawn(move || {
        while !server_stop.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = server.accept() else {
                thread::sleep(std::time::Duration::from_millis(2));
                continue;
            };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let length = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or(0);
            stream.read_exact(&mut vec![0; length]).unwrap();
            let response = if header.starts_with("POST ") {
                server_revoked.fetch_add(1, Ordering::Relaxed);
                vec![]
            } else {
                serde_json::to_vec(&EpochState {
                    seed_hash: frozen.seed_hash.clone(),
                    root: frozen.root.clone(),
                    heads: frozen.heads.clone(),
                    handoff: Some(frozen.clone()),
                })
                .unwrap()
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len())
                .unwrap();
            stream.write_all(&response).unwrap();
        }
    });
    let client = Client {
        owner: owner.clone(),
        key: Mutex::new(Credential {
            link: Invitation { room: key.room.clone(), capability: key.capability.clone() }.link().unwrap(),
            ended: false,
        }),
        connection: Mutex::new(None),
        creator: std::sync::atomic::AtomicBool::new(true),
        notice: Arc::new(Mutex::new(None)),
        access: test_access(&owner, &key, false),
    };
    let result = client.finalize_at("identity-token", &freeze, Some(&receipt), &key);
    let first_revoked = revoked.load(Ordering::Relaxed);
    let first_binding = owner.share().unwrap();
    let Reply::State { reading, .. } = request(&owner, Request::State).unwrap() else { panic!("owner state") };
    // This represents the explicit same-origin CloudKit restage acknowledging the
    // newer flushed payload, without opening a room or changing the frozen token.
    let refreshed = pop_library::CatalogReceipt { snapshot_version: reading.version, ..receipt };
    let resumed = client.finalize_at("identity-token", &freeze, Some(&refreshed), &key);
    stop.store(true, Ordering::Relaxed);
    serving.join().unwrap();
    assert!(result.is_err(), "newer accepted local edits must refuse the earlier durable cloud receipt");
    assert_eq!(first_revoked, 0, "the relay remains available until the newer state is durable in CloudKit");
    assert_eq!(first_binding, Some(relay));
    resumed.unwrap();
    assert_eq!(revoked.load(Ordering::Relaxed), 1);
    assert_eq!(owner.share().unwrap().unwrap().endpoint, pop_storage::LIBRARY_SHARE_ENDPOINT);
    request(&owner, Request::Flush).unwrap();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    let reopened = Store::open(owner.path(), Mode::Snapshot).unwrap();
    assert_eq!(reopened.document().unwrap().reading().unwrap().value["hits"], 1);
    reopened.close().unwrap();
    drop(directory);
}

#[test]
fn completed_recoveries_remain_discoverable_after_failed_install_edit_retry_and_restart() {
    let (directory, owner) = fixture();
    let seed = stage(&owner).unwrap();
    let epoch = pop_sharing_protocol::lifecycle::EpochState {
        seed_hash: seed.hash.clone(),
        root: seed.version.clone(),
        heads: seed.version.clone(),
        handoff: None,
    };
    let key = RoomKey {
        origin: pop_sharing_protocol::TRUSTED_ORIGIN.into(),
        room: "a".repeat(32),
        capability: "b".repeat(64),
    };
    let state_directory = directory.path().join("relay-state");
    let journal = journal::Journal::new(&state_directory, &key, owner.app().document_uuid.as_deref().unwrap()).unwrap();
    let edit = || Request::Apply {
        batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
            .unwrap(),
        origin: pop_core::Origin::Window,
    };
    request(&owner, edit()).unwrap();
    request(&owner, Request::Flush).unwrap();
    // Recovery writes a separate SQLite file successfully; the original writer
    // cannot begin canonical installation while another SQLite writer holds it.
    let lock = rusqlite::Connection::open(owner.path()).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert!(epochs::install_prepared(&owner, &seed, &epoch, &journal).is_err());
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    let credential = Credential {
        link: Invitation { room: key.room.clone(), capability: key.capability.clone() }.link().unwrap(),
        ended: false,
    }
    .encode()
    .unwrap();
    let client = Client::restore(owner.clone(), &credential, &state_directory, None).unwrap();
    let pending_path = client.status().recovery_path.expect("completed pending recovery is immediately visible");
    let pending_copy = Store::open(Path::new(&pending_path), Mode::Snapshot).unwrap();
    assert_eq!(pending_copy.document().unwrap().reading().unwrap().value["hits"], 1);
    pending_copy.close().unwrap();
    drop(client);
    request(&owner, edit()).unwrap();
    request(&owner, Request::Flush).unwrap();
    epochs::install_prepared(&owner, &seed, &epoch, &journal).unwrap();
    let canonical = owner.path().to_owned();
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    drop(owner);
    drop(journal);
    // Startup discovery must not depend on whichever receipt happened to win the
    // single rolling journal slot, or a UI poll before that slot was overwritten.
    let mut retained = vec![];
    for path in recovered_documents(&state_directory).unwrap() {
        assert!(Path::new(&path).starts_with(state_directory.join("Recovered")));
        let store = Store::open(Path::new(&path), Mode::Snapshot).unwrap();
        assert!(store.app().document_uuid.is_some());
        assert!(store.share().unwrap().is_none());
        retained.push(store.document().unwrap().reading().unwrap().value["hits"].as_i64().unwrap());
        store.close().unwrap();
    }
    retained.sort();
    assert_eq!(
        retained,
        [1, 2],
        "every completed recovery must remain discoverable across journal rollover and restart"
    );
    let reopened = Store::open(&canonical, Mode::Snapshot).unwrap();
    assert_eq!(reopened.document().unwrap().reading().unwrap().value["hits"], 0);
    reopened.close().unwrap();
    let _ = std::fs::remove_file(&seed.path);
    let _ = std::fs::remove_dir(seed.path.parent().unwrap());
}
