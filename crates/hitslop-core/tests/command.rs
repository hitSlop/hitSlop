//! Shared live/closed command routing and bounded Unix transport. Fault peers exercise
//! real socket loss and identity checks, not private call ordering.
use hitslop_core::{
    command::{self, ExportCompletion, ExportHandler, ExportRequest},
    envelope::{self, Envelope},
    file,
    owner::{Failure, Owner, Reply, Request},
    registry,
    socket::{self, Server},
    store::{Mode, Store},
};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
mod support;
const SCHEMA: &str =
    r#"{"kind":"object","properties":{"title":{"kind":"text"},"hits":{"kind":"counter"}}}"#;
fn document() -> (tempfile::TempDir, PathBuf) {
    support::isolate_registry();
    let dir = tempfile::tempdir_in("/tmp").unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/app.js"), "export default {}").unwrap();
    support::write_app(
        &stage,
        support::App::new(SCHEMA, r#"{"title":"Initial","hits":0}"#),
    );
    let template = dir.path().join("template.slop");
    file::pack(&stage, &template).unwrap();
    let path = dir.path().join("doc.slop");
    file::create_document(&template, &path).unwrap();
    (dir, path)
}
fn request(path: &Path, method: &str) -> Value {
    json!({"method":method,"documentPath":std::fs::canonicalize(path).unwrap()})
}
fn batch(path: &Path) -> Value {
    let mut r = request(path, "batch");
    r["ops"] = r#"[{"type":"increment","path":["hits"],"by":1}]"#.into();
    r
}
fn checked(text: String) -> Value {
    assert!(
        envelope::is_valid(Envelope::SocketReply, text.as_bytes()),
        "invalid reply: {text}"
    );
    serde_json::from_str(&text).unwrap()
}
fn run(value: Value) -> Value {
    checked(command::request(&value.to_string(), None))
}
fn call(owner: &Owner, request: Request) -> std::result::Result<Reply, Failure> {
    let (tx, rx) = mpsc::channel();
    owner.submit(
        request,
        None,
        None,
        Box::new(move |r| {
            tx.send(r).unwrap();
        }),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}
fn close(owner: &Owner) {
    call(
        owner,
        Request::Close {
            preview: None,
            icon: None,
        },
    )
    .unwrap();
}
fn open(path: &Path) -> Arc<Owner> {
    Arc::new(Owner::open(path, Mode::Document, Arc::new(|_| {})).unwrap())
}
struct NoExport;
impl ExportHandler for NoExport {
    fn export(&self, _: ExportRequest, completion: Arc<ExportCompletion>) {
        completion.complete(Err(Failure {
            kind: hitslop_core::owner::FailureKind::Rejected,
            message: "No export".into(),
            reason: None,
            op_index: None,
        }));
    }
}

#[test]
fn closed_commands_edit_theme_data_and_attachments_then_reopen() {
    let (_dir, path) = document();
    let applied = run(batch(&path));
    assert_eq!(applied["method"], "batch");
    assert_eq!(applied["sequence"], 1);
    let mut theme = request(&path, "theme.set");
    theme["values"] = json!({"accent":"#123456"});
    assert_eq!(run(theme)["state"]["effective"]["accent"], "#123456");
    let mut put = request(&path, "attachments.put");
    put["bytes"] = "YWJj".into();
    let attachment = run(put);
    let id = attachment["state"]["id"].as_str().unwrap();
    let mut read = request(&path, "attachments.read");
    read["attachmentID"] = id.into();
    assert_eq!(run(read)["state"]["bytes"], "YWJj");
    assert_eq!(
        run(request(&path, "attachments.list"))["state"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let state = run(request(&path, "get"));
    assert_eq!(state["state"]["state"]["value"]["hits"], 1);
    assert_eq!(state["state"]["state"]["theme"]["accent"], "#123456");
    assert!(
        registry::Lease::acquire(&path).is_ok(),
        "closed commands release ownership"
    );
}

#[test]
fn live_commands_use_the_owner_and_discovery_can_withdraw_and_republish() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    assert!(registry::discovery(&path).unwrap().is_some());
    assert_eq!(run(batch(&path))["sequence"], 1);
    assert_eq!(
        run(request(&path, "get"))["state"]["state"]["value"]["hits"],
        1
    );
    server.withdraw();
    assert!(registry::discovery(&path).unwrap().is_none());
    server.publish().unwrap();
    assert!(registry::discovery(&path).unwrap().is_some());
    close(&owner);
    server.stop();
    assert!(!server.path().exists());
    assert!(registry::discovery(&path).unwrap().is_none());
}

#[test]
fn stale_epochs_malformed_and_expired_commands_never_mutate() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    let mut stale = batch(&path);
    stale["epoch"] = "previous-owner".into();
    assert_eq!(
        checked(socket::call(server.path(), &stale.to_string()).unwrap())["code"],
        "owner_replaced"
    );
    let malformed = checked(socket::call(server.path(), r#"{"method":"surprise"}"#).unwrap());
    assert_eq!(malformed["code"], "rejected");
    stale["epoch"] = owner.epoch().into();
    assert_eq!(
        checked(command::dispatch(
            &owner,
            &stale.to_string(),
            None,
            Instant::now() - Duration::from_millis(1)
        ))["code"],
        "closing"
    );
    assert_eq!(
        run(request(&path, "get"))["state"]["state"]["value"]["hits"],
        0
    );
    close(&owner);
    server.stop();
}

#[test]
fn partial_frames_oversized_requests_and_client_limit_are_bounded() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    let hello = request(&path, "hello").to_string();
    let mut stream = UnixStream::connect(server.path()).unwrap();
    for byte in hello.bytes().chain([b'\n']) {
        stream.write_all(&[byte]).unwrap();
    }
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).unwrap();
    assert_eq!(checked(reply)["method"], "hello");
    let mut huge = request(&path, "theme.import");
    huge["epoch"] = owner.epoch().into();
    huge["file"] = " ".repeat(1024 * 1024 + 1).into();
    assert_eq!(
        checked(socket::call(server.path(), &huge.to_string()).unwrap())["code"],
        "rejected"
    );
    let mut partial = Vec::new();
    for _ in 0..16 {
        let mut client = UnixStream::connect(server.path()).unwrap();
        client.write_all(b"{").unwrap();
        partial.push(client);
    }
    std::thread::sleep(Duration::from_millis(100));
    let mut excess = UnixStream::connect(server.path()).unwrap();
    excess
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let _ = excess.write_all(hello.as_bytes());
    let _ = excess.write_all(b"\n");
    let result = excess.read(&mut [0u8; 1]);
    assert!(
        matches!(result, Ok(0)) || result.is_err(),
        "client limit admitted a reply"
    );
    drop(partial);
    close(&owner);
    server.stop();
}

fn mock(owner: &Owner, dir: &Path) -> UnixListener {
    let path = dir.join("mock.sock");
    let listener = UnixListener::bind(&path).unwrap();
    owner
        .publish_discovery(&json!({"socket":path,"documentPath":owner.path()}).to_string())
        .unwrap();
    listener
}
fn line(stream: &mut UnixStream) -> String {
    let mut text = String::new();
    BufReader::new(stream).read_line(&mut text).unwrap();
    text
}
#[test]
fn unknown_or_missing_hello_identity_prevents_a_mutation() {
    for hello in [
        json!({"ok":true,"method":"hello","epoch":"owner","coreBuildId":"different"}),
        json!({"ok":true,"method":"hello","epoch":"owner"}),
    ] {
        let (dir, path) = document();
        let owner = open(&path);
        let listener = mock(&owner, dir.path());
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            line(&mut stream);
            writeln!(stream, "{hello}").unwrap();
        });
        let refused = run(batch(&path));
        assert_eq!(refused["ok"], false);
        assert_eq!(refused["code"], "rejected");
        peer.join().unwrap();
        let Reply::State { json } = call(&owner, Request::State).unwrap() else {
            panic!()
        };
        assert_eq!(
            serde_json::from_str::<Value>(&json).unwrap()["value"]["hits"],
            0
        );
        close(&owner);
    }
}
#[test]
fn peer_outcomes_are_forwarded_unchanged_without_disk_changes() {
    for code in [
        "rejected",
        "owner_replaced",
        "closing",
        "save_failed",
        "unknown_outcome",
    ] {
        let (dir, path) = document();
        let owner = open(&path);
        let saved = std::fs::read(&path).unwrap();
        let listener = mock(&owner, dir.path());
        listener.set_nonblocking(true).unwrap();
        let refusal = json!({"ok":false,"error":"Peer refusal","code":code,"epoch":"peer"});
        let expected = refusal.clone();
        let (stop, stopped) = mpsc::channel();
        let peer = std::thread::spawn(move || {
            let mut commands = 0;
            loop {
                if stopped.try_recv().is_ok() {
                    break;
                }
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                let input: Value = serde_json::from_str(&line(&mut stream)).unwrap();
                let reply = if input["method"] == "hello" {
                    json!({"ok":true,"method":"hello","epoch":"peer","coreBuildId":hitslop_core::BUILD_ID})
                } else {
                    commands += 1;
                    refusal.clone()
                };
                writeln!(stream, "{reply}").unwrap();
            }
            commands
        });
        assert_eq!(run(batch(&path)), expected);
        stop.send(()).unwrap();
        let commands = peer.join().unwrap();
        if code == "closing" {
            assert!(commands >= 1);
        } else {
            assert_eq!(commands, 1);
        }
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        close(&owner);
    }
}

