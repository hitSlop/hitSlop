//! One account feed, with fresh snapshots after reconnect and bounded callback delivery.
use super::*;
use pop_contracts::relay::PauseKind;
use pop_sharing_protocol::library::{AccountEntry, LibraryFrame};
use pop_sharing_protocol::supervisor::{Action, Event, Supervisor};
use std::sync::Condvar;
use tokio::sync::{Notify, mpsc, watch};

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
    error: Option<Error>,
    delivery_failed: bool,
}

/// A stoppable account subscription. Document sockets remain independent.
pub struct Account {
    identity: watch::Sender<String>,
    cancel: CancellationToken,
    wake: Arc<Notify>,
    ready: Arc<(Mutex<FeedState>, Condvar)>,
    notice: Notice,
}
impl Account {
    /// Start a library subscription; serial callbacks run on the shared blocking pool.
    ///
    /// # Errors
    /// Returns an error if the network runtime cannot start.
    pub fn start(
        token: &str,
        apply: Arc<dyn Fn(LibraryUpdate) -> Result<()> + Send + Sync>,
        notice: Notice,
    ) -> Result<Arc<Self>> {
        Self::start_at(pop_sharing_protocol::TRUSTED_ORIGIN, token, apply, notice)
    }
    fn start_at(
        origin: &str,
        token: &str,
        apply: Arc<dyn Fn(LibraryUpdate) -> Result<()> + Send + Sync>,
        notice: Notice,
    ) -> Result<Arc<Self>> {
        let (identity, receiver) = watch::channel(token.to_owned());
        let account = Arc::new(Self {
            identity,
            cancel: CancellationToken::new(),
            wake: Arc::new(Notify::new()),
            ready: Arc::new((Mutex::new(FeedState::default()), Condvar::new())),
            notice,
        });
        let (send, mut receive) = mpsc::channel::<(CancellationToken, LibraryUpdate)>(32);
        let cancel = account.cancel.clone();
        let ready = account.ready.clone();
        let retry = account.wake.clone();
        let notice = account.notice.clone();
        network()?.runtime.spawn(async move {
            while let Some((epoch, update)) = receive.recv().await {
                if cancel.is_cancelled() {
                    break;
                }
                if epoch.is_cancelled() {
                    continue;
                }
                let snapshot = matches!(update, LibraryUpdate::Snapshot(_));
                let apply = apply.clone();
                let applying = epoch.clone();
                let stopping = cancel.clone();
                let result = blocking(move || {
                    if stopping.is_cancelled() || applying.is_cancelled() {
                        return Ok(());
                    }
                    apply(update)
                })
                .await;
                if let Err(error) = result {
                    if !epoch.is_cancelled() {
                        let mut state = pop_core::lock(&ready.0);
                        state.error = Some(error);
                        state.ready = false;
                        state.delivery_failed = true;
                        drop(state);
                        crate::library::changes::notify(&notice);
                        retry.notify_one();
                    }
                    continue;
                }
                if snapshot && !epoch.is_cancelled() {
                    let mut state = pop_core::lock(&ready.0);
                    state.ready = true;
                    state.error = None;
                    ready.1.notify_all();
                    drop(state);
                    crate::library::changes::notify(&notice);
                }
            }
        });
        network()?.runtime.spawn(run(
            origin.to_owned(),
            receiver,
            send,
            FeedSession {
                cancel: account.cancel.clone(),
                wake: account.wake.clone(),
                ready: account.ready.clone(),
                notice: account.notice.clone(),
            },
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
        self.cancel.cancel();
        self.ready.1.notify_all();
    }
    pub fn status(&self) -> crate::client::ClientStatus {
        let state = pop_core::lock(&self.ready.0);
        let pause_kind = state.error.as_ref().map(Error::pause_kind).filter(|kind| kind.pauses());
        crate::client::ClientStatus {
            phase: if state.ready {
                SyncPhase::Synced
            } else if pause_kind.is_some() {
                SyncPhase::Paused
            } else if state.error.is_some() {
                SyncPhase::Offline
            } else {
                SyncPhase::Syncing
            },
            detail: state
                .error
                .as_ref()
                .map_or_else(|| if state.ready { "Synced".into() } else { "Syncing…".into() }, ToString::to_string),
            pause_kind,
        }
    }
    /// Wait for a complete applied snapshot, bounded so offline startup remains usable.
    #[cfg(test)]
    pub fn wait_snapshot(&self, timeout: Duration) -> bool {
        let ready = pop_core::lock(&self.ready.0);
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

struct FeedSession {
    cancel: CancellationToken,
    wake: Arc<Notify>,
    ready: Arc<(Mutex<FeedState>, Condvar)>,
    notice: Notice,
}
async fn run(
    origin: String,
    mut identity: watch::Receiver<String>,
    send: mpsc::Sender<(CancellationToken, LibraryUpdate)>,
    session: FeedSession,
) {
    let FeedSession { cancel, wake, ready, notice } = session;
    let mut supervisor = Supervisor::new();
    loop {
        if cancel.is_cancelled() {
            break;
        }
        supervisor.step(Event::Connect);
        let epoch = cancel.child_token();
        let _attempt = epoch.clone().drop_guard();
        pop_core::lock(&ready.0).ready = false;
        crate::library::changes::notify(&notice);
        let token = identity.borrow_and_update().clone();
        let session = async {
            let mut socket = upgrade_at(&origin, Endpoint::Library, &token).await?;
            let mut snapshot = Some(Vec::new());
            let mut ping = tokio::time::interval(Duration::from_millis(pop_sharing_protocol::limits::PING_MS));
            loop {
                let message = tokio::select! {
                    _ = ping.tick() => { socket.send(Message::Text("ping".into())).await.map_err(|e| Error::caused(PauseKind::Failed, "Library heartbeat failed".into(), e))?; continue; },
                    message = socket.next() => message,
                };
                let Some(Ok(message)) = message else {
                    return Err::<(), _>(Error::from("Library disconnected"));
                };
                let bytes = match message {
                    Message::Text(text) if text == "pong" => continue,
                    Message::Text(text) => text.as_bytes().to_vec(),
                    Message::Ping(bytes) => {
                        socket
                            .send(Message::Pong(bytes))
                            .await
                            .map_err(|e| Error::caused(PauseKind::Failed, "Library heartbeat failed".into(), e))?;
                        continue;
                    }
                    Message::Pong(_) => continue,
                    Message::Close(Some(close)) if u16::from(close.code) == 4401 => {
                        return Err(Error::from(pop_core::sync::Pause {
                            kind: PauseKind::Refused,
                            reason: "Refresh your account identity".into(),
                        }));
                    }
                    Message::Close(Some(close)) if u16::from(close.code) == 1008 => {
                        return Err(Error::new(PauseKind::Ended, PauseKind::Ended.message(410)));
                    }
                    _ => return Err(Error::from("Invalid library message")),
                };
                let frame =
                    LibraryFrame::decode(&bytes).map_err(|e| Error::caused(PauseKind::Failed, e.to_string(), e))?;
                let update = match frame {
                    LibraryFrame::Snapshot { entries, last } => {
                        let pending = snapshot.as_mut().ok_or_else(|| Error::from("Unexpected library snapshot"))?;
                        pending.extend(entries);
                        if !last {
                            continue;
                        }
                        LibraryUpdate::Snapshot(snapshot.take().unwrap_or_default())
                    }
                    LibraryFrame::Upsert { entry } if snapshot.is_none() => LibraryUpdate::Upsert(entry),
                    LibraryFrame::Remove { document } if snapshot.is_none() => LibraryUpdate::Remove(document),
                    _ => return Err(Error::from("Library change interrupted snapshot")),
                };
                send.try_send((epoch.clone(), update)).map_err(|_| Error::from("Library delivery overflow"))?;
            }
        };
        let outcome = tokio::select! {
            _ = cancel.cancelled() => break,
            _ = identity.changed() => continue,
            _ = wake.notified() => {
                let failure = {
                    let mut state = pop_core::lock(&ready.0);
                    std::mem::take(&mut state.delivery_failed).then(|| state.error.clone().unwrap_or_else(|| Error::from("Library update could not be applied")))
                };
                if let Some(error) = failure { Err(error) } else { continue; }
            },
            outcome = session => outcome,
        };
        if pop_core::lock(&ready.0).ready {
            supervisor.step(Event::Live);
        }
        if let Err(error) = &outcome {
            let message = match error {
                crate::Error::Sharing { kind: PauseKind::Ended, .. } => "The cloud account was deleted".into(),
                crate::Error::Sharing { kind: PauseKind::Conflict, .. } => "The cloud library is busy".into(),
                error => error.to_string(),
            };
            let mut failure = error.clone();
            if let Error::Sharing { reason, .. } = &mut failure {
                *reason = message;
            }
            {
                let mut state = pop_core::lock(&ready.0);
                state.error = Some(failure);
                state.ready = false;
            }
            crate::library::changes::notify(&notice);
        }
        if matches!(outcome, Err(ref error) if error.pause_kind().pauses()) {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = identity.changed() => continue,
                _ = wake.notified(), if !matches!(outcome, Err(crate::Error::Sharing { kind: PauseKind::Ended, .. })) => continue,
            }
        }
        let Action::RetryAfter { milliseconds } = supervisor.step(Event::Retry) else {
            break;
        };
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = identity.changed() => {},
            _ = wake.notified() => {},
            _ = tokio::time::sleep(Duration::from_millis(milliseconds)) => {},
        }
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
        tungstenite::accept_hdr(stream, |request: &Request, response: Response| {
            assert_eq!(request.uri().path(), "/v1/account/library");
            assert!(!request.headers().contains_key("sec-websocket-protocol"));
            Ok(response)
        })
        .unwrap()
    }

    #[test]
    fn failed_snapshot_application_waits_before_reconnecting() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut first = accept(&listener);
            let start = std::time::Instant::now();
            first.send(Message::Text(r#"{"type":"snapshot","entries":[],"last":true}"#.into())).unwrap();
            let second = accept(&listener);
            let elapsed = start.elapsed();
            drop(second);
            elapsed
        });
        let account =
            Account::start_at(&origin, "fixture-token", Arc::new(|_| Err("Recovery failed".into())), Arc::default())
                .unwrap();
        let elapsed = server.join().unwrap();
        account.stop();
        assert!(elapsed >= Duration::from_millis(180), "reconnected after {elapsed:?}");
    }

    #[test]
    fn incomplete_snapshot_is_discarded_on_reconnect_and_stop_fences_delivery() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (seen, received) = std::sync::mpsc::channel();
        let (finish, finished) = std::sync::mpsc::channel();
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
            Arc::default(),
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
