//! The shared helper command path: validate, acquire the owner or use its live socket,
//! then make one epoch-fenced request. A lost mutation reply is never replayed.
use crate::{
    envelope::{self, Envelope},
    file,
    owner::{self, Failure, FailureKind, Owner},
    registry, socket, store,
    wire::{self, OutcomeCode, SocketFailure, SocketRequest, SocketSuccess},
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
pub type Result<T> = std::result::Result<T, Failure>;
pub(crate) fn failed(kind: FailureKind, message: impl Into<String>) -> Failure {
    Failure {
        kind,
        message: message.into(),
        reason: None,
        op_index: None,
    }
}
fn invalid(message: impl Into<String>) -> Failure {
    Failure {
        kind: FailureKind::Rejected,
        message: message.into(),
        reason: Some("invalid_request".into()),
        op_index: None,
    }
}

#[derive(Clone, Debug)]
pub struct ExportRequest {
    pub document_path: String,
    pub format: String,
    pub output: String,
    pub epoch: Option<String>,
}
/// A native renderer completes this once, while its command is still active. It checks
/// `is_active` immediately before publishing the export to its destination.
pub struct ExportCompletion {
    deadline: Instant,
    sender: Mutex<Option<mpsc::Sender<Result<String>>>>,
}
impl ExportCompletion {
    pub fn is_active(&self) -> bool {
        Instant::now() < self.deadline
            && self
                .sender
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some()
    }
    pub fn complete(&self, result: Result<String>) {
        if let Some(sender) = self.sender.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = sender.send(result);
        }
    }
    fn cancel(&self) {
        self.sender.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}
pub trait ExportHandler: Send + Sync {
    fn export(&self, request: ExportRequest, completion: Arc<ExportCompletion>);
}
fn export(
    request: ExportRequest,
    exporter: Option<&Arc<dyn ExportHandler>>,
    deadline: Instant,
) -> Result<String> {
    let handler = exporter.ok_or_else(|| invalid("Export requires the native renderer"))?;
    let (sender, receive) = mpsc::channel();
    let completion = Arc::new(ExportCompletion {
        deadline,
        sender: Mutex::new(Some(sender)),
    });
    handler.export(request, completion.clone());
    let result = receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_else(|_| {
            Err(failed(
                FailureKind::Failed,
                "Export timed out; outcome may be unknown",
            ))
        });
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
pub(crate) fn failure(
    error: Failure,
    epoch: Option<String>,
    accepted: bool,
    saving: bool,
) -> String {
    let code = match error.kind {
        FailureKind::Full | FailureKind::Busy | FailureKind::Moved | FailureKind::SaveFailed
            if saving =>
        {
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
            epoch,
            reason: (code == OutcomeCode::Rejected)
                .then_some(error.reason)
                .flatten(),
            opIndex: (code == OutcomeCode::Rejected)
                .then_some(error.op_index)
                .flatten(),
        },
    })
}
fn fragment(value: impl Serialize) -> Result<Box<RawValue>> {
    serde_json::value::to_raw_value(&value).map_err(|e| failed(FailureKind::Failed, e.to_string()))
}
fn raw(value: String) -> Result<Box<RawValue>> {
    RawValue::from_string(value).map_err(|e| failed(FailureKind::Failed, e.to_string()))
}
fn call(
    owner: &Owner,
    request: owner::Request,
    epoch: Option<String>,
    deadline: Instant,
) -> Result<owner::Reply> {
    call_inner(owner, request, epoch, deadline, true)
}
fn call_after(
    owner: &Owner,
    request: owner::Request,
    epoch: Option<String>,
    deadline: Instant,
) -> Result<owner::Reply> {
    call_inner(owner, request, epoch, deadline, false)
}
fn call_inner(
    owner: &Owner,
    request: owner::Request,
    epoch: Option<String>,
    deadline: Instant,
    admission: bool,
) -> Result<owner::Reply> {
    if admission && Instant::now() >= deadline {
        return Err(failed(
            FailureKind::Closing,
            "Request expired before admission",
        ));
    }
    let (send, receive) = mpsc::channel();
    let callback = Box::new(move |result| {
        let _ = send.send(result);
    });
    if admission {
        owner.submit_until(request, epoch, None, deadline, callback);
    } else {
        owner.submit(request, epoch, None, callback);
    }
    receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_else(|_| {
            Err(failed(
                FailureKind::Failed,
                "Owner disconnected or timed out; outcome may be unknown",
            ))
        })
}
fn parse(input: &str) -> Result<SocketRequest> {
    if !envelope::is_valid(Envelope::SocketRequest, input.as_bytes()) {
        return Err(invalid("Invalid socket request"));
    }
    let request: SocketRequest =
        serde_json::from_str(input).map_err(|_| invalid("Invalid socket request"))?;
    if input.len() > wire::SOCKET_REQUEST
        && !matches!(request, SocketRequest::AttachmentsPut { .. })
    {
        return Err(invalid("Oversized socket request"));
    }
    if let SocketRequest::Batch { ops, .. } = &request {
        if !serde_json::from_str::<serde_json::Value>(ops).is_ok_and(|v| v.is_array()) {
            return Err(invalid("Operations must be an array"));
        }
    }
    Ok(request)
}
/// Runs a validated socket request against its one owner. Only export reaches native UI.
pub fn dispatch(
    owner: &Owner,
    input: &str,
    exporter: Option<&Arc<dyn ExportHandler>>,
    deadline: Instant,
) -> String {
    let request = match parse(input) {
        Ok(r) => r,
        Err(e) => return failure(e, None, false, false),
    };
    if Path::new(request.path()) != owner.path() {
        return failure(invalid("Document path mismatch"), None, false, false);
    }
    let epoch = owner.epoch();
    let fence = request.epoch().map(str::to_owned);
    let (mut accepted, mut saving) = (false, false);
    let result = (|| -> Result<SocketSuccess> {
        use owner::{Reply, Request};
        let unexpected = || failed(FailureKind::Invalidated, "Unexpected owner result");
        Ok(match request {
            SocketRequest::Hello { .. } => {
                call(owner, Request::Edited, None, deadline)?;
                SocketSuccess::Hello {
                    epoch: epoch.clone(),
                    coreBuildId: crate::BUILD_ID.into(),
                }
            }
            SocketRequest::Get { .. } => {
                saving = true;
                call(owner, Request::Flush, None, deadline)?;
                saving = false;
                let Reply::State { json } = call(owner, Request::State, None, deadline)? else {
                    return Err(unexpected());
                };
                SocketSuccess::Get {
                    epoch: epoch.clone(),
                    state: raw(format!(
                        "{{\"schema\":{},\"state\":{json}}}",
                        owner.app().app.descriptor
                    ))?,
                }
            }
            SocketRequest::Batch { ops, .. } => {
                let Reply::Applied { sequence, ids } = call(
                    owner,
                    Request::Apply {
                        batch_json: format!("{{\"intents\":{ops}}}"),
                        origin: crate::Origin::Agent,
                    },
                    fence,
                    deadline,
                )?
                else {
                    return Err(unexpected());
                };
                accepted = true;
                saving = true;
                call_after(owner, Request::Flush, Some(epoch.clone()), deadline)?;
                saving = false;
                SocketSuccess::Batch {
                    epoch: epoch.clone(),
                    ids,
                    sequence,
                }
            }
            request @ (SocketRequest::ThemeGet { .. }
            | SocketRequest::ThemeSet { .. }
            | SocketRequest::ThemeReset { .. }
            | SocketRequest::ThemeImport { .. }) => {
                let method = request.method();
                let change = match request {
                    SocketRequest::ThemeGet { .. } => owner::ThemeChange::Get,
                    SocketRequest::ThemeSet { values, .. } => {
                        owner::ThemeChange::Set(crate::encode(&values))
                    }
                    SocketRequest::ThemeReset { token, .. } => owner::ThemeChange::Reset(token),
                    SocketRequest::ThemeImport { file, .. } => owner::ThemeChange::Import(file),
                    _ => unreachable!(),
                };
                let Reply::Theme { state, .. } = call(
                    owner,
                    Request::Theme {
                        change,
                        gesture: false,
                    },
                    fence,
                    deadline,
                )?
                else {
                    return Err(unexpected());
                };
                accepted = method != "theme.get";
                saving = true;
                call_after(owner, Request::Flush, Some(epoch.clone()), deadline)?;
                saving = false;
                let state = raw(format!(
                    "{{\"defaults\":{},\"overrides\":{},\"effective\":{}}}",
                    state.defaults, state.overrides, state.effective
                ))?;
                match method {
                    "theme.get" => SocketSuccess::ThemeGet {
                        epoch: epoch.clone(),
                        state,
                    },
                    "theme.set" => SocketSuccess::ThemeSet {
                        epoch: epoch.clone(),
                        state,
                    },
                    "theme.reset" => SocketSuccess::ThemeReset {
                        epoch: epoch.clone(),
                        state,
                    },
                    _ => SocketSuccess::ThemeImport {
                        epoch: epoch.clone(),
                        state,
                    },
                }
            }
            SocketRequest::ThemeExport { .. } => {
                saving = true;
                let Reply::State { json } = call(owner, Request::ExportTheme, None, deadline)?
                else {
                    return Err(unexpected());
                };
                saving = false;
                SocketSuccess::ThemeExport {
                    epoch: epoch.clone(),
                    state: fragment(json!({"file":json}))?,
                }
            }
            SocketRequest::AttachmentsList { .. } => {
                let Reply::Attachments { items } =
                    call(owner, Request::Attachments, None, deadline)?
                else {
                    return Err(unexpected());
                };
                let state: Vec<_> = items
                    .into_iter()
                    .map(|v| json!({"id":v.id,"byteLength":v.bytes}))
                    .collect();
                SocketSuccess::AttachmentsList {
                    epoch: epoch.clone(),
                    state: fragment(state)?,
                }
            }
            SocketRequest::AttachmentsRead { attachmentID, .. } => {
                let Reply::Bytes { bytes: Some(bytes) } = call(
                    owner,
                    Request::ReadAttachment { id: attachmentID },
                    None,
                    deadline,
                )?
                else {
                    return Err(unexpected());
                };
                SocketSuccess::AttachmentsRead {
                    epoch: epoch.clone(),
                    state: fragment(json!({"bytes": data_encoding::BASE64.encode(&bytes)}))?,
                }
            }
            SocketRequest::AttachmentsPut { bytes, .. } => {
                let bytes = data_encoding::BASE64
                    .decode(bytes.as_bytes())
                    .map_err(|_| invalid("Invalid attachment bytes"))?;
                let Reply::Attachment { item } =
                    call(owner, Request::PutAttachment { bytes }, fence, deadline)?
                else {
                    return Err(unexpected());
                };
                SocketSuccess::AttachmentsPut {
                    epoch: epoch.clone(),
                    state: fragment(json!({"id":item.id,"byteLength":item.bytes}))?,
                }
            }
            SocketRequest::Export {
                documentPath,
                format,
                output,
                ..
            } => {
                call(owner, Request::Edited, fence, deadline)?;
                let output = export(
                    ExportRequest {
                        document_path: documentPath,
                        format,
                        output,
                        epoch: Some(epoch.clone()),
                    },
                    exporter,
                    deadline,
                )?;
                SocketSuccess::Export {
                    epoch: Some(epoch.clone()),
                    output,
                }
            }
        })
    })();
    match result {
        Ok(reply) => success(reply),
        Err(error) => failure(error, Some(epoch), accepted, saving),
    }
}
#[derive(Deserialize)]
struct Header {
    ok: bool,
    method: Option<String>,
    epoch: Option<String>,
    #[serde(rename = "coreBuildId")]
    build: Option<String>,
    code: Option<OutcomeCode>,
    error: Option<String>,
}
fn header(json: &str) -> Result<Header> {
    serde_json::from_str(json).map_err(|_| failed(FailureKind::Failed, "Invalid socket response"))
}
fn discovery(path: &Path) -> Result<String> {
    let json = registry::discovery(path)?.ok_or_else(|| {
        failed(
            FailureKind::Locked,
            "Writer is busy without a ready session; retry later",
        )
    })?;
    if !envelope::is_valid(Envelope::SocketDiscovery, json.as_bytes()) {
        return Err(invalid("Invalid live session discovery"));
    }
    #[derive(Deserialize)]
    struct Discovery {
        socket: String,
        #[serde(rename = "documentPath")]
        path: String,
    }
    let value: Discovery =
        serde_json::from_str(&json).map_err(|_| invalid("Invalid live session discovery"))?;
    if Path::new(&value.path) != path {
        return Err(invalid("Invalid live session discovery"));
    }
    Ok(value.socket)
}
fn prepare(input: &str) -> Result<SocketRequest> {
    if input.len() > MAX_REQUEST_BYTES {
        return Err(invalid("Oversized document command"));
    }
    let mut value: serde_json::Value =
        serde_json::from_str(input).map_err(|_| invalid("Invalid document command"))?;
    let method = value["method"]
        .as_str()
        .ok_or_else(|| invalid("Invalid document command"))?
        .to_owned();
    let path = value["documentPath"]
        .as_str()
        .ok_or_else(|| invalid("Invalid document command"))?;
    let path = file::resolve(Path::new(path))?;
    registry::identity(&path)?;
    value["documentPath"] = path.to_string_lossy().into_owned().into();
    if SocketRequest::needs_epoch(&method) {
        value["epoch"] = "x".repeat(128).into();
    }
    parse(&value.to_string())
}
/// Helper entry point. A closed owner is created only after taking the writer lock;
/// discovery is consulted only when that lock is busy. Exports alone may use a snapshot.
pub fn request(input: &str, exporter: Option<Arc<dyn ExportHandler>>) -> String {
    let mut request = match prepare(input) {
        Ok(r) => r,
        Err(mut e) => {
            e.kind = FailureKind::Rejected;
            return failure(e, None, false, false);
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
                    let SocketRequest::Export {
                        documentPath,
                        format,
                        output,
                        ..
                    } = request
                    else {
                        unreachable!()
                    };
                    return match export(
                        ExportRequest {
                            document_path: documentPath,
                            format,
                            output,
                            epoch: None,
                        },
                        exporter.as_ref(),
                        Instant::now() + COMMAND_TIMEOUT,
                    ) {
                        Ok(output) => success(SocketSuccess::Export {
                            epoch: None,
                            output,
                        }),
                        Err(error) => failure(error, None, false, false),
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
                    Err(error) => return failure(invalid(error.message), None, false, false),
                },
                Err(mut error) => {
                    error.kind = FailureKind::Rejected;
                    return failure(error, None, false, false);
                }
            }
        };
        let send = |input: &str| -> Result<String> {
            match &connection {
                Connection::Local(owner) => Ok(dispatch(
                    owner,
                    input,
                    exporter.as_ref(),
                    Instant::now() + COMMAND_TIMEOUT,
                )),
                Connection::Live(path) => socket::call(Path::new(path), input),
            }
        };
        let answer = (|| -> Result<String> {
            let hello_json = send(&crate::encode(&SocketRequest::Hello {
                documentPath: path.to_string_lossy().into_owned(),
            }))
            .map_err(|error| invalid(error.message))?;
            if !envelope::is_valid(Envelope::SocketReply, hello_json.as_bytes()) {
                return Err(invalid("Invalid hello response"));
            }
            let hello = header(&hello_json)?;
            if hello.code == Some(OutcomeCode::Closing) {
                return Ok(hello_json);
            }
            if !hello.ok {
                return Err(invalid(
                    hello.error.unwrap_or_else(|| "Cannot open session".into()),
                ));
            }
            if hello.method.as_deref() != Some("hello")
                || hello.build.as_deref() != Some(crate::BUILD_ID)
            {
                return Err(invalid(
                    "hitSlop was updated while this document was open. Quit and reopen hitSlop, then try again",
                ));
            }
            let epoch = hello.epoch.ok_or_else(|| invalid("Cannot open session"))?;
            request.set_epoch(&epoch);
            let response = match send(&crate::encode(&request)) {
                Ok(response) => response,
                Err(error) => {
                    // Transport loss cannot confirm admission. Its failure already has
                    // unknown outcome, and must not claim the owner accepted the command.
                    return Ok(failure(error, Some(epoch), false, false));
                }
            };
            let result = header(&response)?;
            if result.ok && result.method.as_deref() != Some(request.method()) {
                return Err(failed(
                    FailureKind::Failed,
                    "Socket response method mismatch",
                ));
            }
            Ok(response)
        })();
        if let Connection::Local(owner) = &connection {
            if let Err(error) = call(
                owner,
                owner::Request::Close {
                    preview: None,
                    icon: None,
                },
                None,
                Instant::now() + COMMAND_TIMEOUT,
            ) {
                return failure(error, Some(owner.epoch()), true, true);
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
            Err(error) => return failure(error, None, false, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accepted_mutation_failure_does_not_claim_it_was_refused() {
        let refusal = "Document changed owners; the request was not applied";
        let result: serde_json::Value = serde_json::from_str(&failure(
            failed(FailureKind::Replaced, refusal),
            Some("owner".into()),
            true,
            false,
        ))
        .unwrap();
        assert_eq!(result["code"], "unknown_outcome");
        assert!(
            result["error"]
                .as_str()
                .unwrap()
                .contains("could not be confirmed")
        );
        assert!(!result["error"].as_str().unwrap().contains("not applied"));
        let refused: serde_json::Value = serde_json::from_str(&failure(
            failed(FailureKind::Replaced, refusal),
            Some("owner".into()),
            false,
            false,
        ))
        .unwrap();
        assert_eq!(refused["code"], "owner_replaced");
        assert_eq!(refused["error"], refusal);
    }

    #[test]
    fn an_accepted_mutation_save_failure_keeps_its_classification_and_message() {
        let result: serde_json::Value = serde_json::from_str(&failure(
            failed(FailureKind::Full, "Document storage is full"),
            Some("owner".into()),
            true,
            true,
        ))
        .unwrap();
        assert_eq!(result["code"], "save_failed");
        assert_eq!(result["error"], "Document storage is full");
    }
}
