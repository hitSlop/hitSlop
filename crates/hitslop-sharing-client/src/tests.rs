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
fn creation_retries_back_up_current_bytes_and_credentials_only_keep_link_and_ended() {
    let (_dir, owner) = fixture();
    let SeedBackup { path, hash: first_hash, .. } = stage(&owner).unwrap();
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
                .unwrap(),
            origin: hitslop_core::Origin::Window,
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
    assert!(Client::restore(owner.clone(), &key).unwrap().status().link.is_some());
    assert!(Credential::decode(r#"{"invitation":{},"ended":false}"#).is_err());
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(path.parent().unwrap());
    let _ = std::fs::remove_file(&retry);
    let _ = std::fs::remove_dir(retry.parent().unwrap());
    request(&owner, Request::Close { preview: None, icon: None }).unwrap();
}
#[test]
fn untrusted_invitations_are_refused_before_installing_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("missing.slop");
    assert!(install(&target, "https://attacker.example/join#secret").is_err());
    assert!(!target.exists());
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
    let baseline = dir.path().join("remote.slop");
    request(
        &owner,
        Request::Share {
            share: Share { room: "a".repeat(32), endpoint: hitslop_sharing_protocol::TRUSTED_ORIGIN.into() },
        },
    )
    .unwrap();
    request(&owner, Request::Backup { destination: baseline.clone() }).unwrap();
    let remote = Owner::open(&baseline, Mode::Document, Arc::new(|_| {})).unwrap();
    request(
        &remote,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":7}]}))
                .unwrap(),
            origin: hitslop_core::Origin::Agent,
        },
    )
    .unwrap();
    request(&remote, Request::Close { preview: None, icon: None }).unwrap();
    let seed = std::fs::read(baseline).unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    server.set_nonblocking(true).unwrap();
    let online = Arc::new(AtomicBool::new(false));
    let attempted = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let flags = (online.clone(), attempted.clone(), running.clone());
    let size = seed.len() as u32;
    let seed_hash = hex(&Sha256::digest(&seed));
    let metadata = serde_json::to_vec(&RoomInfo {
        document: owner.app().document_uuid.clone().unwrap(),
        phase: RoomPhase::Live,
        seed_hash: Some(seed_hash),
        seed_size: Some(size),
        app: Some(hex(&Store::open(owner.path(), Mode::Snapshot).unwrap().app_digest().unwrap())),
        capability: None,
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
            let is_seed =
                String::from_utf8_lossy(&request).starts_with(&format!("GET /v1/rooms/{}/seed ", "a".repeat(32)));
            let body = if flags.0.load(Ordering::Relaxed) {
                if is_seed { seed.as_slice() } else { metadata.as_slice() }
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
    let connection = transport::Connection::start(owner.clone(), key, Arc::new(Mutex::new(None)));
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
    assert!(delete_account(&"x".repeat(hitslop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES + 1)).is_err());
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
    let old_share = Share { room: "a".repeat(32), endpoint: hitslop_sharing_protocol::TRUSTED_ORIGIN.into() };
    request(&owner, Request::Share { share: old_share.clone() }).unwrap();
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":3}]}))
                .unwrap(),
            origin: hitslop_core::Origin::Window,
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
        assert!(Client::restore(owner.clone(), &accepted).is_ok());
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
    let reopened = Store::open(&dir.path().join("document.slop"), Mode::Snapshot).unwrap();
    assert_eq!(reopened.app().document_uuid.as_deref(), Some(document.as_str()));
    assert_eq!(reopened.document().unwrap().reading().unwrap().value["hits"], 3);
    assert_eq!(reopened.share().unwrap().unwrap().room, "c".repeat(32));
}