#[test]
fn a_disconnected_hello_is_rejected_before_mutation() {
    let (dir, path) = document();
    let owner = open(&path);
    let listener = mock(&owner, dir.path());
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&line(&mut stream)).unwrap()["method"],
            "hello"
        );
    });
    assert_eq!(run(batch(&path))["code"], "rejected");
    peer.join().unwrap();
    let Reply::State { json } = call(&owner, Request::State).unwrap() else {
        panic!()
    };
    assert_eq!(
        serde_json::from_str::<Value>(&json).unwrap()["value"]["hits"],
        0
    );
    close(&owner);
}
#[test]
fn a_lost_mutation_reply_is_unknown_and_is_never_replayed() {
    let (dir, path) = document();
    let owner = open(&path);
    let listener = mock(&owner, dir.path());
    let actor = owner.clone();
    let peer = std::thread::spawn(move || {
        let (mut hello, _) = listener.accept().unwrap();
        let input = line(&mut hello);
        writeln!(
            hello,
            "{}",
            command::dispatch(
                &actor,
                &input,
                None,
                Instant::now() + Duration::from_secs(5)
            )
        )
        .unwrap();
        drop(hello);
        let (mut edit, _) = listener.accept().unwrap();
        let input = line(&mut edit);
        let applied = checked(command::dispatch(
            &actor,
            &input,
            None,
            Instant::now() + Duration::from_secs(5),
        ));
        assert_eq!(applied["ok"], true);
        drop(edit); // The committed reply never reaches the helper.
    });
    let answer = run(batch(&path));
    assert_eq!(answer["code"], "unknown_outcome");
    assert!(!answer["error"].as_str().unwrap().contains("was accepted"));
    peer.join().unwrap();
    let Reply::State { json } = call(&owner, Request::State).unwrap() else {
        panic!()
    };
    assert_eq!(
        serde_json::from_str::<Value>(&json).unwrap()["value"]["hits"],
        1
    );
    close(&owner);
    let saved = Store::open(&path, Mode::Snapshot)
        .unwrap()
        .document()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&saved.value().unwrap()).unwrap()["hits"],
        1
    );
}
struct Delayed(Mutex<mpsc::Sender<Arc<ExportCompletion>>>);
impl ExportHandler for Delayed {
    fn export(&self, _: ExportRequest, completion: Arc<ExportCompletion>) {
        self.0.lock().unwrap().send(completion).unwrap();
    }
}
#[test]
fn stopping_server_and_closing_owner_preserves_an_admitted_export_reply() {
    let (_dir, path) = document();
    let owner = open(&path);
    let (tx, rx) = mpsc::channel();
    let server = Server::start(owner.clone(), Arc::new(Delayed(Mutex::new(tx)))).unwrap();
    let mut export = request(&path, "export");
    export["epoch"] = owner.epoch().into();
    export["format"] = "png".into();
    export["output"] = "/tmp/result.png".into();
    let socket = server.path().to_owned();
    let client =
        std::thread::spawn(move || checked(socket::call(&socket, &export.to_string()).unwrap()));
    let completion = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(completion.is_active());
    close(&owner);
    server.stop();
    assert!(registry::Lease::acquire(&path).is_ok());
    assert!(completion.is_active());
    completion.complete(Ok("/tmp/result.png".into()));
    assert_eq!(client.join().unwrap()["output"], "/tmp/result.png");
    assert!(!completion.is_active());
}
#[test]
fn an_expired_export_callback_cannot_publish_late() {
    let (_dir, path) = document();
    let owner = open(&path);
    let (tx, rx) = mpsc::channel();
    let exporter: Arc<dyn ExportHandler> = Arc::new(Delayed(Mutex::new(tx)));
    let mut export = request(&path, "export");
    export["epoch"] = owner.epoch().into();
    export["format"] = "png".into();
    export["output"] = "/tmp/result.png".into();
    let actor = owner.clone();
    let worker = std::thread::spawn(move || {
        command::dispatch(
            &actor,
            &export.to_string(),
            Some(&exporter),
            Instant::now() + Duration::from_millis(100),
        )
    });
    let completion = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let answer = checked(worker.join().unwrap());
    assert_eq!(answer["code"], "unknown_outcome");
    assert!(!completion.is_active());
    completion.complete(Ok("/tmp/late.png".into()));
    close(&owner);
}
