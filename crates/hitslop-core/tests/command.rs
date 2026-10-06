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
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
mod support;
const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"text"},"hits":{"kind":"counter"}}}"#;
fn document() -> (tempfile::TempDir, PathBuf) {
    support::isolate_registry();
    let dir = tempfile::tempdir_in("/tmp").unwrap();
    let stage = dir.path().join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/app.js"), "export default {}").unwrap();
    support::write_app(&stage, support::App::new(SCHEMA, r#"{"title":"Initial","hits":0}"#));
    let template = dir.path().join("template.slop");
    file::pack(&stage, &template).unwrap();
    let path = dir.path().join("doc.slop");
    file::create_document(&template, &path).unwrap();
    (dir, path)
}
/// The command protocol these requests are written in.
const PROTOCOL: u64 = 1;
fn request(path: &Path, method: &str) -> Value {
    json!({"protocol":PROTOCOL,"method":method,"documentPath":std::fs::canonicalize(path).unwrap()})
}
fn batch(path: &Path) -> Value {
    let mut r = request(path, "batch");
    r["ops"] = r#"[{"type":"increment","path":["hits"],"by":1}]"#.into();
    r
}
fn checked(text: String) -> Value {
    assert!(envelope::is_valid(Envelope::SocketReply, text.as_bytes()), "invalid reply: {text}");
    serde_json::from_str(&text).unwrap()
}
fn run(value: Value) -> Value {
    checked(command::request(&value.to_string(), PROTOCOL, None))
}
fn call(owner: &Owner, request: Request) -> std::result::Result<Reply, Failure> {
    let (tx, rx) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |r| {
            tx.send(r).unwrap();
        }),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}
fn close(owner: &Owner) {
    call(owner, Request::Close { preview: None, icon: None }).unwrap();
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
fn protocol_preflight_refuses_a_newer_large_payload_without_writing() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    let discovery: Value = serde_json::from_str(&registry::discovery(&path).unwrap().unwrap()).unwrap();
    let socket = Path::new(discovery["socket"].as_str().unwrap());
    let before = std::fs::read(&path).unwrap();
    // A future protocol can carry a payload larger than this reader permits. Its
    // preflight must reach the permanent refusal before any payload is transmitted.
    let request = json!({"protocol": PROTOCOL + 1, "method": "future.batch", "payload": "x".repeat(command::MAX_REQUEST_BYTES + 1)});
    let refusal = checked(socket::call(socket, &request.to_string()).unwrap());
    assert_eq!(refusal["code"], "rejected");
    assert_eq!(refusal["reason"], "requires_update");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(server);
    close(&owner);
}

#[test]
fn closed_commands_edit_theme_data_and_attachments_then_reopen() {
    let (_dir, path) = document();
    // An agent's edit reports the rows it inserted; it reads state with `get`.
    assert_eq!(run(batch(&path)), json!({"ok":true,"method":"batch","ids":[]}));
    let mut theme = request(&path, "batch");
    theme["ops"] = r##"[{"type":"setTheme","values":{"accent":"#123456"}}]"##.into();
    assert_eq!(run(theme)["method"], "batch");
    // A blob arrives with the batch that references it, here inside text.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let stored = || run(request(&path, "attachments.list"))["state"].as_array().unwrap().len();
    let mut attach = request(&path, "batch");
    attach["ops"] = format!(r#"[{{"type":"set","path":["title"],"value":"Cover: {ABC}"}}]"#).into();
    attach["attachments"] = json!(["YWJj"]);
    assert_eq!(run(attach)["method"], "batch");
    let mut read = request(&path, "attachments.read");
    read["attachmentID"] = ABC.into();
    assert_eq!(run(read)["state"]["bytes"], "YWJj");
    // A refused batch's blob is stored first, then reclaimed as its command closes.
    let mut refused = request(&path, "batch");
    refused["ops"] = r#"[{"type":"set","path":["missing"],"value":1}]"#.into();
    refused["attachments"] = json!(["ZGVm"]);
    assert_eq!(run(refused)["ok"], false);
    assert_eq!(stored(), 1);
    // Once nothing references it, a blob is reclaimed at the next close.
    let mut removed = request(&path, "batch");
    removed["ops"] = r#"[{"type":"set","path":["title"],"value":"No cover"}]"#.into();
    run(removed);
    assert_eq!(stored(), 0);
    // A command that only reads writes nothing, even beside a blob nothing references.
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute("INSERT INTO attachments VALUES(?, x'00')", [&"f".repeat(64)])
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    assert_eq!(run(request(&path, "get"))["method"], "get");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // `get` is what an agent reads: no publication sequence, which only orders the page's
    // stream, and the declared colors apart from the effective ones.
    let state = &run(request(&path, "get"))["state"];
    let mut keys: Vec<_> = state.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["defaults", "schema", "theme", "value", "version"]);
    assert_eq!(state["schema"]["properties"]["hits"]["kind"], "counter");
    assert_eq!(state["value"]["hits"], 1);
    assert_eq!(state["theme"]["accent"], "#123456");
    assert_eq!(state["defaults"]["accent"], "#335577", "the app's declared palette");
    assert!(registry::Lease::acquire(&path).is_ok(), "closed commands release ownership");
}

#[test]
fn live_commands_use_the_owner_and_discovery_can_withdraw_and_republish() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    assert!(registry::discovery(&path).unwrap().is_some());
    assert_eq!(run(batch(&path))["ok"], true);
    assert_eq!(run(request(&path, "get"))["state"]["value"]["hits"], 1);
    server.withdraw();
    assert!(registry::discovery(&path).unwrap().is_none());
    server.publish().unwrap();
    assert!(registry::discovery(&path).unwrap().is_some());
    // A discovery record a later build extends still reaches the owner: a client reads only
    // `socket` and `documentPath`, so a client of any age hears the owner's answer.
    let record = std::fs::read_dir(support::registry_folder())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|file| {
            file.extension().is_some_and(|e| e == "json")
                && std::fs::read_to_string(file).is_ok_and(|text| text.contains(&*path.to_string_lossy()))
        })
        .expect("the published discovery record");
    let mut extended: Value = serde_json::from_str(&std::fs::read_to_string(&record).unwrap()).unwrap();
    extended["addedByALaterBuild"] = json!({"anything": true});
    std::fs::write(&record, extended.to_string()).unwrap();
    assert_eq!(run(request(&path, "get"))["state"]["value"]["hits"], 1);
    close(&owner);
    server.stop();
    assert!(!server.path().exists());
    assert!(registry::discovery(&path).unwrap().is_none());
}

