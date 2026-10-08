//! `slop-room`: the opt-in, loopback-only authoritative room proof, a development binary
//! built only with the `dev-sync` feature and never shipped. It runs a room (one
//! authority owner) or a replica owner behind a preview page (`--dev-room PATH`,
//! `--dev-replica-owner PATH`, after the engine's `--client-protocol N`). Bytes and
//! document admission stay in Rust; the wire is private to this binary.
mod framing;
use hitslop_core::{
    Batch, Origin, TextEdit,
    owner::session::{Accepted, Identity, Update},
    owner::{self, Failure, FailureKind, Owner, Reply, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    io::{BufRead, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(15);
/// Edits a replica may have in flight, as the owner's own limit.
const PENDING: usize = 128;
/// A lock whose holder panicked still guards consistent data here: every critical
/// section only inserts, removes or reads one entry.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
/// A named thread; a failure to start one fails the connection or role it serves.
fn spawn(name: &str, work: impl FnOnce() + Send + 'static) -> Result<(), String> {
    std::thread::Builder::new().name(name.into()).spawn(work).map(|_| ()).map_err(|e| e.to_string())
}
fn failure(message: impl Into<String>) -> Failure {
    Failure { kind: FailureKind::Failed, message: message.into(), reason: None, op_index: None }
}
fn wait<T: Send + 'static>(start: impl FnOnce(Box<dyn FnOnce(Result<T, Failure>) + Send>)) -> Result<T, String> {
    wait_result(start).map_err(|e| e.message)
}
fn wait_result<T: Send + 'static>(
    start: impl FnOnce(Box<dyn FnOnce(Result<T, Failure>) + Send>),
) -> Result<T, Failure> {
    let (tx, rx) = mpsc::sync_channel(1);
    start(Box::new(move |value| {
        let _ = tx.try_send(value);
    }));
    rx.recv_timeout(TIMEOUT).map_err(|_| failure("Development sync owner timed out; outcome may be unknown"))?
}
fn token() -> Result<String, String> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| e.to_string())?;
    Ok(data_encoding::HEXLOWER.encode(&bytes))
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    address: SocketAddr,
    credential: String,
    session: String,
    identity: Identity,
}
#[derive(Deserialize)]
struct ReplicaConfig {
    #[serde(flatten)]
    connection: Connection,
    peer: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoomConfig {
    backups: Vec<PathBuf>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Message {
    Hello { credential: String, session: String, identity: Identity, peer: String, version: Vec<u8> },
    Snapshot { identity: Identity, version: Vec<u8> },
    SnapshotRequest,
    Disconnect,
    Disconnected,
    Mutation { id: u64, version: Vec<u8> },
    Accepted { id: u64, reply_length: usize },
    Refused { id: u64 },
    Update { before: Vec<u8>, after: Vec<u8> },
    Rejected { error: String },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Mutation {
    Apply { batch: Batch, origin: String },
    Command { name: String, args_json: String, origin: String },
}
fn origin(value: &str) -> Result<Origin, String> {
    match value {
        "page" => Ok(Origin::Page),
        "agent" => Ok(Origin::Agent),
        "window" => Ok(Origin::Window),
        _ => Err("Unknown mutation origin".into()),
    }
}
fn origin_name(value: Origin) -> String {
    match value {
        Origin::Page => "page",
        Origin::Agent => "agent",
        Origin::Window => "window",
    }
    .into()
}
impl Mutation {
    fn from_request(request: Request) -> Result<Self, String> {
        match request {
            Request::Apply { batch, origin } => Ok(Self::Apply { batch, origin: origin_name(origin) }),
            Request::Command { name, args_json, origin } => {
                Ok(Self::Command { name, args_json, origin: origin_name(origin) })
            }
            _ => Err("Unsupported shared mutation".into()),
        }
    }
    fn request(self) -> Result<Request, String> {
        Ok(match self {
            Self::Apply { batch, origin: value } => Request::Apply { batch, origin: origin(&value)? },
            Self::Command { name, args_json, origin: value } => {
                Request::Command { name, args_json, origin: origin(&value)? }
            }
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Response {
    Applied { sequence: u64, ids: Vec<String>, text: Option<(String, [usize; 2])> },
    Command { sequence: u64, ids: Vec<String>, result_json: String },
}
impl Response {
    fn from_reply(reply: Reply) -> Result<Self, String> {
        match reply {
            Reply::Applied { sequence, ids, text } => {
                Ok(Self::Applied { sequence, ids, text: text.map(|t| (t.authored, t.selection)) })
            }
            Reply::Command { sequence, ids, result_json } => Ok(Self::Command { sequence, ids, result_json }),
            _ => Err("Unexpected shared mutation response".into()),
        }
    }
    fn reply(self) -> Reply {
        match self {
            Self::Applied { sequence, ids, text } => Reply::Applied {
                sequence,
                ids,
                text: text.map(|(authored, selection)| TextEdit { authored, selection }),
            },
            Self::Command { sequence, ids, result_json } => Reply::Command { sequence, ids, result_json },
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Refusal {
    message: String,
    reason: Option<String>,
    op_index: Option<u32>,
    rejected: bool,
}
impl Refusal {
    fn from_failure(e: Failure) -> Self {
        Self {
            message: e.message,
            reason: e.reason,
            op_index: e.op_index,
            rejected: matches!(e.kind, FailureKind::Rejected),
        }
    }
    fn failure(self) -> Failure {
        Failure {
            message: self.message,
            reason: self.reason,
            op_index: self.op_index,
            kind: if self.rejected { FailureKind::Rejected } else { FailureKind::Failed },
        }
    }
}
struct Packet {
    message: Message,
    bytes: Vec<u8>,
}
struct Sender {
    tx: mpsc::SyncSender<Arc<Packet>>,
    stream: TcpStream,
    queued_bytes: Arc<std::sync::atomic::AtomicUsize>,
}
// A group commit can release many small updates and replies at once. Bound both
// count and bytes, so normal bursts fit without allowing a queue of giant snapshots.
const OUTPUT_FRAMES: usize = 128;
const OUTPUT_BYTES: usize = 64 * 1024 * 1024;
fn packet_size(packet: &Packet) -> usize {
    packet.bytes.len() + framing::MAX_HEADER
}
struct Timed<'a> {
    stream: &'a mut TcpStream,
    deadline: Instant,
}
impl Timed<'_> {
    fn remaining(&self) -> std::io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|time| !time.is_zero())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::TimedOut, "Development sync frame timed out"))
    }
}
impl Read for Timed<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.stream.read(bytes)
    }
}
impl Write for Timed<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}
impl Sender {
    fn send(&self, message: Message, bytes: Vec<u8>) -> Result<(), String> {
        self.packet(Arc::new(Packet { message, bytes }))
    }
    fn packet(&self, packet: Arc<Packet>) -> Result<(), String> {
        use std::sync::atomic::Ordering;
        let size = packet_size(&packet);
        if self
            .queued_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |bytes| {
                bytes.checked_add(size).filter(|total| *total <= OUTPUT_BYTES)
            })
            .is_err()
        {
            self.disconnect();
            return Err("Development sync output byte budget exhausted".into());
        }
        self.tx.try_send(packet).map_err(|_| {
            self.queued_bytes.fetch_sub(size, Ordering::AcqRel);
            let _ = self.stream.shutdown(Shutdown::Both);
            "Development sync output full or disconnected".into()
        })
    }
    fn disconnect(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}
fn channel(stream: &TcpStream) -> Result<Arc<Sender>, String> {
    // Headers and chunks are separate writes. Disable Nagle on both endpoints so
    // small durable replies do not wait for the peer's delayed TCP acknowledgement.
    stream.set_nodelay(true).map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    writer.set_write_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::sync_channel::<Arc<Packet>>(OUTPUT_FRAMES);
    let queued_bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sender = Arc::new(Sender {
        tx,
        stream: stream.try_clone().map_err(|e| e.to_string())?,
        queued_bytes: queued_bytes.clone(),
    });
    spawn("slop-room.writer", move || {
        for packet in rx {
            let mut timed = Timed { stream: &mut writer, deadline: Instant::now() + Duration::from_secs(5) };
            let result = framing::write(&mut timed, &packet.message, &packet.bytes);
            queued_bytes.fetch_sub(packet_size(&packet), std::sync::atomic::Ordering::AcqRel);
            if result.is_err() {
                break;
            }
        }
        let _ = writer.shutdown(Shutdown::Both);
    })?;
    Ok(sender)
}
fn read(stream: &mut TcpStream) -> Result<(Message, Vec<u8>), String> {
    // Idle sessions do not time out. Once a frame starts, a stalled sender is bounded.
    stream.set_read_timeout(None).map_err(|e| e.to_string())?;
    let mut first = [0];
    stream.read_exact(&mut first).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let timed = Timed { stream, deadline: Instant::now() + Duration::from_secs(5) };
    framing::read(&mut first.as_slice().chain(timed)).map_err(|e| e.to_string())
}
fn startup<T: serde::de::DeserializeOwned>() -> Result<T, String> {
    let mut line = Vec::new();
    loop {
        let mut byte = 0u8;
        // Read only the startup line. Buffered stdin could read ahead into the
        // ordinary preview frames, which the subsequent nonblocking transport owns.
        // SAFETY: byte is writable and the process's stdin remains open.
        let count = unsafe { libc::read(libc::STDIN_FILENO, (&mut byte as *mut u8).cast(), 1) };
        if count < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.to_string());
        }
        if count == 0 || byte == b'\n' {
            break;
        }
        line.push(byte);
        if line.len() > 65536 {
            return Err("Oversized development startup".into());
        }
    }
    serde_json::from_slice(&line).map_err(|e| e.to_string())
}
struct Peer {
    sender: Arc<Sender>,
    paused: bool,
    duplicate: bool,
    skip_next: bool,
    fenced: Arc<Mutex<mpsc::Receiver<()>>>,
}
type Peers = Arc<Mutex<HashMap<String, Peer>>>;
fn authenticate(connection: &Connection, message: Message, bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    let Message::Hello { credential, session, identity, peer, version } = message else {
        return Err("Handshake required".into());
    };
    if !bytes.is_empty()
        || credential != connection.credential
        || session != connection.session
        || identity != connection.identity
        || !matches!(peer.as_str(), "a" | "b")
    {
        return Err("Development room identity or credential mismatch".into());
    }
    Ok((peer, version))
}
fn snapshot(owner: &Owner, sender: &Sender) -> Result<(), String> {
    let (identity, bytes, version) = wait(|done| owner.sync_export(done))?;
    sender.send(Message::Snapshot { identity, version }, bytes)
}
fn broadcast(peers: &Peers, update: Update) {
    let packet = Arc::new(Packet {
        message: Message::Update { before: update.before, after: update.after },
        bytes: update.bytes,
    });
    for peer in lock(peers).values_mut() {
        if !peer.paused {
            if std::mem::take(&mut peer.skip_next) {
                continue;
            }
            let _ = peer.sender.packet(packet.clone());
            if peer.duplicate {
                peer.duplicate = false;
                let _ = peer.sender.packet(packet.clone());
            }
        }
    }
}
fn room_client(mut stream: TcpStream, owner: Arc<Owner>, connection: Connection, peers: Peers) -> Result<(), String> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let mut timed = Timed { stream: &mut stream, deadline: Instant::now() + Duration::from_secs(5) };
    let (message, bytes) = framing::read::<Message>(&mut timed).map_err(|e| e.to_string())?;
    let (peer, version) = authenticate(&connection, message, &bytes)?;
    wait(|done| owner.sync_check_base(version, done))?;
    let sender = channel(&stream)?;
    // Register in the ordered owner callback: a commit cannot fall between the
    // captured snapshot and subscription. No owner callback waits for socket I/O.
    let register_peers = peers.clone();
    let register_peer = peer.clone();
    let register_sender = sender.clone();
    let (fence_send, fence_receive) = mpsc::sync_channel(1);
    wait(|done| {
        owner.sync_export(Box::new(move |result| {
            let result = result.and_then(|(identity, bytes, version)| {
                let mut clients = lock(&register_peers);
                if clients.contains_key(&register_peer) {
                    return Err(failure("Peer is already connected"));
                }
                register_sender.send(Message::Snapshot { identity, version }, bytes).map_err(failure)?;
                clients.insert(
                    register_peer,
                    Peer {
                        sender: register_sender,
                        paused: false,
                        duplicate: false,
                        skip_next: false,
                        fenced: Arc::new(Mutex::new(fence_receive)),
                    },
                );
                Ok(())
            });
            done(result);
        }))
    })?;
    let pending = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let result = (|| {
        loop {
            let (message, bytes) = read(&mut stream)?;
            match message {
                Message::Mutation { id, version } => {
                    if bytes.len() > hitslop_core::command::MAX_REQUEST_BYTES {
                        return Err("Oversized shared mutation".into());
                    }
                    if pending.fetch_add(1, std::sync::atomic::Ordering::AcqRel) >= PENDING {
                        return Err("Too many pending shared mutations".into());
                    }
                    let mutation: Mutation = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    let request = mutation.request()?;
                    let output = sender.clone();
                    let pending = pending.clone();
                    owner.sync_submit(
                        request,
                        version,
                        Box::new(move |result| {
                            pending.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                            match result {
                                Ok(accepted) => {
                                    let response = Response::from_reply(accepted.reply)
                                        .and_then(|reply| serde_json::to_vec(&reply).map_err(|e| e.to_string()));
                                    match response {
                                        Ok(mut payload) => {
                                            let reply_length = payload.len();
                                            payload.extend_from_slice(&accepted.updates);
                                            let _ = output.send(Message::Accepted { id, reply_length }, payload);
                                        }
                                        Err(error) => {
                                            let _ = output.send(Message::Rejected { error }, vec![]);
                                        }
                                    }
                                }
                                Err(error) => {
                                    let payload = serde_json::to_vec(&Refusal::from_failure(error)).unwrap_or_default();
                                    let _ = output.send(Message::Refused { id }, payload);
                                }
                            }
                        }),
                    );
                }
                Message::SnapshotRequest if bytes.is_empty() => snapshot(&owner, &sender)?,
                Message::Disconnected if bytes.is_empty() => {
                    let _ = fence_send.try_send(());
                }
                _ => return Err("Unexpected room frame".into()),
            }
        }
    })();
    sender.disconnect();
    lock(&peers).remove(&peer);
    result
}
struct NoExport;
impl hitslop_core::command::ExportHandler for NoExport {
    fn export(
        &self,
        _: hitslop_core::command::ExportRequest,
        completion: Arc<hitslop_core::command::ExportCompletion>,
    ) {
        completion.complete(Err(failure("Native exports are unavailable in the development room harness")));
    }
}
fn socket(owner: Arc<Owner>) -> Result<hitslop_core::socket::Server, String> {
    hitslop_core::socket::Server::start(owner, Arc::new(NoExport)).map_err(|e| e.to_string())
}

