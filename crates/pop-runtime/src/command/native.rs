//! Native socket routing and blocking CLI driver.
// Native failures carry classified text across the owner/FFI boundary, not Rust error sources.
use super::*;
use crate::{lock, socket};
use pop_contracts::wire::SocketRequest;
use pop_contracts::wire::engine::{
    AttachmentBytes, AttachmentInfo, DocumentRequest, EngineSuccess, GetState, ThemeExportState, True,
};
use pop_contracts::wire::socket::protocol_mismatch;
use pop_storage::{file, registry, store};
use serde::Deserialize;
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
pub const MAX_REQUEST_BYTES: usize = wire::SOCKET_ATTACHMENT;
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(35);
const ADMISSION: Duration = Duration::from_secs(2);
/// The permanent preflight has no document path and never admits an operation.
#[cfg(not(target_os = "ios"))]
pub(crate) fn preflight(input: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Method {
        method: Box<RawValue>,
    }
    let method: Method = serde_json::from_str(input).ok()?;
    if serde_json::from_str::<String>(method.method.get()).ok().as_deref() != Some("hello") {
        return None;
    }
    Some(match wire::socket::check_protocol(input) {
        Err(error) => failure(Failure::from(error), false, false),
        Ok(()) if input.len() <= 1024 && serde_json::from_str::<wire::socket::Hello>(input).is_ok() => {
            crate::encode(&wire::socket::HelloSuccess::Hello {
                _ok: True,
                application_id: pop_contracts::format::APPLICATION_ID,
            })
        }
        Ok(()) => failure(invalid("Invalid protocol preflight"), false, false),
    })
}
/// The host drains unsent page edits and fences the page until `resume`.
/// It never renders or publishes an export.
pub trait PageBarrier: Send + Sync {
    fn settle(&self, completion: Arc<PageSettlement>);
    fn resume(&self);
}
/// A bounded page drain acknowledgement. Late drains must undo their page fence.
pub struct PageSettlement {
    deadline: Instant,
    sender: Mutex<Option<mpsc::Sender<Result<()>>>>,
}
impl PageSettlement {
    pub fn is_active(&self) -> bool {
        Instant::now() < self.deadline && lock(&self.sender).is_some()
    }
    pub fn complete(&self, result: Result<()>) -> bool {
        lock(&self.sender).take().is_some_and(|sender| sender.send(result).is_ok())
    }
}
struct ResumePage<'a>(Option<&'a dyn PageBarrier>);
impl Drop for ResumePage<'_> {
    fn drop(&mut self) {
        if let Some(page) = self.0 {
            page.resume();
        }
    }
}
fn settle(page: &dyn PageBarrier, deadline: Instant) -> Result<()> {
    let (sender, receive) = mpsc::channel();
    let completion = Arc::new(PageSettlement { deadline, sender: Mutex::new(Some(sender)) });
    page.settle(completion.clone());
    let result = receive.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    // Synchronize cancellation with completion before checking for a reply that raced
    // the deadline. A late host learns it must undo its own fence.
    lock(&completion.sender).take();
    result
        .or_else(|_| receive.try_recv())
        .unwrap_or_else(|_| Err(Failure::new(FailureKind::Failed, "Page drain timed out; outcome may be unknown")))
}
struct CaptureSource(std::path::PathBuf);
impl Drop for CaptureSource {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn export(
    owner: &Owner,
    format: wire::engine::ExportFormat,
    output: &Path,
    page: Option<&dyn PageBarrier>,
    deadline: Instant,
    saving: &mut bool,
) -> Result<()> {
    let evaluator = owner.evaluator().ok_or_else(|| invalid("Export requires the capture renderer"))?;
    let directory = std::env::temp_dir().join(format!(
        "pop-capture-{}",
        pop_core::random_hex(16).map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?
    ));
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))?;
    let source = CaptureSource(directory);
    let path = source.0.join("capture.pop");
    {
        if let Some(page) = page {
            settle(page, deadline)?;
        }
        let _resume = ResumePage(page);
        *saving = true;
        call(owner, Owner::flush, deadline)?;
        *saving = false;
        call(owner, |owner, options, callback| owner.capture_source(path.clone(), options, callback), deadline)?;
    }
    crate::capture::export_saved(&path, format, output, evaluator, deadline).map_err(Failure::from)
}
/// A document result remains typed until its transport writes it.
pub type DocumentResult = std::result::Result<EngineSuccess, wire::engine::Failure>;

