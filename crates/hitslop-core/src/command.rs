//! The shared command path: validate a request, then run it on the document's live owner
//! through its socket, or take the writer lock and run the same owner here. A lost
//! mutation reply is never replayed.
use crate::{
    Code,
    envelope::{self, Envelope},
    file, lock,
    owner::{self, Failure, FailureKind, Owner},
    registry, socket, store,
    wire::{self, OutcomeCode, PageRequest, SocketFailure, SocketRequest, SocketSuccess},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, value::RawValue};
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

pub const MAX_REQUEST_BYTES: usize = wire::SOCKET_ATTACHMENT;
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(35);
const ADMISSION: Duration = Duration::from_secs(2);
pub fn protocol() -> String {
    json!({"version":wire::HELPER_PROTOCOL,"minimum":wire::HELPER_MINIMUM_PROTOCOL}).to_string()
}
pub fn supports_protocol(version: u64) -> bool {
    (wire::HELPER_MINIMUM_PROTOCOL..=wire::HELPER_PROTOCOL).contains(&version)
}
/// Refuses a request written in a protocol this build does not serve, before its envelope
/// is read: a request from an engine of another build gets a clear answer.
fn check_protocol(request: &serde_json::Value) -> Result<()> {
    let version = request["protocol"].as_u64().ok_or_else(|| invalid("Invalid socket request"))?;
    let message = if version > wire::HELPER_PROTOCOL {
        "This command needs a newer hitSlop; update hitSlop"
    } else if version < wire::HELPER_MINIMUM_PROTOCOL {
        "This hitSlop no longer serves this command protocol; update @hitslop/cli"
    } else {
        return Ok(());
    };
    Err(Failure::rejected(Code::RequiresUpdate, message))
}
pub type Result<T> = std::result::Result<T, Failure>;
fn invalid(message: impl Into<String>) -> Failure {
    Failure::rejected(Code::InvalidRequest, message)
}

