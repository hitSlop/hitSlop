//! Native socket routing and blocking CLI driver.
use super::*;
use crate::capture::CaptureFormat;
use crate::wire::SocketRequest;
use crate::wire::engine::{
    AttachmentBytes, AttachmentInfo, EngineRequest, EngineSuccess, ExportFormat, GetState, ThemeExportState, True,
};
use crate::{file, lock, registry, socket, store};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
pub const MAX_REQUEST_BYTES: usize = wire::SOCKET_ATTACHMENT;
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(35);
const ADMISSION: Duration = Duration::from_secs(2);
/// The permanent preflight has no document path and never admits an operation.
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
            json!({"ok": true, "method": "hello"}).to_string()
        }
        Ok(()) => failure(invalid("Invalid protocol preflight"), false, false),
    })
}
#[derive(Clone, Debug)]
pub struct ExportRequest {
    pub document_path: String,
    /// None requests a clean document copy after the live page drains.
    pub format: Option<crate::engine::ExportFormat>,
    pub output: String,
}
/// A native renderer completes this once, while its command is still active. It checks
/// `is_active` immediately before publishing the export to its destination.
pub struct ExportCompletion {
    deadline: Instant,
    sender: Mutex<Option<mpsc::Sender<Result<String>>>>,
}
impl ExportCompletion {
    pub fn is_active(&self) -> bool {
        Instant::now() < self.deadline && lock(&self.sender).is_some()
    }
    pub fn complete(&self, result: Result<String>) {
        if let Some(sender) = lock(&self.sender).take() {
            let _ = sender.send(result);
        }
    }
    fn cancel(&self) {
        lock(&self.sender).take();
    }
}
pub trait ExportHandler: Send + Sync {
    fn export(&self, request: ExportRequest, completion: Arc<ExportCompletion>);
}
fn export(request: ExportRequest, exporter: Option<&Arc<dyn ExportHandler>>, deadline: Instant) -> Result<String> {
    let handler = exporter.ok_or_else(|| invalid("Export requires the native renderer"))?;
    let (sender, receive) = mpsc::channel();
    let completion = Arc::new(ExportCompletion { deadline, sender: Mutex::new(Some(sender)) });
    handler.export(request, completion.clone());
    let result = receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_else(|_| Err(Failure::new(FailureKind::Failed, "Export timed out; outcome may be unknown")));
    completion.cancel();
    result
}
fn success(result: EngineSuccess) -> String {
    crate::encode(&result)
}
/// One owner request, admitted before `deadline`.
fn call(owner: &Owner, request: owner::Request, deadline: Instant) -> Result<owner::Reply> {
    call_inner(owner, request, deadline, true)
}
/// The rest of a request already admitted: it waits only for the deadline.
fn call_after(owner: &Owner, request: owner::Request, deadline: Instant) -> Result<owner::Reply> {
    call_inner(owner, request, deadline, false)
}
fn call_inner(owner: &Owner, request: owner::Request, deadline: Instant, admission: bool) -> Result<owner::Reply> {
    if admission && Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Closing, "Request expired before admission"));
    }
    let (send, receive) = mpsc::channel();
    let callback = Box::new(move |result| {
        let _ = send.send(result);
    });
    if admission {
        owner.submit_until(request, None, deadline, callback);
    } else {
        owner.submit(request, None, callback);
    }
    receive.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_else(|_| {
        Err(Failure::new(FailureKind::Failed, "Owner disconnected or timed out; outcome may be unknown"))
    })
}
/// A request as it arrives: its protocol first, then its envelope.
fn parse(input: &str) -> Result<SocketRequest> {
    SocketRequest::decode(input).map_err(Failure::from)
}
/// Serves one request that arrived on `owner`'s socket.
pub fn serve(owner: &Owner, input: &str, exporter: Option<&Arc<dyn ExportHandler>>, deadline: Instant) -> String {
    match parse(input) {
        Ok(request) if Path::new(request.request.document_path().expect("document request")) == owner.path() => {
            dispatch(owner, request.request, exporter, deadline)
        }
        Ok(_) => failure(invalid("Document path mismatch"), false, false),
        Err(error) => failure(error, false, false),
    }
}
/// Runs a validated request against its one owner. Only export reaches native UI. A batch
/// replies once saved: an agent's edit is durable when its command returns.
fn dispatch(
    owner: &Owner,
    request: EngineRequest,
    exporter: Option<&Arc<dyn ExportHandler>>,
    deadline: Instant,
) -> String {
    let (mut accepted, mut saving) = (false, false);
    let result = (|| -> Result<EngineSuccess> {
        use owner::{Reply, Request};
        let unexpected = || Failure::new(FailureKind::Invalidated, "Unexpected owner result");
        Ok(match request {
            EngineRequest::Call { command, args, .. } => {
                let Reply::Command { ids, result_json, .. } = call(
                    owner,
                    Request::Command { name: command, args_json: args.to_string(), origin: crate::Origin::Agent },
                    deadline,
                )?
                else {
                    return Err(unexpected());
                };
                accepted = true;
                saving = true;
                call_after(owner, Request::Flush, deadline)?;
                saving = false;
                EngineSuccess::Call {
                    ok: True,
                    ids,
                    result: serde_json::from_str(&result_json).map_err(|e| invalid(e.to_string()))?,
                }
            }
            EngineRequest::Describe { .. } => {
                saving = true;
                call(owner, Request::Flush, deadline)?;
                saving = false;
                let Reply::State { reading, .. } = call(owner, Request::State, deadline)? else {
                    return Err(unexpected());
                };
                let app = &owner.app().app;
                let mut state = serde_json::to_value(reading).map_err(|e| invalid(e.to_string()))?;
                state["schema"] = serde_json::from_str(app.document_json()).map_err(|e| invalid(e.to_string()))?;
                EngineSuccess::Describe {
                    ok: True,
                    state: serde_json::from_value(crate::describe::describe(
                        serde_json::to_value(app.metadata()).expect("metadata"),
                        state,
                        app.command_metadata(),
                    ))
                    .map_err(|e| invalid(e.to_string()))?,
                }
            }
            EngineRequest::Get { .. } => {
                saving = true;
                call(owner, Request::Flush, deadline)?;
                saving = false;
                let Reply::State { reading, .. } = call(owner, Request::State, deadline)? else {
                    return Err(unexpected());
                };
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

            EngineRequest::Batch { batch, attachments, .. } => {
                // The blobs first, in this one request: their reference edits follow, so
                // no close can find a blob waiting for its reference. A refused batch
                // leaves only blobs nothing references, which its close reclaims.
                for bytes in attachments.unwrap_or_default() {
                    let bytes = data_encoding::BASE64
                        .decode(bytes.as_bytes())
                        .map_err(|_| invalid("Invalid attachment bytes"))?;
                    let Reply::Attachment { .. } = call(owner, Request::PutAttachment { bytes }, deadline)? else {
                        return Err(unexpected());
                    };
                }
                let Reply::Applied { ids, .. } =
                    call(owner, Request::Apply { batch, origin: crate::Origin::Agent }, deadline)?
                else {
                    return Err(unexpected());
                };
                accepted = true;
                saving = true;
                call_after(owner, Request::Flush, deadline)?;
                saving = false;
                EngineSuccess::Batch { ok: True, ids }
            }
            EngineRequest::ThemeExport { .. } => {
                saving = true;
                let Reply::ThemeFile { json } = call(owner, Request::ExportTheme, deadline)? else {
                    return Err(unexpected());
                };
                saving = false;
                EngineSuccess::ThemeExport { ok: True, state: ThemeExportState { file: json } }
            }
            EngineRequest::AttachmentsList { .. } => {
                let Reply::Attachments { items } = call(owner, Request::Attachments, deadline)? else {
                    return Err(unexpected());
                };
                let state: Vec<_> = items
                    .into_iter()
                    .map(|v| AttachmentInfo { id: v.id, byte_length: v.bytes, mime_type: v.media_type })
                    .collect();
                EngineSuccess::AttachmentsList { ok: True, state }
            }
            EngineRequest::AttachmentsRead { attachment_id, .. } => {
                let Reply::Bytes { bytes: Some(bytes) } =
                    call(owner, Request::ReadAttachment { id: attachment_id }, deadline)?
                else {
                    return Err(unexpected());
                };
                EngineSuccess::AttachmentsRead {
                    ok: True,
                    state: AttachmentBytes { bytes: data_encoding::BASE64.encode(&bytes) },
                }
            }
            EngineRequest::Copy { document_path, output, .. } => {
                saving = true;
                let output = if exporter.is_some() {
                    export(ExportRequest { document_path, format: None, output }, exporter, deadline)?
                } else {
                    call(
                        owner,
                        owner::Request::Copy { destination: Path::new(&output).to_owned(), preview: None, icon: None },
                        deadline,
                    )?;
                    output
                };
                EngineSuccess::Copy { ok: True, output }
            }
            EngineRequest::Export { document_path, format, output, .. } => EngineSuccess::Export {
                ok: True,
                output: export(ExportRequest { document_path, format: Some(format), output }, exporter, deadline)?,
            },
            _ => return Err(invalid("Not a document command")),
        })
    })();
    match result {
        Ok(reply) => success(reply),
        Err(error) => failure(error, accepted, saving),
    }
}
#[derive(Deserialize)]
struct Header {
    ok: bool,
    method: Option<String>,
    code: Option<OutcomeCode>,
}
fn header(json: &str) -> Result<Header> {
    serde_json::from_str(json).map_err(|_| Failure::new(FailureKind::Failed, "Invalid socket response"))
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
fn prepare(mut request: EngineRequest, protocol: u64) -> Result<SocketRequest> {
    if !request.is_document() {
        return Err(invalid("Not a document command"));
    }
    let path = request.document_path_mut().expect("document request");
    let resolved = file::resolve(Path::new(path))?;
    registry::identity(&resolved)?;
    *path = resolved.to_string_lossy().into_owned();
    request.check().map_err(invalid)?;
    let limit = request.socket_limit();
    let request = SocketRequest { protocol, request };
    wire::check_json_size(&request, limit).map_err(Failure::from)?;
    Ok(request)
}
/// Engine entry point, for a request written in `protocol`. A closed owner is created only
/// after taking the writer lock; discovery is consulted only when that lock is busy.
/// Exports alone use a snapshot: without an evaluator, a closed export is refused.
pub fn request(input: &str, protocol: u64) -> String {
    if let Some(message) = protocol_mismatch(protocol) {
        return failure(Failure::rejected(Code::RequiresUpdate, message), false, false);
    }
    match wire::engine::EngineRequest::parse(input) {
        Ok(request) => request_checked(request, protocol, None),
        Err(error) => failure(invalid(error.to_string()), false, false),
    }
}
pub fn request_with_evaluator(
    input: wire::engine::EngineRequest,
    protocol: u64,
    evaluator: Option<owner::Evaluator>,
) -> String {
    if let Some(message) = protocol_mismatch(protocol) {
        return failure(Failure::rejected(Code::RequiresUpdate, message), false, false);
    }
    request_checked(input, protocol, evaluator)
}
fn request_checked(input: EngineRequest, protocol: u64, evaluator: Option<owner::Evaluator>) -> String {
    let request = match prepare(input, protocol) {
        Ok(r) => r,
        Err(mut e) => {
            e.kind = FailureKind::Rejected;
            return failure(e, false, false);
        }
    };
    let path = Path::new(request.request.document_path().expect("document request")).to_owned();
    let admission = Instant::now() + ADMISSION;
    loop {
        enum Connection {
            Local(Owner),
            Live(String),
        }
        let connection = if matches!(request.request, EngineRequest::Export { .. }) {
            match discovery(&path) {
                Ok(socket) => Connection::Live(socket),
                Err(_) => {
                    let EngineRequest::Export { format, output, .. } = request.request else { unreachable!() };
                    let Some(evaluator) = &evaluator else {
                        return failure(invalid("Export requires the capture renderer"), false, false);
                    };
                    let format = match format {
                        ExportFormat::Png => CaptureFormat::Png,
                        ExportFormat::Pdf => CaptureFormat::Pdf,
                    };
                    let deadline = Instant::now() + COMMAND_TIMEOUT;
                    return match crate::capture::export_saved(&path, format, Path::new(&output), evaluator, deadline) {
                        Ok(()) => success(EngineSuccess::Export { ok: True, output }),
                        Err(error) => failure(error.into(), false, false),
                    };
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
                    Err(error) => return failure(invalid(error.message), false, false),
                },
                Err(mut error) => {
                    error.kind = FailureKind::Rejected;
                    return failure(error, false, false);
                }
            }
        };
        let answer = match &connection {
            Connection::Local(owner) => {
                Ok(dispatch(owner, request.request.clone(), None, Instant::now() + COMMAND_TIMEOUT))
            }
            Connection::Live(socket) => match socket::call(Path::new(socket), &crate::encode(&request)) {
                // Transport loss cannot confirm admission: its failure already has an
                // unknown outcome and must not claim the owner accepted the command.
                Err(error) => Ok(failure(error, false, false)),
                Ok(response) => match header(&response) {
                    Ok(result) if result.ok && result.method.as_deref() != Some(request.request.method()) => {
                        Err(Failure::new(FailureKind::Failed, "Socket response method mismatch"))
                    }
                    Ok(_) => Ok(response),
                    Err(error) => Err(error),
                },
            },
        };
        if let Connection::Local(owner) = &connection {
            let close = owner::Request::Close { preview: None, icon: None };
            if let Err(error) = call(owner, close, Instant::now() + COMMAND_TIMEOUT) {
                return failure(error, true, true);
            }
        }
        match answer {
            Ok(reply)
                if header(&reply).is_ok_and(|reply| reply.code == Some(OutcomeCode::Closing))
                    && Instant::now() < admission =>
            {
                // A closing refusal confirms this request was not admitted. Once the
                // retry window ends, return the owner's complete outcome unchanged.
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(reply) => return reply,
            Err(error) => return failure(error, false, false),
        }
    }
}
