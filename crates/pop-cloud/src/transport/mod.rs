//! Native sharing tasks share one runtime; synchronous native entry points enter it once.
use crate::attachments;
use crate::client::*;
#[cfg(test)]
use crate::library::receipts;
use futures_util::{SinkExt, StreamExt};
use pop_contracts::relay::frame::{Frame, MAX_FRAME};
use pop_contracts::relay::{PauseKind, SyncPhase};
use pop_runtime::owner::{CallOptions, Completion, Owner, SyncConnection, SyncEvent};
use pop_sharing_protocol::failure::{self, Route};
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, tungstenite};
use tokio_util::sync::CancellationToken;

pub(crate) mod account;
mod http;
pub(crate) use http::{Endpoint, HttpOperation, Reply, http};
mod session;
pub(crate) use session::Connection;
type TransportResult<T> = Result<T>;
struct Network {
    runtime: tokio::runtime::Runtime,
    http: reqwest::Client,
}
fn network() -> Result<&'static Network> {
    static NETWORK: OnceLock<Result<Network>> = OnceLock::new();
    NETWORK
        .get_or_init(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .max_blocking_threads(4)
                .thread_name("pop.sharing.io")
                .enable_all()
                .build()
                .map_err(Error::from)?;
            let http = reqwest::Client::builder()
                .http1_only()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| Error::caused(PauseKind::Failed, e.to_string(), e))?;
            Ok(Network { runtime, http })
        })
        .as_ref()
        .map_err(Clone::clone)
}
/// Only synchronous native callers and blocking-pool effects enter here. Every socket
/// task calls the async APIs directly. Tokio rejects block_on in an async task, so a
/// misplaced synchronous call fails immediately instead of deadlocking a worker.
pub(crate) fn spawn_library(work: impl FnOnce() + Send + 'static) -> Result<()> {
    network()?.runtime.spawn_blocking(work);
    Ok(())
}
pub(crate) fn block_on<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    network()?.runtime.handle().block_on(future)
}
/// Temporary staging cleanup must not perform file I/O on a socket task.
pub(crate) fn discard_files(paths: Vec<std::path::PathBuf>, directory: Option<std::path::PathBuf>) {
    let cleanup = move || {
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
        if let Some(directory) = directory {
            let _ = std::fs::remove_dir(directory);
        }
    };
    match network() {
        Ok(network) => {
            network.runtime.spawn_blocking(cleanup);
        }
        Err(_) => cleanup(),
    }
}
pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| Error::caused(PauseKind::Refused, "Sharing work could not be confirmed".into(), e))?
}
/// Completion timeout and dropped completion both leave an unknown owner outcome.
#[derive(Clone, Default)]
pub(crate) struct OwnerOutcome(Arc<std::sync::atomic::AtomicBool>);
impl OwnerOutcome {
    fn unknown(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
}
struct PendingOwner {
    outcome: OwnerOutcome,
    completed: bool,
}
impl Drop for PendingOwner {
    fn drop(&mut self) {
        if !self.completed {
            self.outcome.0.store(true, std::sync::atomic::Ordering::Release);
        }
    }
}
pub(crate) async fn completion<T>(
    outcome: &OwnerOutcome,
    reply: tokio::sync::oneshot::Receiver<T>,
    timeout: Duration,
) -> Result<T> {
    let mut pending = PendingOwner { outcome: outcome.clone(), completed: false };
    let result = tokio::time::timeout(timeout, reply)
        .await
        .map_err(unconfirmed_owner_cause)?
        .map_err(unconfirmed_owner_cause)?;
    pending.completed = true;
    Ok(result)
}
fn unconfirmed_owner_cause(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    let mut error = unconfirmed_owner();
    if let Error::Sharing { source: cause, .. } = &mut error {
        *cause = Some(Arc::new(source));
    }
    error
}
fn unconfirmed_owner() -> Error {
    Error::Sharing {
        kind: PauseKind::Refused,
        reason: "The owner result could not be confirmed; sharing is paused".into(),
        unknown: true,
        status: None,
        code: None,
        source: None,
    }
}
pub(crate) async fn owner_request<T: Send + 'static>(
    owner: &Owner,
    outcome: &OwnerOutcome,
    request: impl FnOnce(&Owner, CallOptions, Completion<T>),
    effect: OwnerEffect,
) -> Result<T> {
    let (send, reply) = tokio::sync::oneshot::channel();
    request(
        owner,
        CallOptions::default(),
        Box::new(move |result| {
            let _ = send.send(result);
        }),
    );
    completion(outcome, reply, Duration::from_secs(20)).await?.map_err(|e| owner_failure(e, effect))
}
#[derive(Clone, Copy)]
pub(crate) enum OwnerEffect {
    Other,
    Attachment,
}
pub(crate) fn owner_failure(error: pop_runtime::owner::Failure, effect: OwnerEffect) -> Error {
    use pop_contracts::wire::Code;
    use pop_runtime::owner::FailureKind;
    let kind = match error.reason {
        Some(Code::StaleBase) if error.kind == FailureKind::Rejected => PauseKind::Conflict,
        Some(Code::RequiresUpdate) => PauseKind::NeedsUpdate,
        Some(Code::MissingDependencies) => PauseKind::MissingHistory,
        Some(Code::TooLarge) if matches!(effect, OwnerEffect::Attachment) => PauseKind::FilesFull,
        Some(_) => PauseKind::Refused,
        None if error.kind == FailureKind::Busy => PauseKind::Busy,
        None => PauseKind::Refused,
    };
    let reason = if kind == PauseKind::Conflict { kind.message(409) } else { error.message.clone() };
    Error::caused(kind, reason, error)
}
async fn response_body(mut response: reqwest::Response, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) =
        response.chunk().await.map_err(|e| Error::caused(PauseKind::Failed, "Relay body unavailable".into(), e))?
    {
        if chunk.len() as u64 > limit.saturating_sub(bytes.len() as u64) {
            return Err("Relay body exceeds its byte limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
type AsyncSocket = WebSocketStream<reqwest::Upgraded>;
async fn upgrade_async(key: &RoomKey) -> Result<AsyncSocket> {
    upgrade_at(&key.origin, Endpoint::Sync(&key.room), &key.bearer()).await
}
async fn upgrade_at(origin: &str, endpoint: Endpoint<'_>, identity: &str) -> Result<AsyncSocket> {
    let nonce = tungstenite::handshake::client::generate_key();
    let response = network()?
        .http
        .get(endpoint.url(origin)?)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", &nonce)
        .bearer_auth(identity)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| Error::caused(PauseKind::Failed, "Relay socket unavailable".into(), e))?;
    let status = response.status().as_u16();
    if status != 101 {
        let bytes = response_body(response, pop_sharing_protocol::CONTROL_BYTES as u64).await.unwrap_or_default();
        let mut error = refusal(status, &bytes, endpoint.classification());
        if status == 401
            && matches!(endpoint, Endpoint::Library)
            && let Error::Sharing { reason, .. } = &mut error
        {
            *reason = "Refresh your account identity".into();
        }
        return Err(error);
    }
    let headers = response.headers();
    let token = |name: &str, expected: &str| {
        headers
            .get(name)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|value| value.split(',').any(|v| v.trim().eq_ignore_ascii_case(expected)))
    };
    if !token("Connection", "upgrade")
        || !token("Upgrade", "websocket")
        || headers.get("Sec-WebSocket-Accept").map(|h| h.as_bytes())
            != Some(tungstenite::handshake::derive_accept_key(nonce.as_bytes()).as_bytes())
    {
        return Err("Invalid relay socket upgrade".into());
    }
    let upgraded =
        response.upgrade().await.map_err(|e| Error::caused(PauseKind::Failed, "Relay socket unavailable".into(), e))?;
    let mut config = tungstenite::protocol::WebSocketConfig::default();
    let limit = if matches!(endpoint, Endpoint::Library) { pop_sharing_protocol::CONTROL_BYTES } else { MAX_FRAME };
    config.max_message_size = Some(limit);
    config.max_frame_size = Some(limit);
    Ok(WebSocketStream::from_raw_socket(upgraded, tungstenite::protocol::Role::Client, Some(config)).await)
}
pub(crate) fn refusal(status: u16, body: &[u8], route: Route) -> Error {
    let body = serde_json::from_slice::<pop_sharing_protocol::http::ApiFailure>(body).ok();
    let kind = failure::classify(status, body.as_ref().map(|b| b.code.as_str()), route);
    let reason = if kind == PauseKind::NeedsUpdate {
        body.as_ref().map_or_else(|| kind.message(status), |b| b.message.clone())
    } else {
        kind.message(status)
    };
    Error::Sharing { kind, reason, unknown: false, status: Some(status), code: body.map(|b| b.code), source: None }
}
fn trace(event: &str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var("POP_SHARING_TRACE").is_ok_and(|v| v == "1")) {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        eprintln!("pop.sharing time={time} event={event}");
    }
}
fn trace_frame(frame: &Frame, incoming: bool) {
    let event = match (incoming, frame) {
        (false, Frame::Hello { .. }) => "send_hello",
        (false, Frame::Push { .. }) => "send_push",
        (true, Frame::Welcome { .. }) => "receive_welcome",
        (true, Frame::Records(_)) => "receive_records",
        (true, Frame::CaughtUp) => "receive_caught_up",
        (true, Frame::Ack { .. }) => "receive_ack",
        (true, Frame::Error { .. }) => "receive_error",
        _ => "unexpected_frame",
    };
    trace(event);
}
#[derive(Default)]
struct Queue {
    events: VecDeque<SyncEvent>,
    bytes: usize,
    overflow: bool,
}
impl Queue {
    fn enqueue(&mut self, event: SyncEvent) {
        if self.overflow {
            return;
        }
        let bytes = match &event {
            SyncEvent::Send { bytes } => bytes.len(),
            _ => 0,
        };
        if self.events.len() >= pop_contracts::relay::limits::QUEUED_FRAMES
            || self.bytes.saturating_add(bytes) > pop_contracts::relay::limits::QUEUED_BYTES
        {
            self.events.clear();
            self.bytes = 0;
            self.overflow = true;
            return;
        }
        self.bytes += bytes;
        self.events.push_back(event);
    }
}
struct Status {
    value: Mutex<(SyncPhase, String, Option<PauseKind>)>,
    notice: Notice,
    cancellation: CancellationToken,
}
impl Status {
    fn set(&self, phase: SyncPhase, detail: String) {
        self.update(phase, detail, None);
    }
    fn paused(&self, kind: PauseKind, reason: String) {
        self.update(SyncPhase::Paused, reason, Some(kind));
    }
    fn update(&self, phase: SyncPhase, detail: String, kind: Option<PauseKind>) {
        if self.cancellation.is_cancelled() {
            return;
        }
        let changed = {
            let mut old = self.value.lock().unwrap();
            let value = (phase, detail, kind);
            if *old == value {
                false
            } else {
                *old = value;
                true
            }
        };
        if changed {
            let callback = self.notice.lock().unwrap().clone();
            if let Some(callback) = callback
                && !self.cancellation.is_cancelled()
            {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
            }
        }
    }
}
// The synchronous fixture entry points drive the actual async implementation.
#[cfg(test)]
fn upgrade(key: &RoomKey) -> TransportResult<AsyncSocket> {
    let key = key.clone();
    block_on(async move { upgrade_async(&key).await })
}
#[cfg(test)]
fn connected(
    owner: &Arc<Owner>,
    key: &RoomKey,
    _stop: &AtomicBool,
    status: &Status,
    retried_full: &mut bool,
    receipt: &receipts::Receipt,
) -> TransportResult<bool> {
    session::connected_fixture(owner, key, status, retried_full, receipt)
}