// Failure: an engine of another build sent a request the owner could not read, and the
// refusal did not say which side to update. Oracle: literal codes and an unchanged value.
#[test]
fn malformed_unsupported_and_expired_commands_never_mutate() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    let malformed = checked(socket::call(server.path(), r#"{"protocol":1,"method":"surprise"}"#).unwrap());
    assert_eq!(malformed["code"], "rejected");
    // The permanent refusal path: another protocol, even with a method this build has never
    // heard of, gets exactly this flat reply, naming the side to update.
    for (protocol, update) in [(2, "update hitSlop"), (9_999, "update hitSlop"), (0, "update the hitSlop CLI")] {
        for mut other in [batch(&path), json!({"documentPath": path, "method": "a-later-method"})] {
            other["protocol"] = protocol.into();
            other["surprise"] = "a field of another protocol".into();
            let refused = checked(socket::call(server.path(), &other.to_string()).unwrap());
            let mut keys: Vec<_> = refused.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(keys, ["code", "error", "ok", "reason"], "{refused}");
            assert_eq!(
                (refused["ok"].as_bool(), refused["code"].as_str(), refused["reason"].as_str()),
                (Some(false), Some("rejected"), Some("requires_update"))
            );
            assert!(refused["error"].as_str().unwrap().contains(update), "{refused}");
        }
    }
    assert_eq!(
        checked(command::serve(&owner, &batch(&path).to_string(), None, Instant::now() - Duration::from_millis(1)))["code"],
        "closing"
    );
    assert_eq!(run(request(&path, "get"))["state"]["value"]["hits"], 0);
    close(&owner);
    server.stop();
}

#[test]
fn partial_frames_oversized_requests_and_client_limit_are_bounded() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    let get = request(&path, "get").to_string();
    let mut stream = UnixStream::connect(server.path()).unwrap();
    for byte in get.bytes().chain([b'\n']) {
        stream.write_all(&[byte]).unwrap();
    }
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).unwrap();
    assert_eq!(checked(reply)["method"], "get");
    let mut huge = batch(&path);
    huge["ops"] = format!("[{}]", " ".repeat(1024 * 1024 + 1)).into();
    assert_eq!(checked(socket::call(server.path(), &huge.to_string()).unwrap())["code"], "rejected");
    let mut partial = Vec::new();
    for _ in 0..16 {
        let mut client = UnixStream::connect(server.path()).unwrap();
        client.write_all(b"{").unwrap();
        partial.push(client);
    }
    std::thread::sleep(Duration::from_millis(100));
    // Use the bounded client: on macOS, setting SO_RCVTIMEO after the server has
    // already rejected and closed an excess connection can itself fail with EINVAL.
    assert!(socket::call(server.path(), &get).is_err(), "client limit admitted a reply");
    drop(partial);
    close(&owner);
    server.stop();
}