#[derive(Clone, Debug)]
pub struct ExportRequest {
    pub document_path: String,
    pub format: String,
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
#[derive(Serialize)]
struct Success {
    ok: bool,
    #[serde(flatten)]
    result: SocketSuccess,
}
#[derive(Serialize)]
struct Rejected {
    ok: bool,
    #[serde(flatten)]
    result: SocketFailure,
}
fn success(result: SocketSuccess) -> String {
    crate::encode(&Success { ok: true, result })
}
pub(crate) fn failure(error: Failure, accepted: bool, saving: bool) -> String {
    let code = match error.kind {
        FailureKind::Full | FailureKind::Busy | FailureKind::Moved | FailureKind::SaveFailed if saving => {
            OutcomeCode::SaveFailed
        }
        FailureKind::SaveFailed => OutcomeCode::SaveFailed,
        _ if accepted => OutcomeCode::UnknownOutcome,
        FailureKind::Rejected | FailureKind::ReadOnly => OutcomeCode::Rejected,
        FailureKind::Replaced => OutcomeCode::OwnerReplaced,
        FailureKind::Closing | FailureKind::Closed => OutcomeCode::Closing,
        FailureKind::Invalidated => OutcomeCode::OwnerInvalidated,
        _ => OutcomeCode::UnknownOutcome,
    };
    let message = if accepted && code == OutcomeCode::UnknownOutcome {
        "Command was accepted, but its final state could not be confirmed.".into()
    } else {
        error.message
    };
    crate::encode(&Rejected {
        ok: false,
        result: SocketFailure {
            error: message,
            code,
            reason: (code == OutcomeCode::Rejected).then_some(error.reason).flatten(),
            opIndex: (code == OutcomeCode::Rejected).then_some(error.op_index).flatten(),
        },
    })
}
fn fragment(value: impl Serialize) -> Result<Box<RawValue>> {
    serde_json::value::to_raw_value(&value).map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))
}
fn raw(value: String) -> Result<Box<RawValue>> {
    RawValue::from_string(value).map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))
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
    let value: serde_json::Value = serde_json::from_str(input).map_err(|_| invalid("Invalid socket request"))?;
    check_protocol(&value)?;
    if !envelope::is_valid(Envelope::SocketRequest, input.as_bytes()) {
        return Err(invalid("Invalid socket request"));
    }
    let request: SocketRequest = serde_json::from_value(value).map_err(|_| invalid("Invalid socket request"))?;
    // A batch carrying attachments may reach the attachment limit; anything else is small.
    if input.len() > wire::SOCKET_REQUEST && !matches!(request, SocketRequest::Batch { attachments: Some(_), .. }) {
        return Err(invalid("Oversized socket request"));
    }
    if let SocketRequest::Batch { ops, .. } = &request
        && !serde_json::from_str::<serde_json::Value>(ops).is_ok_and(|v| v.is_array())
    {
        return Err(invalid("Operations must be an array"));
    }
    Ok(request)
}
/// Serves one request that arrived on `owner`'s socket.
pub fn serve(owner: &Owner, input: &str, exporter: Option<&Arc<dyn ExportHandler>>, deadline: Instant) -> String {
    match parse(input) {
        Ok(request) if Path::new(request.path()) == owner.path() => dispatch(owner, request, exporter, deadline),
        Ok(_) => failure(invalid("Document path mismatch"), false, false),
        Err(error) => failure(error, false, false),
    }
}
/// Runs a validated request against its one owner. Only export reaches native UI. A batch
/// replies once saved: an agent's edit is durable when its command returns.
fn dispatch(
    owner: &Owner,
    request: SocketRequest,
    exporter: Option<&Arc<dyn ExportHandler>>,
    deadline: Instant,
) -> String {
    let (mut accepted, mut saving) = (false, false);
    let result = (|| -> Result<SocketSuccess> {
        use owner::{Reply, Request};
        let unexpected = || Failure::new(FailureKind::Invalidated, "Unexpected owner result");
        Ok(match request {
            SocketRequest::Get { .. } => {
                saving = true;
                call(owner, Request::Flush, deadline)?;
                saving = false;
                let Reply::State { json } = call(owner, Request::State, deadline)? else {
                    return Err(unexpected());
                };
                let app = &owner.app().app;
                SocketSuccess::Get {
                    state: raw(format!("{{\"schema\":{},\"theme\":{},\"state\":{json}}}", app.descriptor, app.theme))?,
                }
            }
            SocketRequest::Batch { ops, base, attachments, .. } => {
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
                // `ops` was checked to be an array; the core parses the batch.
                let batch_json = match base {
                    Some(base) => format!("{{\"base\":{},\"intents\":{ops}}}", json!(base)),
                    None => format!("{{\"intents\":{ops}}}"),
                };
                let Reply::Applied { sequence, ids, version, .. } =
                    call(owner, Request::Apply { batch_json, origin: crate::Origin::Agent }, deadline)?
                else {
                    return Err(unexpected());
                };
                accepted = true;
                saving = true;
                call_after(owner, Request::Flush, deadline)?;
                saving = false;
                SocketSuccess::Batch { ids, sequence, version }
            }
            SocketRequest::ThemeExport { .. } => {
                saving = true;
                let Reply::State { json } = call(owner, Request::ExportTheme, deadline)? else {
                    return Err(unexpected());
                };
                saving = false;
                SocketSuccess::ThemeExport { state: fragment(json!({"file":json}))? }
            }
            SocketRequest::AttachmentsList { .. } => {
                let Reply::Attachments { items } = call(owner, Request::Attachments, deadline)? else {
                    return Err(unexpected());
                };
                let state: Vec<_> = items.into_iter().map(|v| json!({"id":v.id,"byteLength":v.bytes})).collect();
                SocketSuccess::AttachmentsList { state: fragment(state)? }
            }
            SocketRequest::AttachmentsRead { attachmentID, .. } => {
                let Reply::Bytes { bytes: Some(bytes) } =
                    call(owner, Request::ReadAttachment { id: attachmentID }, deadline)?
                else {
                    return Err(unexpected());
                };
                SocketSuccess::AttachmentsRead {
                    state: fragment(json!({"bytes": data_encoding::BASE64.encode(&bytes)}))?,
                }
            }
            SocketRequest::Export { documentPath, format, output, .. } => SocketSuccess::Export {
                output: export(ExportRequest { document_path: documentPath, format, output }, exporter, deadline)?,
            },
        })
    })();
    match result {
        Ok(reply) => success(reply),
        Err(error) => failure(error, accepted, saving),
    }
}
/// A page request answered: the reply the page receives, and the owner's failure when it
/// refused, which the host reports in its own terms (a storage failure, for instance).
pub struct PageReply {
    pub json: String,
    pub failure: Option<Failure>,
}
fn page_failure(error: Failure) -> PageReply {
    PageReply { json: failure(error.clone(), false, true), failure: Some(error) }
}
/// Runs one document request from the page `view` and answers through `reply`. An edit
/// replies once accepted, ahead of its save; `flush` waits for the save. The window's own
/// requests (config, readiness, resizing, errors) are the host's.
pub fn page(owner: &Owner, view: String, input: &str, reply: impl FnOnce(PageReply) + Send + 'static) {
    use owner::{Reply, Request};
    type Answer = fn(Reply) -> Option<serde_json::Value>;
    fn sequence(reply: Reply) -> Option<serde_json::Value> {
        match reply {
            Reply::Applied { sequence, .. } => Some(json!({ "sequence": sequence })),
            _ => None,
        }
    }
    let request = envelope::is_valid(Envelope::PageRequest, input.as_bytes())
        .then(|| serde_json::from_str::<PageRequest>(input).ok())
        .flatten();
    let (request, answer): (Request, Answer) = match request {
        Some(PageRequest::Open {}) => (Request::State, |reply| match reply {
            Reply::State { json } => Some(json!({ "state": json })),
            _ => None,
        }),
        Some(PageRequest::Apply { batch }) => {
            (Request::Apply { batch_json: batch, origin: crate::Origin::Page }, |reply| match reply {
                Reply::Applied { sequence, ids, text: None, .. } => Some(json!({ "sequence": sequence, "ids": ids })),
                Reply::Applied { sequence, ids, text: Some(text), .. } => Some(json!({
                    "sequence": sequence, "ids": ids, "authored": text.authored,
                    "selectionStart": text.selection[0], "selectionEnd": text.selection[1],
                })),
                _ => None,
            })
        }
        Some(PageRequest::Flush {}) => (Request::Flush, |reply| matches!(reply, Reply::Unit).then(|| json!({}))),
        Some(PageRequest::Undo {}) => (Request::Undo { redo: false }, sequence),
        Some(PageRequest::Redo {}) => (Request::Undo { redo: true }, sequence),
        Some(PageRequest::AttachmentsPut { bytes }) => match data_encoding::BASE64.decode(bytes.as_bytes()) {
            Ok(bytes) => (Request::PutAttachment { bytes }, |reply| match reply {
                Reply::Attachment { item } => Some(json!({ "id": item.id, "byteLength": item.bytes })),
                _ => None,
            }),
            Err(_) => return reply(page_failure(invalid("Invalid attachment bytes"))),
        },
        Some(PageRequest::AttachmentsRead { attachmentID }) => {
            (Request::ReadAttachment { id: attachmentID }, |reply| match reply {
                Reply::Bytes { bytes: Some(bytes) } => Some(json!({ "bytes": data_encoding::BASE64.encode(&bytes) })),
                _ => None,
            })
        }
        Some(_) => return reply(page_failure(invalid("Not a document request"))),
        None => return reply(page_failure(invalid("Invalid page request"))),
    };
    owner.submit(
        request,
        Some(view),
        Box::new(move |result| {
            reply(match result.map(answer) {
                Ok(Some(mut value)) => {
                    value["ok"] = true.into();
                    PageReply { json: value.to_string(), failure: None }
                }
                Ok(None) => page_failure(Failure::new(FailureKind::Invalidated, "Unexpected owner result")),
                Err(error) => page_failure(error),
            })
        }),
    );
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
    if !envelope::is_valid(Envelope::SocketDiscovery, json.as_bytes()) {
        return Err(invalid("Invalid live session discovery"));
    }
    #[derive(Deserialize)]
    struct Discovery {
        socket: String,
        #[serde(rename = "documentPath")]
        path: String,
    }
    let value: Discovery = serde_json::from_str(&json).map_err(|_| invalid("Invalid live session discovery"))?;
    if Path::new(&value.path) != path {
        return Err(invalid("Invalid live session discovery"));
    }
    Ok(value.socket)
}
/// Whether a client's request is an export, which only the app's helper can render.
pub fn is_export(input: &str) -> bool {
    #[derive(Deserialize)]
    struct Method {
        method: String,
    }
    serde_json::from_str::<Method>(input).is_ok_and(|request| request.method == "export")
}
/// A client's request in `protocol`, naming its document by its resolved path.
fn prepare(input: &str, protocol: u64) -> Result<SocketRequest> {
    if input.len() > MAX_REQUEST_BYTES {
        return Err(invalid("Oversized document command"));
    }
    let mut value: serde_json::Value = serde_json::from_str(input).map_err(|_| invalid("Invalid document command"))?;
    let path = value["documentPath"].as_str().ok_or_else(|| invalid("Invalid document command"))?;
    let path = file::resolve(Path::new(path))?;
    registry::identity(&path)?;
    value["documentPath"] = path.to_string_lossy().into_owned().into();
    value["protocol"] = protocol.into();
    parse(&value.to_string())
}
/// Engine entry point, for a request written in `protocol`. A closed owner is created only
/// after taking the writer lock; discovery is consulted only when that lock is busy.
/// Exports alone may use a snapshot.
pub fn request(input: &str, protocol: u64, exporter: Option<Arc<dyn ExportHandler>>) -> String {
    let request = match prepare(input, protocol) {
        Ok(r) => r,
        Err(mut e) => {
            e.kind = FailureKind::Rejected;
            return failure(e, false, false);
        }
    };
    let path = Path::new(request.path()).to_owned();
    let admission = Instant::now() + ADMISSION;
    loop {
        enum Connection {
            Local(Owner),
            Live(String),
        }
        let connection = if matches!(request, SocketRequest::Export { .. }) {
            match discovery(&path) {
                Ok(socket) => Connection::Live(socket),
                Err(_) => {
                    let SocketRequest::Export { documentPath, format, output, .. } = request else { unreachable!() };
                    return match export(
                        ExportRequest { document_path: documentPath, format, output },
                        exporter.as_ref(),
                        Instant::now() + COMMAND_TIMEOUT,
                    ) {
                        Ok(output) => success(SocketSuccess::Export { output }),
                        Err(error) => failure(error, false, false),
                    };
                }
            }
        } else {
            match Owner::open(&path, store::Mode::Document, Arc::new(|_| {})) {
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
                Ok(dispatch(owner, request.clone(), exporter.as_ref(), Instant::now() + COMMAND_TIMEOUT))
            }
            Connection::Live(socket) => match socket::call(Path::new(socket), &crate::encode(&request)) {
                // Transport loss cannot confirm admission: its failure already has an
                // unknown outcome and must not claim the owner accepted the command.
                Err(error) => Ok(failure(error, false, false)),
                Ok(response) => match header(&response) {
                    Ok(result) if result.ok && result.method.as_deref() != Some(request.method()) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accepted_mutation_failure_does_not_claim_it_was_refused() {
        let refusal = "Document changed owners; the request was not applied";
        let result: serde_json::Value =
            serde_json::from_str(&failure(Failure::new(FailureKind::Replaced, refusal), true, false)).unwrap();
        assert_eq!(result["code"], "unknown_outcome");
        assert!(result["error"].as_str().unwrap().contains("could not be confirmed"));
        assert!(!result["error"].as_str().unwrap().contains("not applied"));
        let refused: serde_json::Value =
            serde_json::from_str(&failure(Failure::new(FailureKind::Replaced, refusal), false, false)).unwrap();
        assert_eq!(refused["code"], "owner_replaced");
        assert_eq!(refused["error"], refusal);
    }

    #[test]
    fn an_accepted_mutation_save_failure_keeps_its_classification_and_message() {
        let result: serde_json::Value =
            serde_json::from_str(&failure(Failure::new(FailureKind::Full, "Document storage is full"), true, true))
                .unwrap();
        assert_eq!(result["code"], "save_failed");
        assert_eq!(result["error"], "Document storage is full");
    }
}