/// Versions use the seed route's canonical peer encoding.
pub(crate) async fn compact(key: &RoomKey, base: pop_core::sync::Base) -> Result<()> {
    let (bytes, hash, version) = blocking(move || {
        use sha2::{Digest, Sha256};
        let hash = pop_contracts::hex(&Sha256::digest(&base.bytes));
        let version = pop_contracts::relay::frame::version_json(base.frontier.iter().map(|(&p, &n)| (p, n as u32)));
        Ok((base.bytes, hash, version))
    })
    .await?;
    http(
        &key.origin,
        &key.bearer(),
        HttpOperation::new(reqwest::Method::POST, Endpoint::Compact(&key.room)).body(&bytes).headers(&[
            ("Content-Type", "application/octet-stream"),
            ("X-Sharing-SHA256", &hash),
            ("X-Sharing-Version", &version),
        ]),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
use pop_contracts::relay::envelope;
#[cfg(test)]
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
#[cfg(test)]
mod tests {
    use super::*;
    use pop_sharing_protocol::http::{RoomInfo, RoomPhase};
    use pop_storage::store::{Mode, Share, Store};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn access_lookup_requires_an_explicit_permission_denial() {
        for status in [200, 400, 401, 403, 404, 410, 422, 429, 503] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    if stream.read(&mut byte).unwrap() == 0 {
                        break;
                    }
                    request.push(byte[0]);
                }
                write!(stream, "HTTP/1.1 {status} Result\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}")
                    .unwrap();
            });
            let result = block_on(http(
                &origin,
                "identity",
                HttpOperation::new(reqwest::Method::GET, Endpoint::Sharing("fixture"))
                    .reply(Reply::Optional(&[403, 410])),
            ));
            server.join().unwrap();
            match status {
                200 => assert_eq!(result.unwrap(), Some(b"{}".to_vec())),
                403 | 410 => assert_eq!(result.unwrap(), None),
                _ => assert!(result.is_err(), "HTTP {status} cannot authorize removal: {result:?}"),
            }
        }
    }
    #[test]
    fn incomplete_access_lookups_cannot_authorize_removal_or_creation() {
        for status in [403, 410, 404] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                write!(stream, "HTTP/1.1 {status} Reply\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort")
                    .unwrap();
            });
            let result = block_on(http(
                &origin,
                "identity",
                HttpOperation::new(
                    reqwest::Method::GET,
                    if status == 404 { Endpoint::Document("fixture") } else { Endpoint::Sharing("fixture") },
                )
                .reply(Reply::Optional(if status == 404 { &[404] } else { &[403, 410] })),
            ));
            server.join().unwrap();
            assert!(result.is_err(), "an incomplete {status} reply cannot establish absence: {result:?}");
        }
    }
    use tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};

    #[test]
    fn local_owner_failures_preserve_recovery_and_stop_uncertain_replays() {
        use pop_contracts::wire::Code;
        use pop_runtime::owner::{Failure, FailureKind};
        for (kind, reason, expected) in [
            (FailureKind::Rejected, Some(Code::RequiresUpdate), Some(PauseKind::NeedsUpdate)),
            (FailureKind::Rejected, Some(Code::MissingDependencies), Some(PauseKind::MissingHistory)),
            (FailureKind::Busy, None, None),
            (FailureKind::Full, None, Some(PauseKind::Refused)),
            (FailureKind::Invalidated, None, Some(PauseKind::Refused)),
            (FailureKind::Failed, None, Some(PauseKind::Refused)),
            (FailureKind::SaveFailed, None, Some(PauseKind::Refused)),
            (FailureKind::Replaced, None, Some(PauseKind::Refused)),
            (FailureKind::Closing, None, Some(PauseKind::Refused)),
            (FailureKind::Closed, None, Some(PauseKind::Refused)),
            (FailureKind::Moved, None, Some(PauseKind::Refused)),
            (FailureKind::Locked, None, Some(PauseKind::Refused)),
            (FailureKind::ReadOnly, None, Some(PauseKind::Refused)),
        ] {
            let message = format!("local failure {kind:?} {reason:?}");
            let failure = Failure { kind, reason, message: message.clone(), op_index: None, path: vec![] };
            match (Error::from(failure), expected) {
                (pause, Some(expected)) if pause.pause_kind().pauses() => {
                    assert_eq!(pause.pause_kind(), expected, "{kind:?} {reason:?}");
                    assert_eq!(pause.to_string(), message);
                }
                (crate::Error::Sharing { kind: PauseKind::Busy, .. }, None) => {}
                (actual, expected) => panic!("{kind:?} {reason:?}: expected {expected:?}, received {actual:?}"),
            }
        }
        for disconnected in [false, true] {
            let failure = if disconnected {
                let (send, reply) = tokio::sync::oneshot::channel::<()>();
                drop(send);
                block_on(completion(&OwnerOutcome::default(), reply, Duration::from_secs(1))).unwrap_err()
            } else {
                let (_send, reply) = tokio::sync::oneshot::channel::<()>();
                block_on(completion(&OwnerOutcome::default(), reply, Duration::ZERO)).unwrap_err()
            };
            assert!(
                matches!(failure, crate::Error::Sharing { kind: PauseKind::Refused, .. }),
                "an unconfirmed owner result must not be automatically replayed: {failure:?}"
            );
        }
        let attachment = Failure {
            kind: FailureKind::Rejected,
            reason: Some(Code::TooLarge),
            message: "attachment capacity".into(),
            op_index: None,
            path: vec![],
        };
        assert!(matches!(
            owner_failure(attachment, OwnerEffect::Attachment),
            crate::Error::Sharing { kind: PauseKind::FilesFull, .. }
        ));
        let stale = Failure {
            kind: FailureKind::Rejected,
            reason: Some(Code::StaleBase),
            message: "frontier advanced".into(),
            op_index: None,
            path: vec![],
        };
        assert!(matches!(Error::from(stale), crate::Error::Sharing { kind: PauseKind::Conflict, .. }));
    }

    fn upgrade_refusal(headers: &str, body: &[u8], status: u16) -> crate::Error {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
            identity: "memory-only-test-token".into(),
            identity_updates: None,
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        let headers = headers.to_owned();
        let body = body.to_vec();
        let serving = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            write!(stream, "HTTP/1.1 {status} Reply\r\nConnection: close\r\n{headers}\r\n").unwrap();
            for fragment in body.chunks(3) {
                if stream.write_all(fragment).is_err() {
                    break;
                }
            }
        });
        let result = upgrade(&key);
        serving.join().unwrap();
        match result {
            Err(failure) => failure,
            Ok(_) => panic!("a refused upgrade must not create a socket"),
        }
    }

    #[test]
    fn fragmented_and_chunked_upgrade_refusals_preserve_the_update_reason() {
        let body = br#"{"code":"requires_update","message":"Update pop to keep sharing"}"#;
        let chunked = format!(
            "d;extension=yes\r\n{}\r\n{:x}\r\n{}\r\n0\r\nX-Trailer: ignored\r\n\r\n",
            std::str::from_utf8(&body[..13]).unwrap(),
            body.len() - 13,
            std::str::from_utf8(&body[13..]).unwrap()
        );
        for (headers, bytes) in [
            (format!("Content-Length: {}\r\n", body.len()), body.as_slice()),
            (String::new(), body.as_slice()),
            ("Transfer-Encoding: chunked\r\n".into(), chunked.as_bytes()),
        ] {
            let pause = upgrade_refusal(&headers, bytes, 400);
            assert!(pause.pause_kind().pauses());
            assert_eq!(pause.pause_kind(), PauseKind::NeedsUpdate);
            assert_eq!(pause.to_string(), "Update pop to keep sharing");
        }
    }

    #[test]
    fn invalid_upgrade_bodies_keep_the_status_based_refusal() {
        let limit = pop_sharing_protocol::CONTROL_BYTES;
        for (headers, body) in [
            ("Content-Length: 20\r\n".into(), b"short".to_vec()),
            ("Content-Length: 0\r\n".into(), Vec::new()),
            ("Content-Length: 3\r\n".into(), b"bad".to_vec()),
            (format!("Content-Length: {}\r\n", limit + 1), vec![b'x'; limit + 1]),
            (String::new(), vec![b'x'; limit + 1]),
            ("Transfer-Encoding: chunked\r\n".into(), b"5\r\nshort\r\n".to_vec()),
            ("Transfer-Encoding: chunked\r\n".into(), b"invalid\r\n".to_vec()),
            ("Transfer-Encoding: chunked\r\n".into(), b"0\r\n".to_vec()),
        ] {
            let pause = upgrade_refusal(&headers, &body, 400);
            assert!(pause.pause_kind().pauses());
            assert_eq!(pause.pause_kind(), PauseKind::Refused, "{headers}");
        }
        for status in [429, 503] {
            assert!(matches!(
                upgrade_refusal("Content-Length: 20\r\n", b"short", status),
                crate::Error::Sharing { kind: PauseKind::Failed, .. }
            ));
        }
    }

    /// Exercise the actual socket loop and HTTP attachment barrier over loopback.
    #[allow(clippy::result_large_err, reason = "tungstenite fixes the handshake callback error type")]
    fn local_connection(close: CloseCode) -> (bool, SyncPhase, Option<PauseKind>) {
        let (_dir, owner) = crate::client::tests::fixture();
        request(&owner, |owner: &Owner, options, callback| {
            owner.set_share(Share { endpoint: pop_sharing_protocol::TRUSTED_ORIGIN.into() }, options, callback)
        })
        .unwrap();
        let store = Store::open(owner.path(), Mode::Snapshot).unwrap();
        let checkpoint = store.document().unwrap().checkpoint().unwrap();
        let vv = loro::LoroDoc::decode_import_blob_meta(&checkpoint, false).unwrap().partial_end_vv;
        let version: pop_contracts::relay::frame::Version = vv.iter().map(|(&p, &n)| (p, n as u32)).collect();
        let info = serde_json::to_vec(&RoomInfo {
            document: owner.app().document_uuid.clone().unwrap(),
            phase: RoomPhase::Live,
            seed_hash: None,
            seed_size: None,
            app: None,
            compact: false,
        })
        .unwrap();
        let remote_bytes = b"quiet-room attachment".to_vec();
        let remote = pop_storage::store::Attachment::from_bytes(&remote_bytes).unwrap();
        let remote_id = remote.id.clone();
        let inventory = serde_json::to_vec(&vec![pop_sharing_protocol::resources::Resource {
            key: remote.id.clone(),
            hash: remote.id,
            size: remote.bytes,
            media_type: remote.media_type,
        }])
        .unwrap();
        let observing_owner = owner.clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let key = RoomKey {
            room: owner.app().document_uuid.clone().unwrap(),
            identity: "memory-only-test-token".into(),
            identity_updates: None,
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let serving_stop = stop.clone();
        let serving = thread::spawn(move || {
            let mut handlers = vec![];
            while !serving_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                let info = info.clone();
                let version = version.clone();
                let inventory = inventory.clone();
                let remote_bytes = remote_bytes.clone();
                let remote_id = remote_id.clone();
                let observing_owner = observing_owner.clone();
                handlers.push(thread::spawn(move || {
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_secs(25))).unwrap();
                    let mut peek = [0; 1024];
                    let n = stream.peek(&mut peek).unwrap();
                    let first = String::from_utf8_lossy(&peek[..n]);
                    if first.contains("/sync ") {
                        let mut ws = tungstenite::accept_hdr(
                            stream,
                            |request: &tungstenite::handshake::server::Request,
                             response: tungstenite::handshake::server::Response| {
                                assert!(!request.headers().contains_key("Sec-WebSocket-Protocol"));
                                Ok(response)
                            },
                        )
                        .unwrap();
                        assert!(matches!(ws.read().unwrap(), Message::Binary(_)));
                        for frame in [
                            Frame::Welcome { permission: pop_contracts::relay::frame::Permission::Write, version },
                            Frame::CaughtUp,
                        ] {
                            ws.send(Message::Binary(envelope::encode(frame).unwrap().into())).unwrap();
                        }
                        // No Records frame is sent. The initial connection must hydrate files anyway.
                        let deadline = Instant::now() + Duration::from_secs(5);
                        loop {
                            if matches!(
                                request(&observing_owner, |owner: &Owner, options, callback| owner.read_attachment(
                                    remote_id.clone(),
                                    options,
                                    callback
                                ))
                                .unwrap(),
                                bytes if !bytes.is_empty()
                            ) {
                                break;
                            }
                            assert!(
                                Instant::now() < deadline,
                                "a quiet connection never downloaded its missing attachment"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        ws.send(Message::Text("pong".into())).unwrap();
                        ws.send(Message::Pong(vec![1, 2].into())).unwrap();
                        if close == CloseCode::Error {
                            assert_eq!(
                                ws.read().unwrap(),
                                Message::Text("ping".into()),
                                "idle connections keep the relay socket alive"
                            );
                            ws.send(Message::Text("pong".into())).unwrap();
                        }
                        ws.close(Some(CloseFrame { code: close, reason: "retry".into() })).unwrap();
                        // Read the client's close/flush so the stream remains live through its barrier.
                        let _ = ws.read();
                    } else {
                        let mut header = vec![];
                        while !header.ends_with(b"\r\n\r\n") {
                            let mut byte = [0];
                            stream.read_exact(&mut byte).unwrap();
                            header.push(byte[0]);
                        }
                        let header = String::from_utf8(header).unwrap();
                        let size: usize = header
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned))
                            .unwrap_or_else(|| "0".into())
                            .parse()
                            .unwrap();
                        stream.read_exact(&mut vec![0; size]).unwrap();
                        let (code, body) = if header.starts_with("PUT ") {
                            (413, &[][..])
                        } else if header.contains("/blobs ") {
                            (200, inventory.as_slice())
                        } else if header.contains(&format!("/blob/{remote_id} ")) {
                            (200, remote_bytes.as_slice())
                        } else {
                            (200, info.as_slice())
                        };
                        write!(
                            stream,
                            "HTTP/1.1 {code} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .unwrap();
                        stream.write_all(body).unwrap();
                    }
                }));
            }
            for handler in handlers {
                handler.join().unwrap();
            }
        });
        let status = Status {
            value: Mutex::new((SyncPhase::Offline, String::new(), None)),
            notice: Arc::default(),
            cancellation: CancellationToken::new(),
        };
        let result = connected(
            &owner,
            &key,
            &stop,
            &status,
            &mut false,
            &crate::client::tests::test_receipt(&owner, &key, true),
        );
        stop.store(true, Ordering::Relaxed);
        serving.join().unwrap();
        request(&owner, |owner: &Owner, options, callback| owner.close(None, None, options, callback)).unwrap();
        let (phase, _, kind) = status.value.into_inner().unwrap();
        (result.unwrap_or_else(|e| panic!("{}", e.to_string())), phase, kind)
    }
    #[test]
    fn keepalive_pong_and_transient_closes_reconnect_without_pausing() {
        for code in [CloseCode::Error, CloseCode::Again] {
            let (terminal, phase, kind) = local_connection(code);
            assert!(!terminal);
            assert_ne!(phase, SyncPhase::Paused);
            assert_eq!(kind, None);
        }
    }

    /// Answer the socket upgrade with `answer` (or hang up) and report what `connected` made of it.
    fn handshake(
        answer: Option<(u16, &'static str)>,
    ) -> (std::result::Result<bool, Error>, SyncPhase, Option<PauseKind>) {
        let (_dir, owner) = crate::client::tests::fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
            identity: "identity-token".into(),
            identity_updates: None,
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        let serving = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            assert!(String::from_utf8(header).unwrap().starts_with(&format!(
                "GET /v1/documents/{}/sync ",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned()
            )));
            if let Some((code, body)) = answer {
                write!(
                    stream,
                    "HTTP/1.1 {code} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let status = Status {
            value: Mutex::new((SyncPhase::Offline, String::new(), None)),
            notice: Arc::default(),
            cancellation: CancellationToken::new(),
        };
        let result = connected(
            &owner,
            &key,
            &AtomicBool::new(false),
            &status,
            &mut false,
            &crate::client::tests::test_receipt(&owner, &key, true),
        );
        // The relay saw one upgrade: a terminal answer does not loop back for another.
        serving.join().unwrap();
        request(&owner, |owner: &Owner, options, callback| owner.close(None, None, options, callback)).unwrap();
        let (phase, _, kind) = status.value.into_inner().unwrap();
        (result, phase, kind)
    }
    #[test]
    fn permanent_handshake_refusals_pause_instead_of_reconnecting() {
        for (code, body, kind) in [
            (403, "", PauseKind::Refused),
            (404, "", PauseKind::Refused),
            (400, r#"{"code":"requires_update","message":"Update pop to keep sharing"}"#, PauseKind::NeedsUpdate),
        ] {
            let (result, phase, paused) = handshake(Some((code, body)));
            assert!(matches!(result, Ok(true)), "{code} is terminal: {result:?}");
            assert_eq!((phase, paused), (SyncPhase::Paused, Some(kind)), "{code}");
        }
        let (result, phase, paused) = handshake(Some((410, "")));
        assert!(matches!(result, Ok(true)));
        assert_eq!((phase, paused), (SyncPhase::Ended, None));
    }
    #[test]
    fn transient_handshake_failures_stay_retryable() {
        for answer in [Some((503, "")), Some((429, "")), None] {
            let (result, phase, paused) = handshake(answer);
            assert!(
                matches!(result, Err(crate::Error::Sharing { kind: PauseKind::Failed, .. })),
                "{answer:?} retries: {result:?}"
            );
            assert_ne!(phase, SyncPhase::Paused);
            assert_eq!(paused, None);
        }
    }
}