fn mock(owner: &Owner, dir: &Path) -> UnixListener {
    let path = dir.join("mock.sock");
    let listener = UnixListener::bind(&path).unwrap();
    owner.publish_discovery(&json!({"socket":path,"documentPath":owner.path()}).to_string()).unwrap();
    listener
}
fn line(stream: &mut UnixStream) -> String {
    let mut text = String::new();
    BufReader::new(stream).read_line(&mut text).unwrap();
    text
}
#[test]
fn peer_outcomes_are_forwarded_unchanged_without_disk_changes() {
    for code in ["rejected", "owner_replaced", "closing", "save_failed", "unknown_outcome"] {
        let (dir, path) = document();
        let owner = open(&path);
        let saved = std::fs::read(&path).unwrap();
        let listener = mock(&owner, dir.path());
        listener.set_nonblocking(true).unwrap();
        let refusal = json!({"ok":false,"error":"Peer refusal","code":code});
        let expected = refusal.clone();
        let (stop, stopped) = mpsc::channel();
        let peer = std::thread::spawn(move || {
            let mut commands = 0;
            loop {
                if stopped.try_recv().is_ok() {
                    break;
                }
                let mut stream = match listener.accept() {
                    // macOS hands out accepted streams nonblocking like their listener.
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                let input: Value = serde_json::from_str(&line(&mut stream)).unwrap();
                if input["method"] == "hello" {
                    writeln!(stream, "{}", json!({"ok":true,"method":"hello"})).unwrap();
                    continue;
                }
                commands += 1;
                writeln!(stream, "{refusal}").unwrap();
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
fn a_lost_mutation_reply_is_unknown_and_is_never_replayed() {
    let (dir, path) = document();
    let owner = open(&path);
    let listener = mock(&owner, dir.path());
    let actor = owner.clone();
    let peer = std::thread::spawn(move || {
        let (mut hello, _) = listener.accept().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line(&mut hello)).unwrap()["method"], "hello");
        writeln!(hello, "{}", json!({"ok":true,"method":"hello"})).unwrap();
        drop(hello);
        let (mut edit, _) = listener.accept().unwrap();
        let input = line(&mut edit);
        let applied = checked(command::serve(&actor, &input, None, Instant::now() + Duration::from_secs(5)));
        assert_eq!(applied["ok"], true);
        drop(edit); // The committed reply never reaches the helper.
    });
    let answer = run(batch(&path));
    assert_eq!(answer["code"], "unknown_outcome");
    assert!(!answer["error"].as_str().unwrap().contains("was accepted"));
    peer.join().unwrap();
    let Reply::State { json, .. } = call(&owner, Request::State).unwrap() else { panic!() };
    assert_eq!(serde_json::from_str::<Value>(&json).unwrap()["value"]["hits"], 1);
    close(&owner);
    let saved = Store::open(&path, Mode::Snapshot).unwrap().document().unwrap();
    assert_eq!(serde_json::from_str::<Value>(&saved.value()).unwrap()["hits"], 1);
}
// An accepted batch survives in memory when a discard replaces the owner's state before
// its save confirms; its reply is unknown, never a promise that replaying it is safe.
#[test]
fn a_batch_discarded_before_its_save_confirms_reports_an_unknown_outcome() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(NoExport)).unwrap();
    let lock = rusqlite::Connection::open(&path).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let request = batch(&path);
    let client = std::thread::spawn(move || run(request));
    let hits = |owner: &Owner| {
        let Reply::State { json, .. } = call(owner, Request::State).unwrap() else { panic!() };
        serde_json::from_str::<Value>(&json).unwrap()["value"]["hits"].clone()
    };
    // Accepted, and waiting for its save.
    let start = Instant::now();
    while hits(&owner) != 1 && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let (tx, discarded) = mpsc::channel();
    owner.submit(Request::Discard, None, Box::new(move |r| tx.send(r).unwrap()));
    let reply = client.join().unwrap();
    assert_eq!(reply["code"], "unknown_outcome");
    assert!(reply["error"].as_str().unwrap().contains("could not be confirmed"));
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    discarded.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    // The save in flight committed before the reload read it.
    assert_eq!(hits(&owner), 1);
    close(&owner);
    server.stop();
}
fn page(owner: &Owner, view: &str, input: &str) -> command::PageReply {
    let (tx, rx) = mpsc::channel();
    command::page(owner, view.into(), input, move |reply| tx.send(reply).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}
fn page_json(reply: &command::PageReply) -> Value {
    serde_json::from_str(&reply.json).unwrap()
}
// The page's document requests run through the core: replies the page reads, refusals it
// can act on, and the owner's failure for the host. Oracle: literal replies and the value.
#[test]
fn page_requests_answer_the_page_and_refuse_what_it_may_not_do() {
    let (_dir, path) = document();
    let owner = open(&path);
    owner.attach("page".into());
    let opened = page_json(&page(&owner, "page", r#"{"method":"open"}"#));
    assert_eq!(opened["ok"], true);
    let state: Value = serde_json::from_str(opened["state"].as_str().unwrap()).unwrap();
    assert_eq!(state["value"]["hits"], 0);
    let batch = |intent: Value| json!({"method":"apply","batch":json!({"intents":[intent]}).to_string()}).to_string();
    let applied = page(&owner, "page", &batch(json!({"type":"increment","path":["hits"],"by":2})));
    assert!(applied.failure.is_none());
    assert_eq!(page_json(&applied), json!({"ok":true,"sequence":1,"ids":[]}));
    let palette = page(&owner, "page", &batch(json!({"type":"setTheme","values":{"accent":"#123456"}})));
    assert_eq!(page_json(&palette)["reason"], "invalid_request", "the page cannot change the palette");
    assert!(palette.failure.is_some());
    let put = page_json(&page(&owner, "page", r#"{"method":"attachments.put","bytes":"YWJj"}"#));
    assert_eq!(put["byteLength"], 3);
    let read = json!({"method":"attachments.read","attachmentID":put["id"]}).to_string();
    assert_eq!(page_json(&page(&owner, "page", &read))["bytes"], "YWJj");
    assert_eq!(page_json(&page(&owner, "page", r#"{"method":"flush"}"#)), json!({"ok":true}));
    let stale = page(&owner, "replaced", r#"{"method":"undo"}"#);
    assert_eq!(page_json(&stale)["code"], "owner_replaced");
    assert_eq!(stale.failure.map(|f| f.kind), Some(hitslop_core::owner::FailureKind::Replaced));
    // The core checks the envelope and parses the payload; either way a malformed or
    // oversized request is a definite refusal that applies nothing.
    let increment = json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}).to_string();
    for (refused, reason) in [
        (json!({"method":"ready"}), "invalid_request"),
        (json!({"method":"surprise"}), "invalid_request"),
        (json!({"method":"apply"}), "invalid_request"),
        (json!({"method":"apply","batch":increment,"extra":true}), "invalid_request"),
        (json!({"method":"flush","batch":increment}), "invalid_request"),
        (json!({"view":"","method":"apply","batch":increment}), "invalid_request"),
        (json!({"method":"apply","batch":{"intents":[]}}), "invalid_request"),
        (
            json!({"method":"apply","batch":json!({"intents":[{"type":"increment","path":["hits"],"by":1,"extra":1}]}).to_string()}),
            "invalid_request",
        ),
        // Text edits are batches now; there is no separate text request.
        (json!({"method":"text","request":json!({"base":"x"}).to_string()}), "invalid_request"),
        (json!({"method":"apply","batch":json!({"base":"x","intents":[]}).to_string()}), "invalid_version"),
        // The core bounds opaque document payloads in UTF-8 bytes.
        (json!({"method":"apply","batch":"😀".repeat(1_048_577)}), "too_large"),
    ] {
        let reply = page_json(&page(&owner, "page", &refused.to_string()));
        assert_eq!(
            (reply["code"].as_str(), reply["reason"].as_str()),
            (Some("rejected"), Some(reason)),
            "{refused:.120}"
        );
    }
    assert_eq!(page_json(&page(&owner, "page", "not json"))["code"], "rejected");
    let Reply::State { json, .. } = call(&owner, Request::State).unwrap() else { panic!() };
    assert_eq!(serde_json::from_str::<Value>(&json).unwrap()["value"]["hits"], 2);
    close(&owner);
}
// Failure: an agent's batch replaced text written after its `get`; then a batch's merged
// version, reused as the next base, deleted it. Oracle: the literal merged text, with each
// base read by `get` before the rewrite it serves.
#[test]
fn a_based_batch_keeps_text_written_since_the_agents_read() {
    let (_dir, path) = document();
    let set = |title: &str, base: Option<&Value>| {
        let mut batch = request(&path, "batch");
        batch["ops"] = json!([{"type":"set","path":["title"],"value":title}]).to_string().into();
        if let Some(base) = base {
            batch["base"] = base.clone();
        }
        run(batch)
    };
    let read = || run(request(&path, "get"))["state"].clone();
    let before = read();
    // The person types after the agent's read.
    set("Initial typed", None);
    set("First", Some(&before["version"]));
    assert_eq!(read()["value"]["title"], "First typed");
    // The agent reads again before its next rewrite, and the person keeps typing.
    let again = read();
    set("First typed!", None);
    set("Second typed", Some(&again["version"]));
    assert_eq!(read()["value"]["title"], "Second typed!");
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
    export["format"] = "png".into();
    export["output"] = "/tmp/result.png".into();
    let socket = server.path().to_owned();
    let client = std::thread::spawn(move || checked(socket::call(&socket, &export.to_string()).unwrap()));
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
    export["format"] = "png".into();
    export["output"] = "/tmp/result.png".into();
    let actor = owner.clone();
    let worker = std::thread::spawn(move || {
        command::serve(&actor, &export.to_string(), Some(&exporter), Instant::now() + Duration::from_millis(100))
    });
    let completion = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let answer = checked(worker.join().unwrap());
    assert_eq!(answer["code"], "unknown_outcome");
    assert!(!completion.is_active());
    completion.complete(Ok("/tmp/late.png".into()));
    close(&owner);
}
struct Panics;
impl ExportHandler for Panics {
    fn export(&self, _: ExportRequest, _: Arc<ExportCompletion>) {
        panic!("a renderer fault");
    }
}
// Failure: a client whose request panicked kept its place, so after sixteen such faults the
// socket dropped every connection unanswered. Oracle: the next request is answered.
#[test]
fn a_client_that_panics_frees_its_place() {
    let (_dir, path) = document();
    let owner = open(&path);
    let server = Server::start(owner.clone(), Arc::new(Panics)).unwrap();
    let mut export = request(&path, "export");
    export["format"] = "png".into();
    export["output"] = "/tmp/result.png".into();
    for _ in 0..16 {
        assert!(socket::call(server.path(), &export.to_string()).is_err(), "a panicked request has no reply");
    }
    let answer = socket::call(server.path(), &request(&path, "get").to_string());
    assert_eq!(checked(answer.expect("a place is free"))["method"], "get");
    close(&owner);
    server.stop();
}
