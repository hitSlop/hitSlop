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
fn immutable_local_seed_survives_retry_after_edits_without_persisting_admin_or_identity_tokens() {
    let (dir, owner) = fixture();
    let reservation = RoomReservation {
        document: owner.app().document_uuid.clone().unwrap(),
        room: "a".repeat(32),
        generation: 1,
        capability: Some("b".repeat(64)),
        previous_room: None,
        phase: RoomPhase::Pending,
        invitation: None,
    };
    let (path, init, invitation) = stage(&owner, &dir.path().join("seeds"), &reservation).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    request(
        &owner,
        Request::Apply {
            batch: serde_json::from_value(serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}))
                .unwrap(),
            origin: hitslop_core::Origin::Window,
        },
    )
    .unwrap();
    let (_, retry, same) = stage(&owner, &dir.path().join("seeds"), &reservation).unwrap();
    assert_eq!(init.seed_hash, retry.seed_hash);
    assert_eq!(invitation.seed_hash, same.seed_hash);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let key = Credential { invitation, ended: false }.encode().unwrap();
    let json: serde_json::Value = serde_json::from_str(&key).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 2);
    assert_eq!(accept_invitation(Some(&key), &same.link().unwrap()).unwrap(), key);
    assert!(Client::restore(owner.clone(), &key).unwrap().status().link.is_some());
    let mut other = reservation.clone();
    other.room = "c".repeat(32);
    assert!(validate_reservation_membership(owner.share().unwrap(), &other).is_err());
    assert!(stage(&owner, &dir.path().join("seeds"), &other).is_err());
    other.previous_room = Some(reservation.room);
    assert!(
        validate_reservation_membership(owner.share().unwrap(), &other).is_ok(),
        "An owner on the ended predecessor can cancel its pending replacement"
    );
    assert!(
        validate_reservation_membership(
            Some(Share { room: other.room.clone(), endpoint: "https://attacker.example".into() }),
            &other
        )
        .is_err()
    );
    assert!(stage(&owner, &dir.path().join("seeds"), &other).is_ok());
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
            let body = if flags.0.load(Ordering::Relaxed) { seed.as_slice() } else { &[] };
            let status = if body.is_empty() { 503 } else { 200 };
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Reply\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
        }
    });
    let key = Credential {
        invitation: Invitation {
            document: owner.app().document_uuid.clone().unwrap(),
            room: "a".repeat(32),
            origin: format!("http://{address}"),
            capability: "b".repeat(64),
            seed_hash,
            size,
        },
        ended: false,
    };
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
        assert_eq!(value(), 7, "The accepted owner state must catch up without a UI action: {}", connection.status());
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
