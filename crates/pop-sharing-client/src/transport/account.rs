//! One account feed, with fresh snapshots after reconnect and bounded callback delivery.
use super::*;
use pop_sharing_protocol::library::{AccountEntry, LibraryFrame, SOCKET_PROTOCOL};
use std::sync::{
    Condvar,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{Notify, watch};

/// A complete library snapshot or a subsequent change.
#[derive(Clone, Debug)]
pub enum LibraryUpdate {
    Snapshot(Vec<AccountEntry>),
    Upsert(AccountEntry),
    Remove(String),
}

#[derive(Default)]
struct FeedState {
    ready: bool,
    error: Option<String>,
}

/// A stoppable account subscription. Document sockets remain independent.
pub struct Account {
    identity: watch::Sender<String>,
    cancel: CancellationToken,
    wake: Arc<Notify>,
    ready: Arc<(Mutex<FeedState>, Condvar)>,
    generation: Arc<AtomicU64>,
}
impl Account {
    /// Start a library subscription; callbacks run on a separate serial thread.
    ///
    /// # Errors
    /// Returns an error if the network runtime or delivery thread cannot start.
    pub fn start(token: &str, apply: Arc<dyn Fn(LibraryUpdate) -> Result<()> + Send + Sync>) -> Result<Arc<Self>> {
        Self::start_at(pop_sharing_protocol::TRUSTED_ORIGIN, token, apply)
    }
    fn start_at(
        origin: &str,
        token: &str,
        apply: Arc<dyn Fn(LibraryUpdate) -> Result<()> + Send + Sync>,
    ) -> Result<Arc<Self>> {
        let (identity, receiver) = watch::channel(token.to_owned());
        let account = Arc::new(Self {
            identity,
            cancel: CancellationToken::new(),
            wake: Arc::new(Notify::new()),
            ready: Arc::new((Mutex::new(FeedState::default()), Condvar::new())),
            generation: Arc::new(AtomicU64::new(0)),
        });
        let (send, receive) = mpsc::sync_channel::<(u64, LibraryUpdate)>(32);
        let generation = account.generation.clone();
        let cancel = account.cancel.clone();
        let ready = account.ready.clone();
        let retry = account.wake.clone();
        std::thread::Builder::new()
            .name("pop.library.apply".into())
            .spawn(move || {
                while let Ok((epoch, update)) = receive.recv() {
                    if cancel.is_cancelled() {
                        break;
                    }
                    if generation.load(Ordering::Acquire) != epoch {
                        continue;
                    }
                    let snapshot = matches!(update, LibraryUpdate::Snapshot(_));
                    if let Err(error) = apply(update) {
                        ready.0.lock().unwrap().error = Some(error);
                        retry.notify_one();
                        continue;
                    }
                    if snapshot && generation.load(Ordering::Acquire) == epoch {
                        let mut state = ready.0.lock().unwrap();
                        state.ready = true;
                        state.error = None;
                        ready.1.notify_all();
                    }
                }
            })
            .map_err(fail)?;
        network().map_err(String::from)?.runtime.spawn(run(
            origin.to_owned(),
            receiver,
            account.cancel.clone(),
            account.wake.clone(),
            account.ready.clone(),
            account.generation.clone(),
            send,
        ));
        Ok(account)
    }
    /// Refresh credentials. A changed identity immediately replaces the connection.
    pub fn set_identity(&self, token: &str) {
        self.identity.send_if_modified(|old| {
            if old == token {
                false
            } else {
                *old = token.to_owned();
                true
            }
        });
    }
    /// Reconnect after foregrounding or an explicit refresh.
    pub fn wake(&self) {
        self.wake.notify_one();
    }
    /// Stop delivery and discard queued callbacks.
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.cancel.cancel();
        self.ready.1.notify_all();
    }
    /// Latest connection failure, retained until a complete snapshot is applied.
    pub fn last_error(&self) -> Option<String> {
        self.ready.0.lock().unwrap().error.clone()
    }
    /// Wait for a complete applied snapshot, bounded so offline startup remains usable.
    pub fn wait_snapshot(&self, timeout: Duration) -> bool {
        let ready = self.ready.0.lock().unwrap();
        self.ready
            .1
            .wait_timeout_while(ready, timeout, |state| !state.ready && !self.cancel.is_cancelled())
            .unwrap()
            .0
            .ready
    }
}
impl Drop for Account {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn run(
    origin: String,
    mut identity: watch::Receiver<String>,
    cancel: CancellationToken,
    wake: Arc<Notify>,
    ready: Arc<(Mutex<FeedState>, Condvar)>,
    generation: Arc<AtomicU64>,
    send: mpsc::SyncSender<(u64, LibraryUpdate)>,
) {
    let mut backoff = pop_sharing_protocol::limits::BACKOFF_MIN_MS;
    loop {
        if cancel.is_cancelled() {
            break;
        }
        let epoch = generation.fetch_add(1, Ordering::AcqRel) + 1;
        ready.0.lock().unwrap().ready = false;
        let token = identity.borrow_and_update().clone();
        let session = async {
            let url = format!("{origin}/v1/account/library");
            let mut socket = upgrade_at(&url, &token, SOCKET_PROTOCOL).await?;
            let mut snapshot = Some(Vec::new());
            let mut ping = tokio::time::interval(Duration::from_millis(pop_sharing_protocol::limits::PING_MS));
            loop {
                let message = tokio::select! {
                    _ = ping.tick() => { socket.send(Message::Text("ping".into())).await.map_err(|_| HttpFailure::from("Library heartbeat failed"))?; continue; },
                    message = socket.next() => message,
                };
                let Some(Ok(message)) = message else {
                    return Err::<(), _>(HttpFailure::from("Library disconnected"));
                };
                let bytes = match message {
                    Message::Text(text) if text == "pong" => continue,
                    Message::Text(text) => text.as_bytes().to_vec(),
                    Message::Ping(bytes) => {
                        socket
                            .send(Message::Pong(bytes))
                            .await
                            .map_err(|_| HttpFailure::from("Library heartbeat failed"))?;
                        continue;
                    }
                    Message::Pong(_) => continue,
                    Message::Close(Some(close)) if u16::from(close.code) == 4401 => {
                        return Err(HttpFailure::Paused(pop_core::sync::Pause {
                            kind: PauseKind::Refused,
                            reason: "Refresh your account identity".into(),
                        }));
                    }
                    Message::Close(Some(close)) if u16::from(close.code) == 1008 => return Err(HttpFailure::Ended),
                    _ => return Err(HttpFailure::from("Invalid library message")),
                };
                let frame = LibraryFrame::decode(&bytes).map_err(|e| HttpFailure::from(e.to_string()))?;
                let update = match frame {
                    LibraryFrame::Snapshot { entries, last } => {
                        let pending =
                            snapshot.as_mut().ok_or_else(|| HttpFailure::from("Unexpected library snapshot"))?;
                        pending.extend(entries);
                        if !last {
                            continue;
                        }
                        backoff = pop_sharing_protocol::limits::BACKOFF_MIN_MS;
                        LibraryUpdate::Snapshot(snapshot.take().unwrap_or_default())
                    }
                    LibraryFrame::Upsert { entry } if snapshot.is_none() => LibraryUpdate::Upsert(entry),
                    LibraryFrame::Remove { document } if snapshot.is_none() => LibraryUpdate::Remove(document),
                    _ => return Err(HttpFailure::from("Library change interrupted snapshot")),
                };
                send.try_send((epoch, update)).map_err(|_| HttpFailure::from("Library delivery overflow"))?;
            }
        };
        let outcome = tokio::select! {
            _ = cancel.cancelled() => break,
            _ = identity.changed() => continue,
            _ = wake.notified() => continue,
            outcome = session => outcome,
        };
        if let Err(error) = &outcome {
            let message = match error {
                HttpFailure::Ended => "The cloud account was deleted".into(),
                HttpFailure::Paused(pause) => pause.reason.clone(),
                HttpFailure::Other(message) | HttpFailure::Busy(message) => message.clone(),
                HttpFailure::Conflict => "The cloud library is busy".into(),
            };
            ready.0.lock().unwrap().error = Some(message);
        }
        if matches!(outcome, Err(HttpFailure::Ended | HttpFailure::Paused(_))) {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = identity.changed() => continue,
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = identity.changed() => {},
            _ = wake.notified() => {},
            _ = tokio::time::sleep(Duration::from_millis(backoff)) => {},
        }
        backoff = backoff.saturating_mul(2).min(pop_sharing_protocol::limits::BACKOFF_MAX_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use tungstenite::handshake::server::{Request, Response};

    #[expect(clippy::result_large_err, reason = "tungstenite fixes the handshake callback error type")]
    fn accept(listener: &TcpListener) -> tungstenite::WebSocket<TcpStream> {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        tungstenite::accept_hdr(stream, |request: &Request, mut response: Response| {
            assert_eq!(request.uri().path(), "/v1/account/library");
            assert_eq!(request.headers()["sec-websocket-protocol"], SOCKET_PROTOCOL);
            response.headers_mut().insert("sec-websocket-protocol", SOCKET_PROTOCOL.parse().unwrap());
            Ok(response)
        })
        .unwrap()
    }

    #[test]
    fn incomplete_snapshot_is_discarded_on_reconnect_and_stop_fences_delivery() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (seen, received) = mpsc::channel();
        let (finish, finished) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let mut first = accept(&listener);
            first.send(Message::Text(r#"{"type":"snapshot","entries":[],"last":false}"#.into())).unwrap();
            drop(first);
            let mut second = accept(&listener);
            second.send(Message::Text(r#"{"type":"snapshot","entries":[],"last":true}"#.into())).unwrap();
            second
                .send(Message::Text(r#"{"type":"remove","document":"00000000-0000-4000-8000-000000000001"}"#.into()))
                .unwrap();
            finished.recv_timeout(Duration::from_secs(5)).unwrap();
            let _ = second
                .send(Message::Text(r#"{"type":"remove","document":"00000000-0000-4000-8000-000000000002"}"#.into()));
        });
        let account = Account::start_at(
            &origin,
            "fixture-token",
            Arc::new(move |event| {
                seen.send(event).unwrap();
                Ok(())
            }),
        )
        .unwrap();
        assert!(
            matches!(received.recv_timeout(Duration::from_secs(5)).unwrap(), LibraryUpdate::Snapshot(v) if v.is_empty())
        );
        assert!(
            matches!(received.recv_timeout(Duration::from_secs(5)).unwrap(), LibraryUpdate::Remove(id) if id.ends_with('1'))
        );
        assert!(account.wait_snapshot(Duration::from_secs(1)));
        account.stop();
        finish.send(()).unwrap();
        server.join().unwrap();
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
    }
}