fn room(path: &Path) -> ExitCode {
    let result = (|| -> Result<(), String> {
        let config: RoomConfig = startup()?;
        if config.backups.len() != 2 {
            return Err("The room needs two replica backup paths".into());
        }
        let peers: Peers = Arc::new(Mutex::new(HashMap::new()));
        let owner = Arc::new(
            Owner::open_with_evaluator(
                path,
                hitslop_core::store::Mode::Document,
                Arc::new(|_| {}),
                Some(slop_engine::evaluator()?),
            )
            .map_err(|e| e.to_string())?,
        );
        let broadcast_peers = peers.clone();
        wait(|done| owner.sync_authority(Arc::new(move |update| broadcast(&broadcast_peers, update)), done))?;
        for destination in config.backups {
            wait(|done| owner.submit(Request::Backup { destination }, None, done))?;
        }
        let (identity, _, _) = wait(|done| owner.sync_export(done))?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let connection = Connection {
            address: listener.local_addr().map_err(|e| e.to_string())?,
            credential: token()?,
            session: token()?,
            identity,
        };
        let server = socket(owner.clone())?;
        let accept_owner = owner.clone();
        let accept_connection = connection.clone();
        let accept_peers = peers.clone();
        spawn("slop-room.accept", move || {
            let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            for stream in listener.incoming().flatten() {
                if active
                    .fetch_update(std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire, |count| {
                        (count < 8).then_some(count + 1)
                    })
                    .is_err()
                {
                    continue;
                }
                let owner = accept_owner.clone();
                let connection = accept_connection.clone();
                let peers = accept_peers.clone();
                let running = active.clone();
                let started = spawn("slop-room.client", move || {
                    let _ = room_client(stream, owner, connection, peers);
                    running.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                });
                if started.is_err() {
                    active.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                }
            }
        })?;
        println!("{}", json!({"type":"ready","pid":std::process::id(),"connection":connection}));
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        for line in std::io::stdin().lock().lines() {
            let line = line.map_err(|e| e.to_string())?;
            let control: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            let id = control["id"].as_u64().ok_or("Control ID required")?;
            let peer = control["peer"].as_str().ok_or("Control peer required")?;
            let action = control["action"].as_str().ok_or("Control action required")?;
            let outcome = (|| -> Result<(), String> {
                if action == "disconnect" {
                    let (sender, fenced) = {
                        let clients = lock(&peers);
                        let client = clients.get(peer).ok_or("Peer is not connected")?;
                        (client.sender.clone(), client.fenced.clone())
                    };
                    sender.send(Message::Disconnect, vec![])?;
                    let result = fenced
                        .lock()
                        .unwrap()
                        .recv_timeout(TIMEOUT)
                        .map_err(|_| "Replica did not acknowledge disconnection".to_string());
                    sender.disconnect();
                    return result;
                }
                let sender = {
                    let mut clients = lock(&peers);
                    let client = clients.get_mut(peer).ok_or("Peer is not connected")?;
                    match action {
                        "pauseDelivery" => {
                            client.paused = true;
                            return Ok(());
                        }
                        "duplicateNext" => {
                            client.duplicate = true;
                            return Ok(());
                        }
                        "skipNext" => {
                            client.skip_next = true;
                            return Ok(());
                        }
                        "resumeDelivery" | "snapshot" => client.sender.clone(),
                        _ => return Err("Unknown room control".into()),
                    }
                };
                let resume = action == "resumeDelivery";
                let clients = peers.clone();
                let name = peer.to_string();
                wait(|done| {
                    owner.sync_export(Box::new(move |result| {
                        done(result.and_then(|(identity, bytes, version)| {
                            let mut clients = lock(&clients);
                            sender.send(Message::Snapshot { identity, version }, bytes).map_err(failure)?;
                            if resume && let Some(client) = clients.get_mut(&name) {
                                client.paused = false;
                            }
                            Ok(())
                        }));
                    }))
                })?;
                Ok(())
            })();
            println!("{}", json!({"type":"control","id":id,"ok":outcome.is_ok(),"error":outcome.err()}));
            std::io::stdout().flush().map_err(|e| e.to_string())?;
        }
        server.stop();
        for peer in lock(&peers).values() {
            peer.sender.disconnect();
        }
        wait(|done| owner.submit(Request::Close { preview: None, icon: None }, None, done))?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Development room: {error}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

type Pending = Arc<Mutex<HashMap<u64, owner::session::SyncCompletion>>>;
fn finish_pending(pending: &Pending) {
    for (_, done) in lock(pending).drain() {
        done(Err(failure(
            "Room disconnected; mutation outcome is unknown. Inspect synchronized state before retrying.",
        )));
    }
}
fn replica(path: &Path) -> ExitCode {
    let config: ReplicaConfig = match startup() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Development replica: {error}");
            return ExitCode::FAILURE;
        }
    };
    if !config.connection.address.ip().is_loopback() {
        eprintln!("Development sync requires loopback");
        return ExitCode::FAILURE;
    }
    let server = Arc::new(Mutex::new(None));
    let retained = server.clone();
    let result = slop_engine::preview::serve_configured(path, move |owner| {
        let mut stream = TcpStream::connect_timeout(&config.connection.address, Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        let (_, _, version) = wait(|done| owner.sync_export(done))?;
        framing::write(
            &mut stream,
            &Message::Hello {
                credential: config.connection.credential,
                session: config.connection.session,
                identity: config.connection.identity.clone(),
                peer: config.peer,
                version,
            },
            &[],
        )
        .map_err(|e| e.to_string())?;
        stream.set_read_timeout(Some(TIMEOUT)).map_err(|e| e.to_string())?;
        let (message, bytes) = framing::read::<Message>(&mut stream).map_err(|e| e.to_string())?;
        let Message::Snapshot { identity, version: initial_version } = message else {
            return Err("Room did not provide its snapshot".into());
        };
        if identity != config.connection.identity {
            return Err("Room snapshot identity mismatch".into());
        }
        let sender = channel(&stream)?;
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let ids = std::sync::atomic::AtomicU64::new(1);
        let requests = pending.clone();
        let resyncing = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let request_resyncing = resyncing.clone();
        let output = sender.clone();
        wait(|done| {
            owner.sync_replica(
                identity.clone(),
                Arc::new(move |request, version, done| {
                    let result = Mutation::from_request(request)
                        .and_then(|request| serde_json::to_vec(&request).map_err(|e| e.to_string()));
                    let payload = match result {
                        Ok(payload) => payload,
                        Err(error) => {
                            done(Err(failure(error)));
                            return;
                        }
                    };
                    let id = ids.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut callbacks = lock(&requests);
                    if request_resyncing.load(std::sync::atomic::Ordering::Acquire) {
                        done(Err(Failure {
                            kind: FailureKind::Rejected,
                            message: "Shared snapshot is being installed; keep your draft and retry".into(),
                            reason: Some("stale_base".into()),
                            op_index: None,
                        }));
                        return;
                    }

                    if callbacks.len() >= PENDING {
                        done(Err(Failure {
                            kind: FailureKind::Rejected,
                            message: "Too many pending shared mutations; request was not sent".into(),
                            reason: Some("invalid_request".into()),
                            op_index: None,
                        }));
                        return;
                    }
                    callbacks.insert(id, done);
                    if let Err(error) = output.send(Message::Mutation { id, version }, payload)
                        && let Some(done) = callbacks.remove(&id)
                    {
                        done(Err(failure(error)));
                    }
                }),
                done,
            )
        })?;
        wait(|done| owner.sync_snapshot(identity.clone(), bytes, done))?;
        *lock(&retained) = Some(socket(owner.clone())?);
        let reader_owner = owner.clone();
        spawn("slop-room.reader", move || {
            let mut version = initial_version;
            let mut awaiting = false;
            let mut deferred_snapshot = false;
            let mut snapshot_retries = 0;
            let result = (|| -> Result<(), String> {
                loop {
                    let (message, bytes) = read(&mut stream)?;
                    match message {
                        Message::Update { before, after } => {
                            if after == version {
                                continue;
                            }
                            if awaiting {
                                continue;
                            }
                            if before != version {
                                awaiting = true;
                                resyncing.store(true, std::sync::atomic::Ordering::Release);
                                sender.send(Message::SnapshotRequest, vec![])?;
                                continue;
                            }
                            wait(|done| reader_owner.sync_install(identity.clone(), bytes, done))?;
                            version = after;
                        }
                        Message::Snapshot { identity: received, version: next } => {
                            if received != identity {
                                return Err("Snapshot identity mismatch".into());
                            }
                            resyncing.store(true, std::sync::atomic::Ordering::Release);
                            if !lock(&pending).is_empty() {
                                deferred_snapshot = true;
                                continue;
                            }
                            wait(|done| reader_owner.submit(Request::Flush, None, done))?;
                            if let Err(error) =
                                wait_result(|done| reader_owner.sync_snapshot(identity.clone(), bytes, done))
                            {
                                if snapshot_retries < 3
                                    && (matches!(error.kind, FailureKind::Busy)
                                        || error.reason.as_deref() == Some("stale_base"))
                                {
                                    snapshot_retries += 1;
                                    awaiting = true;
                                    sender.send(Message::SnapshotRequest, vec![])?;
                                    continue;
                                }
                                return Err(error.message);
                            }
                            snapshot_retries = 0;
                            version = next;
                            awaiting = false;
                            resyncing.store(false, std::sync::atomic::Ordering::Release);
                        }
                        Message::Accepted { id, reply_length } => {
                            if reply_length > bytes.len() {
                                return Err("Malformed accepted response".into());
                            }
                            let response: Response =
                                serde_json::from_slice(&bytes[..reply_length]).map_err(|e| e.to_string())?;
                            let done = lock(&pending).remove(&id).ok_or("Unknown accepted request")?;
                            done(Ok(Accepted { reply: response.reply(), updates: bytes[reply_length..].to_vec() }));
                        }
                        Message::Refused { id } => {
                            let refusal: Refusal = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                            let done = lock(&pending).remove(&id).ok_or("Unknown refused request")?;
                            done(Err(refusal.failure()));
                        }
                        Message::Disconnect if bytes.is_empty() => {
                            resyncing.store(true, std::sync::atomic::Ordering::Release);
                            wait(|done| reader_owner.sync_disconnect(done))?;
                            sender.send(Message::Disconnected, vec![])?;
                        }
                        Message::Rejected { error } => return Err(error),
                        _ => return Err("Unexpected replica frame".into()),
                    }
                    if deferred_snapshot && lock(&pending).is_empty() {
                        wait(|done| reader_owner.submit(Request::Flush, None, done))?;
                        sender.send(Message::SnapshotRequest, vec![])?;
                        deferred_snapshot = false;
                        awaiting = true;
                    }
                }
            })();
            sender.disconnect();
            reader_owner.sync_disconnect(Box::new(|_| {}));
            finish_pending(&pending);
            if let Err(error) = result {
                eprintln!("Development replica disconnected: {error}");
            }
        })?;
        Ok(())
    });
    if let Some(server) = lock(&server).take() {
        server.stop();
    }
    result
}

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    // The room's owner runs commands in this executable's restricted child.
    if args.len() == 1 && args[0] == "--evaluate-command" {
        hitslop_runner::child();
        return ExitCode::SUCCESS;
    }
    if let Some(folder) = std::env::var_os("HITSLOP_TEST_REGISTRY").filter(|folder| !folder.is_empty()) {
        let _ = hitslop_core::registry::use_folder(Path::new(&folder));
    }
    let protocol = hitslop_core::command::protocol();
    let version = serde_json::from_str::<serde_json::Value>(&protocol).ok().and_then(|v| v["version"].as_u64());
    match args.as_slice() {
        [flag, n, role, path] if flag == "--client-protocol" && n.to_str().and_then(|n| n.parse().ok()) == version => {
            match role.to_str() {
                Some("--dev-room") => room(Path::new(path)),
                Some("--dev-replica-owner") => replica(Path::new(path)),
                _ => usage(),
            }
        }
        _ => usage(),
    }
}
fn usage() -> ExitCode {
    eprintln!("Use --client-protocol N --dev-room PATH, or --dev-replica-owner PATH");
    ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connection() -> Connection {
        Connection {
            address: "127.0.0.1:1234".parse().unwrap(),
            credential: "secret".into(),
            session: "session".into(),
            identity: Identity { document_uuid: "document".into(), app_digest: "digest".into(), layout: 1 },
        }
    }
    fn hello(c: &Connection) -> Message {
        Message::Hello {
            credential: c.credential.clone(),
            session: c.session.clone(),
            identity: c.identity.clone(),
            peer: "a".into(),
            version: vec![],
        }
    }
    #[test]
    fn handshake_binds_credential_session_document_app_and_layout() {
        let c = connection();
        let mut config = serde_json::to_value(&c).unwrap();
        config["peer"] = json!("a");
        let parsed: ReplicaConfig = serde_json::from_value(config).unwrap();
        assert_eq!(parsed.peer, "a");
        assert!(authenticate(&c, hello(&c), &[]).is_ok());
        for field in 0..5 {
            let mut wrong = c.clone();
            match field {
                0 => wrong.credential.push('x'),
                1 => wrong.session.push('x'),
                2 => wrong.identity.document_uuid.push('x'),
                3 => wrong.identity.app_digest.push('x'),
                _ => wrong.identity.layout += 1,
            }
            assert!(authenticate(&c, hello(&wrong), &[]).is_err());
        }
        assert!(authenticate(&c, hello(&c), &[0]).is_err());
        assert!(authenticate(&c, Message::SnapshotRequest, &[]).is_err());
    }
    #[test]
    fn bounded_writer_refuses_slow_reader_without_waiting_in_callback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_slow_reader, _) = listener.accept().unwrap();
        let sender = channel(&client).unwrap();
        let packet = Arc::new(Packet { message: Message::SnapshotRequest, bytes: vec![7; framing::MAX_PAYLOAD] });
        let start = Instant::now();
        let mut refused = false;
        for _ in 0..32 {
            if sender.packet(packet.clone()).is_err() {
                refused = true;
                break;
            }
        }
        assert!(refused);
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn partial_frame_deadline_is_total_not_per_chunk() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let _idle = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let mut timed = Timed { stream: &mut stream, deadline: Instant::now() + Duration::from_millis(30) };
        let start = Instant::now();
        assert!(framing::read::<Message>(&mut timed).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