/// Local results stay typed; replies received from another process retain their bytes.
/// A future peer may have outcome fields this client does not interpret.
#[derive(Debug)]
pub enum CommandReply {
    Local(Box<DocumentResult>),
    Forwarded { json: String, code: Option<OutcomeCode> },
}
impl From<DocumentResult> for CommandReply {
    fn from(result: DocumentResult) -> Self {
        Self::Local(Box::new(result))
    }
}
impl CommandReply {
    fn is_closing(&self) -> bool {
        match self {
            Self::Local(result) => result.as_ref().as_ref().is_err_and(|error| error.code == OutcomeCode::Closing),
            Self::Forwarded { code, .. } => *code == Some(OutcomeCode::Closing),
        }
    }
}
/// Encodes a local result once, or forwards a peer's bytes at the JSON edge.
pub fn encode_reply(result: impl Into<CommandReply>) -> String {
    match result.into() {
        CommandReply::Local(result) => match *result {
            Ok(reply) => crate::encode(&reply),
            Err(error) => crate::encode(&error),
        },
        CommandReply::Forwarded { json, .. } => json,
    }
}
/// Waits for one typed operation, preserving the socket admission deadline.
fn call<T: Send + 'static>(
    owner: &Owner,
    run: impl FnOnce(&Owner, owner::CallOptions, owner::Completion<T>),
    deadline: Instant,
) -> Result<T> {
    call_inner(owner, run, deadline, true)
}
fn call_after<T: Send + 'static>(
    owner: &Owner,
    run: impl FnOnce(&Owner, owner::CallOptions, owner::Completion<T>),
    deadline: Instant,
) -> Result<T> {
    call_inner(owner, run, deadline, false)
}
fn call_inner<T: Send + 'static>(
    owner: &Owner,
    run: impl FnOnce(&Owner, owner::CallOptions, owner::Completion<T>),
    deadline: Instant,
    admission: bool,
) -> Result<T> {
    if admission && Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Closing, "Request expired before admission"));
    }
    let (send, receive) = mpsc::channel();
    run(
        owner,
        owner::CallOptions { view: None, deadline: admission.then_some(deadline) },
        Box::new(move |result| {
            let _ = send.send(result);
        }),
    );
    receive.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_else(|_| {
        Err(Failure::new(FailureKind::Failed, "Owner disconnected or timed out; outcome may be unknown"))
    })
}
/// Serves one request that arrived on `owner`'s socket.
pub fn serve(owner: &Owner, input: &str, page: Option<&dyn PageBarrier>, deadline: Instant) -> DocumentResult {
    match SocketRequest::decode(input).map_err(Failure::from) {
        Ok(request) if Path::new(request.request.document_path()) == owner.path() => {
            dispatch(owner, request.request, page, deadline)
        }
        Ok(_) => Err(classified_failure(invalid("Document path mismatch"), false, false)),
        Err(error) => Err(classified_failure(error, false, false)),
    }
}
/// Runs a validated request against its one owner. Only export reaches native UI. A batch
/// replies once saved: an agent's edit is durable when its command returns.
fn dispatch(
    owner: &Owner,
    request: DocumentRequest,
    page: Option<&dyn PageBarrier>,
    deadline: Instant,
) -> DocumentResult {
    let (mut accepted, mut saving) = (false, false);
    let result = (|| -> Result<EngineSuccess> {
        use owner::{Applied, CommandResult, Reading};
        Ok(match request {
            DocumentRequest::Call { command, args, .. } => {
                let CommandResult { ids, result_json, .. } = call(
                    owner,
                    |owner: &Owner, options, callback| {
                        owner.command(command, args.to_string(), pop_core::Origin::Agent, options, callback)
                    },
                    deadline,
                )?;
                accepted = true;
                saving = true;
                call_after(owner, Owner::flush, deadline)?;
                saving = false;
                EngineSuccess::Call {
                    ok: True,
                    ids,
                    result: serde_json::from_str(&result_json).map_err(|e| invalid(e.to_string()))?,
                }
            }
            DocumentRequest::Describe { .. } => {
                saving = true;
                call(owner, Owner::flush, deadline)?;
                saving = false;
                let Reading { reading, .. } = call(owner, Owner::state, deadline)?;
                let app = &owner.app().app;
                EngineSuccess::Describe { ok: True, state: pop_core::describe::describe(app, reading) }
            }
            DocumentRequest::Get { .. } => {
                saving = true;
                call(owner, Owner::flush, deadline)?;
                saving = false;
                let Reading { reading, .. } = call(owner, Owner::state, deadline)?;
                let app = &owner.app().app;
                EngineSuccess::Get {
                    ok: True,
                    state: GetState {
                        schema: serde_json::from_str(app.document_json()).map_err(|e| invalid(e.to_string()))?,
                        defaults: app.spec().theme_tokens().iter().cloned().collect(),
                        version: reading.version,
                        value: reading.value,
                        theme: reading.theme,
                    },
                }
            }

            DocumentRequest::Batch { batch, attachments, .. } => {
                // The blobs first, in this one request: their reference edits follow, so
                // no close can find a blob waiting for its reference. A refused batch
                // leaves only blobs nothing references, which its close reclaims.
                for bytes in attachments.unwrap_or_default() {
                    let bytes = data_encoding::BASE64
                        .decode(bytes.as_bytes())
                        .map_err(|error| invalid(format!("Invalid attachment bytes: {error}")))?;
                    let _ = call(
                        owner,
                        |owner: &Owner, options, callback| owner.put_attachment(bytes, options, callback),
                        deadline,
                    )?;
                }
                let Applied { ids, .. } = call(
                    owner,
                    |owner: &Owner, options, callback| owner.apply(batch, pop_core::Origin::Agent, options, callback),
                    deadline,
                )?;
                accepted = true;
                saving = true;
                call_after(owner, Owner::flush, deadline)?;
                saving = false;
                EngineSuccess::Batch { ok: True, ids }
            }
            DocumentRequest::ThemeExport { .. } => {
                saving = true;
                let json = call(owner, Owner::export_theme, deadline)?;
                saving = false;
                EngineSuccess::ThemeExport { ok: True, state: ThemeExportState { file: json } }
            }
            DocumentRequest::AttachmentsList { .. } => {
                let items = call(owner, Owner::attachments, deadline)?;
                let state: Vec<_> = items
                    .into_iter()
                    .map(|v| AttachmentInfo { id: v.id, byte_length: v.bytes, mime_type: v.media_type })
                    .collect();
                EngineSuccess::AttachmentsList { ok: True, state }
            }
            DocumentRequest::AttachmentsRead { attachment_id, .. } => {
                let bytes = call(
                    owner,
                    |owner: &Owner, options, callback| owner.read_attachment(attachment_id, options, callback),
                    deadline,
                )?;
                EngineSuccess::AttachmentsRead {
                    ok: True,
                    state: AttachmentBytes { bytes: data_encoding::BASE64.encode(&bytes) },
                }
            }
            DocumentRequest::Export { format, output, .. } => {
                export(owner, format, Path::new(&output), page, deadline, &mut saving)?;
                EngineSuccess::Export { ok: True, output }
            }
        })
    })();
    result.map_err(|error| classified_failure(error, accepted, saving))
}
fn socket_reply(json: String, method: &str) -> CommandReply {
    #[derive(Deserialize)]
    struct Header {
        ok: bool,
        method: Option<String>,
        code: Option<String>,
    }
    let header = match serde_json::from_str::<Header>(&json) {
        Ok(header) => header,
        Err(error) => {
            return Err(classified_failure(
                Failure::new(FailureKind::Failed, format!("Invalid socket response: {error}")),
                false,
                false,
            ))
            .into();
        }
    };
    if header.ok && header.method.as_deref() != Some(method) {
        return Err(classified_failure(
            Failure::new(FailureKind::Failed, "Socket response method mismatch"),
            false,
            false,
        ))
        .into();
    }
    CommandReply::Forwarded { code: header.code.as_deref().and_then(OutcomeCode::from_name), json }
}
fn discovery(path: &Path) -> Result<String> {
    let json = registry::discovery(path)?
        .ok_or_else(|| Failure::new(FailureKind::Locked, "Writer is busy without a ready session; retry later"))?;
    let value = wire::socket::Discovery::decode(&json).map_err(Failure::from)?;
    if Path::new(&value.document_path) != path {
        return Err(invalid("Invalid live session discovery"));
    }
    Ok(value.socket)
}
/// A client's request in `protocol`, naming its document by its resolved path.
fn prepare(mut request: DocumentRequest, protocol: u64) -> Result<SocketRequest> {
    let path = request.document_path_mut();
    let resolved = file::existing(Path::new(path))?;
    *path = resolved.to_string_lossy().into_owned();
    Ok(SocketRequest { protocol, request })
}
/// Engine entry point, for a request written in `protocol`. A closed owner is created only
/// after taking the writer lock; discovery is consulted only when that lock is busy.
/// Exports alone use a snapshot: without an evaluator, a closed export is refused.
pub fn request(input: &str, protocol: u64) -> String {
    if let Some(message) = protocol_mismatch(protocol) {
        return failure(Failure::rejected(Code::RequiresUpdate, message), false, false);
    }
    match wire::engine::DocumentRequest::parse(input) {
        Ok(request) => encode_reply(request_checked(request, protocol, None)),
        Err(error) => failure(invalid(error.to_string()), false, false),
    }
}
pub fn request_with_evaluator(
    input: wire::engine::DocumentRequest,
    protocol: u64,
    evaluator: Option<owner::Evaluator>,
) -> CommandReply {
    request_checked(input, protocol, evaluator)
}
fn request_checked(input: DocumentRequest, protocol: u64, evaluator: Option<owner::Evaluator>) -> CommandReply {
    let request = match prepare(input, protocol) {
        Ok(r) => r,
        Err(mut e) => {
            e.kind = FailureKind::Rejected;
            return Err(classified_failure(e, false, false)).into();
        }
    };
    let path = Path::new(request.request.document_path()).to_owned();
    let admission = Instant::now() + ADMISSION;
    loop {
        enum Connection {
            Local(Owner),
            Live(String),
        }
        let connection = if matches!(request.request, DocumentRequest::Export { .. }) {
            // Only a held writer lock makes discovery authoritative. Acquiring a lease
            // withdraws a crashed owner's publication before the saved read proceeds.
            let live = if registry::discovery(&path).ok().flatten().is_some() {
                match registry::Lease::acquire(&path) {
                    Ok(lease) => {
                        drop(lease);
                        None
                    }
                    Err(store::Error::Locked) => discovery(&path).ok(),
                    Err(error) => return Err(classified_failure(error.into(), false, false)).into(),
                }
            } else {
                None
            };
            match live {
                Some(socket) => Connection::Live(socket),
                None => {
                    let DocumentRequest::Export { format, output, .. } = request.request else { unreachable!() };
                    let Some(evaluator) = &evaluator else {
                        return Err(classified_failure(invalid("Export requires the capture renderer"), false, false))
                            .into();
                    };
                    let deadline = Instant::now() + COMMAND_TIMEOUT;
                    return match crate::capture::export_saved(&path, format, Path::new(&output), evaluator, deadline) {
                        Ok(()) => Ok(EngineSuccess::Export { ok: True, output }),
                        Err(error) => Err(classified_failure(error.into(), false, false)),
                    }.into();
                }
            }
        } else {
            match Owner::open_with_evaluator(&path, store::Mode::Document, Arc::new(|_| {}), evaluator.clone()) {
                Ok(owner) => Connection::Local(owner),
                Err(error) if error.kind == FailureKind::Locked => match discovery(&path) {
                    Ok(socket) => Connection::Live(socket),
                    Err(_) if Instant::now() < admission => {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    Err(error) => return Err(classified_failure(invalid(error.message), false, false)).into(),
                },
                Err(mut error) => {
                    error.kind = FailureKind::Rejected;
                    return Err(classified_failure(error, false, false)).into();
                }
            }
        };
        let answer = match &connection {
            Connection::Local(owner) => {
                dispatch(owner, request.request.clone(), None, Instant::now() + COMMAND_TIMEOUT).into()
            }
            Connection::Live(socket) => match socket::call(Path::new(socket), &crate::encode(&request)) {
                Err(error) => Err(classified_failure(error, false, false)).into(),
                Ok(response) => socket_reply(response, request.request.method()),
            },
        };
        if let Connection::Local(owner) = &connection {
            let close = |owner: &Owner, options, callback| owner.close(None, None, options, callback);
            if let Err(error) = call(owner, close, Instant::now() + COMMAND_TIMEOUT)
                && matches!(request.request, DocumentRequest::Batch { .. } | DocumentRequest::Call { .. })
            {
                return Err(classified_failure(error, true, true)).into();
            }
        }
        if answer.is_closing() && Instant::now() < admission {
            std::thread::sleep(Duration::from_millis(50));
        } else {
            return answer;
        }
    }
}
