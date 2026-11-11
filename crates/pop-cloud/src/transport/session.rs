//! Event-driven native driver with cancellable async effects and bounded socket queues.
use super::*;
use crate::library::receipts;
use pop_contracts::relay::PauseKind;
use pop_contracts::relay::SyncPhase;
use pop_contracts::relay::frame::ErrorCode;
use pop_runtime::owner::ReceivedFrame;
use pop_sharing_protocol::supervisor::budget::{BudgetDecision, PushBudget};
use pop_sharing_protocol::supervisor::maintenance::{FullState, MaintenanceAction, Step};
use pop_sharing_protocol::supervisor::{Action, Event as SupervisorEvent, Schedule, Supervisor, Wake, maintenance};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(test)]
use std::sync::mpsc;
use tokio::sync::{Notify, oneshot, watch};

type Identity = watch::Receiver<Option<String>>;
type Live = Arc<Mutex<LiveState>>;
#[derive(Default)]
struct LiveState {
    stopped: bool,
    connection: Option<SyncConnection>,
}
fn stopped() -> crate::Error {
    Error::new(PauseKind::Failed, "Sharing stopped".into())
}
async fn cancelled<T>(
    cancel: &CancellationToken,
    work: impl Future<Output = TransportResult<T>>,
) -> TransportResult<T> {
    tokio::select! { biased;
        _ = cancel.cancelled() => Err(stopped()),
        result = work => result,
    }
}
#[derive(Clone)]
struct Operations {
    owner: Arc<Owner>,
    key: RoomKey,
    receipt: receipts::Receipt,
    cancel: CancellationToken,
    identity: Identity,
    outcome: OwnerOutcome,
}
impl Operations {
    fn current_key(&self) -> RoomKey {
        let mut key = self.key.clone();
        key.identity_updates = Some(self.identity.clone());
        key
    }
    async fn call<T, Fut: Future<Output = Result<T>>>(&self, run: impl FnOnce(Self) -> Fut) -> Result<T> {
        let mut current = self.clone();
        current.key = self.current_key();
        cancelled(&self.cancel, run(current)).await
    }
    async fn version(&self) -> Result<pop_runtime::owner::Version> {
        cancelled(&self.cancel, owner_request(&self.owner, &self.outcome, Owner::version, OwnerEffect::Other)).await
    }
}

pub(crate) struct Connection {
    cancellation: CancellationToken,
    identity: watch::Sender<Option<String>>,
    live: Live,
    status: Arc<Status>,
    running: Arc<AtomicBool>,
}
struct Running(Arc<AtomicBool>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl Connection {
    pub fn start(owner: Arc<Owner>, key: RoomKey, notice: Notice, receipt: receipts::Receipt) -> Self {
        let cancellation = CancellationToken::new();
        let status = Arc::new(Status {
            value: Mutex::new((SyncPhase::Offline, "Connecting".into(), None)),
            notice,
            cancellation: cancellation.clone(),
        });
        let initial = Some(key.bearer().to_owned());
        let (identity, receive) = watch::channel(initial);
        let live = Live::default();
        let running = Arc::new(AtomicBool::new(false));
        match network() {
            Ok(network) => {
                let operations = Operations {
                    owner,
                    key,
                    receipt,
                    cancel: cancellation.clone(),
                    identity: receive,
                    outcome: OwnerOutcome::default(),
                };
                let output = status.clone();
                let attached = live.clone();
                running.store(true, Ordering::Release);
                let active = running.clone();
                network.runtime.spawn(async move {
                    let _running = Running(active);
                    supervise(operations, output, attached).await;
                });
            }
            Err(error) => status.paused(PauseKind::Refused, error.to_string()),
        }
        Self { cancellation, identity, live, status, running }
    }
    pub fn status(&self) -> (SyncPhase, String, Option<PauseKind>) {
        self.status.value.lock().unwrap().clone()
    }
    pub fn is_running(&self) -> bool {
        !self.cancellation.is_cancelled() && self.running.load(Ordering::Acquire)
    }
    pub fn set_identity(&self, identity: String) {
        self.identity.send_replace(Some(identity));
    }
    pub fn stop(self) {
        self.cancellation.cancel();
        // Fence the owner immediately, including when a bounded blocking effect has
        // not returned. Already accepted owner writes may still finish truthfully.
        {
            let mut live = self.live.lock().unwrap();
            live.stopped = true;
            if let Some(connection) = live.connection.take() {
                connection.detach();
            }
        }
        // Cleanup is asynchronous. The admission fence above is the stop guarantee;
        // an accepted owner write may still finish without holding up the caller.
    }
}

async fn supervise(operations: Operations, status: Arc<Status>, live: Live) {
    let mut supervisor = Supervisor::new();
    let mut retried_full = false;
    while !operations.cancel.is_cancelled() {
        supervisor.step(SupervisorEvent::Connect);
        let attempt = connected(&operations, &status, &live, &mut supervisor, &mut retried_full).await;
        if operations.cancel.is_cancelled() {
            break;
        }
        match attempt {
            Ok(true) => break,
            Ok(false) => status.set(SyncPhase::Offline, "Offline; reconnecting".into()),
            Err(crate::Error::Sharing { kind: PauseKind::Ended, .. }) => {
                status.set(SyncPhase::Ended, "Ended".into());
                break;
            }
            Err(pause) if pause.pause_kind().pauses() => {
                status.paused(pause.pause_kind(), pause.to_string());
                break;
            }
            Err(error) => status.set(SyncPhase::Offline, format!("Offline; reconnecting: {error}")),
        }
        let Action::RetryAfter { milliseconds } = supervisor.step(SupervisorEvent::Retry) else {
            break;
        };
        tokio::select! {
            _ = operations.cancel.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_millis(milliseconds)) => {},
        }
    }
    supervisor.step(SupervisorEvent::Stop);
    trace("supervisor_stopped");
}

#[derive(Default)]
struct Events {
    queue: Mutex<Queue>,
    ready: Notify,
}
impl Events {
    fn push(&self, event: SyncEvent) {
        self.queue.lock().unwrap().enqueue(event);
        self.ready.notify_one();
    }
    async fn next(&self) -> Queue {
        loop {
            let ready = self.ready.notified();
            {
                let mut queue = self.queue.lock().unwrap();
                if queue.overflow || !queue.events.is_empty() {
                    return std::mem::take(&mut *queue);
                }
            }
            ready.await;
        }
    }
}
struct Attached {
    connection: SyncConnection,
    live: Live,
}
impl Drop for Attached {
    fn drop(&mut self) {
        self.connection.detach();
        self.live.lock().unwrap().connection.take();
    }
}
fn milliseconds(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}
async fn send(socket: &mut AsyncSocket, cancel: &CancellationToken, message: Message) -> TransportResult<()> {
    cancelled(cancel, async {
        tokio::time::timeout(Duration::from_secs(10), socket.send(message))
            .await
            .map_err(|e| Error::caused(PauseKind::Failed, "Socket send timed out".into(), e))?
            .map_err(|e| Error::caused(PauseKind::Failed, "Socket send failed".into(), e))
    })
    .await
}
fn full_state(waiting: bool, retried: bool) -> FullState {
    match (waiting, retried) {
        (true, true) => FullState::Retried,
        (true, false) => FullState::Waiting,
        _ => FullState::Available,
    }
}
/// Folding cannot make the room fit: a terminal pause; local edits keep saving.
fn outgrown() -> crate::Error {
    Error::from(pop_core::sync::Pause { kind: PauseKind::Outgrown, reason: PauseKind::Outgrown.message(413) })
}

struct AttemptCancellation(CancellationToken);
impl Drop for AttemptCancellation {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// While an ordered owner/file/HTTP effect runs, only transport controls can bypass
/// it. Document frames stay bounded and ordered until resource preparation finishes.
enum Incoming {
    Frame(ReceivedFrame),
    Control(Message),
}
impl Incoming {
    fn decode(message: Message) -> Result<Self> {
        match message {
            Message::Binary(bytes) => {
                let received = ReceivedFrame::decode(bytes.to_vec());
                received
                    .frame()
                    .map_err(|error| Error::caused(PauseKind::Refused, error.to_string(), error.clone()))?;
                Ok(Self::Frame(received))
            }
            control => Ok(Self::Control(control)),
        }
    }
}
struct SocketPump {
    socket: AsyncSocket,
    cancel: CancellationToken,
    events: Arc<Events>,
    incoming: VecDeque<ReceivedFrame>,
    incoming_bytes: usize,
    heartbeat: tokio::time::Instant,
}
impl SocketPump {
    fn new(socket: AsyncSocket, cancel: CancellationToken, events: Arc<Events>) -> Self {
        Self {
            socket,
            cancel,
            events,
            incoming: VecDeque::new(),
            incoming_bytes: 0,
            heartbeat: tokio::time::Instant::now() + Duration::from_millis(pop_sharing_protocol::limits::PING_MS),
        }
    }
    async fn send(&mut self, message: Message) -> TransportResult<()> {
        let heartbeat = matches!(&message, Message::Text(text) if text.as_str() == "ping");
        send(&mut self.socket, &self.cancel, message).await?;
        if heartbeat {
            self.heartbeat = tokio::time::Instant::now() + Duration::from_millis(pop_sharing_protocol::limits::PING_MS);
        }
        Ok(())
    }
    async fn next(&mut self) -> Option<Result<Incoming>> {
        if let Some(frame) = self.incoming.pop_front() {
            self.incoming_bytes -= frame.bytes().len();
            return Some(Ok(Incoming::Frame(frame)));
        }
        self.socket.next().await.map(|result| {
            result
                .map_err(|e| Error::caused(PauseKind::Failed, "Relay socket read failed".into(), e))
                .and_then(Incoming::decode)
        })
    }
    fn interrupt(&self, reason: &'static str) -> crate::Error {
        self.cancel.cancel();
        Error::new(PauseKind::Failed, reason.into())
    }
    async fn effect<T>(&mut self, work: impl Future<Output = TransportResult<T>>) -> TransportResult<T> {
        self.effect_until(work, false).await
    }
    /// An ordering probe must settle even if the socket closes immediately after a
    /// frame. Dropping it would make a normal disconnect an unknown owner outcome.
    async fn owner_effect<T>(&mut self, work: impl Future<Output = TransportResult<T>>) -> TransportResult<T> {
        self.effect_until(work, true).await
    }
    async fn effect_until<T>(
        &mut self,
        work: impl Future<Output = TransportResult<T>>,
        settle_owner: bool,
    ) -> TransportResult<T> {
        let mut closed = false;
        tokio::pin!(work);
        loop {
            enum Input<T> {
                Result(TransportResult<T>),
                Socket(Option<std::result::Result<Message, tungstenite::Error>>),
                Heartbeat,
                Owner,
            }
            let input = tokio::select! { biased;
                _ = self.cancel.cancelled() => return Err(stopped()),
                result = &mut work => Input::Result(result),
                message = self.socket.next(), if !closed => Input::Socket(message),
                _ = self.events.ready.notified() => Input::Owner,
                _ = tokio::time::sleep_until(self.heartbeat), if !closed => Input::Heartbeat,
            };
            match input {
                Input::Result(result) => {
                    return if closed {
                        result.and_then(|_| Err(self.interrupt("Relay socket closed")))
                    } else {
                        result
                    };
                }
                Input::Heartbeat => self.send(Message::Text("ping".into())).await?,
                Input::Owner => {
                    if self.events.queue.lock().unwrap().overflow {
                        return Err(self.interrupt("Owner socket queue budget exceeded"));
                    }
                }
                Input::Socket(Some(Ok(Message::Ping(bytes)))) => self.send(Message::Pong(bytes)).await?,
                Input::Socket(Some(Ok(Message::Binary(bytes)))) => {
                    let received = ReceivedFrame::decode(bytes.to_vec());
                    let frame = match received.frame() {
                        Ok(frame) => frame,
                        Err(error) => {
                            self.cancel.cancel();
                            return Err(Error::caused(PauseKind::Refused, error.to_string(), error.clone()));
                        }
                    };
                    if let Frame::Error { code, message } = frame {
                        self.cancel.cancel();
                        return Err(match ErrorCode::try_from(*code) {
                            Ok(ErrorCode::Ended) => Error::new(PauseKind::Ended, PauseKind::Ended.message(410)),
                            Ok(ErrorCode::Protocol) => Error::from(pop_core::sync::Pause {
                                kind: PauseKind::NeedsUpdate,
                                reason: message.clone(),
                            }),
                            Ok(ErrorCode::App | ErrorCode::Malformed) | Err(_) => {
                                Error::from(pop_core::sync::Pause { kind: PauseKind::Refused, reason: message.clone() })
                            }
                        });
                    }
                    if self.incoming.len() >= pop_contracts::relay::limits::QUEUED_FRAMES
                        || self.incoming_bytes.saturating_add(bytes.len()) > pop_contracts::relay::limits::QUEUED_BYTES
                    {
                        return Err(self.interrupt("Deferred socket queue budget exceeded"));
                    }
                    self.incoming_bytes += bytes.len();
                    self.incoming.push_back(received);
                }
                Input::Socket(Some(Ok(Message::Close(_)))) | Input::Socket(None) => {
                    if settle_owner {
                        closed = true;
                    } else {
                        return Err(self.interrupt("Relay socket closed"));
                    }
                }
                Input::Socket(Some(Err(_))) => {
                    if settle_owner {
                        closed = true;
                    } else {
                        return Err(self.interrupt("Relay socket read failed"));
                    }
                }
                Input::Socket(Some(Ok(_))) => {}
            }
        }
    }
}

async fn connected(
    operations: &Operations,
    status: &Status,
    live: &Live,
    supervisor: &mut Supervisor,
    retried_full: &mut bool,
) -> TransportResult<bool> {
    let mut attempt = operations.clone();
    attempt.cancel = operations.cancel.child_token();
    attempt.outcome = OwnerOutcome::default();
    let cancel = attempt.cancel.clone();
    let _scope = AttemptCancellation(cancel.clone());
    let result = connected_attempt(&attempt, status, live, supervisor, retried_full).await;
    if attempt.outcome.unknown() {
        return Err(unconfirmed_owner());
    }
    match result {
        Err(crate::Error::Sharing { kind: PauseKind::Failed, .. })
            if cancel.is_cancelled() && !operations.cancel.is_cancelled() =>
        {
            Ok(false)
        }
        result => result,
    }
}

async fn connected_attempt(
    operations: &Operations,
    status: &Status,
    live: &Live,
    supervisor: &mut Supervisor,
    retried_full: &mut bool,
) -> TransportResult<bool> {
    let socket = match cancelled(&operations.cancel, upgrade_async(&operations.current_key())).await {
        Ok(socket) => socket,
        Err(error) => {
            match error {
                crate::Error::Sharing { kind: PauseKind::Ended, .. } => status.set(SyncPhase::Ended, "Ended".into()),
                pause if pause.pause_kind().pauses() => status.paused(pause.pause_kind(), pause.to_string()),
                error => return Err(error),
            }
            return Ok(true);
        }
    };
    trace("socket_opened");
    let events = Arc::new(Events::default());
    let sink = events.clone();
    let mut pump = SocketPump::new(socket, operations.cancel.clone(), events.clone());
    let admission = {
        let mut live = live.lock().unwrap();
        if live.stopped || operations.cancel.is_cancelled() {
            return Err(stopped());
        }
        let admission = operations.owner.sync_connection(Arc::new(move |event| sink.push(event)));
        live.connection = Some(admission.clone());
        admission
    };
    let attached = Attached { connection: admission.clone(), live: live.clone() };
    if operations.cancel.is_cancelled() {
        return Err(stopped());
    }
    pump.owner_effect(operations.version()).await?;
    trace("owner_attached");
    let mut uploaded = pump
        .effect(operations.call(|operation| async move {
            trace("inventory_start");
            let mut uploaded = Vec::new();
            attachments::download_blobs(&operation.owner, &operation.outcome, &operation.key, &mut uploaded).await?;
            trace("inventory_done");
            Ok(uploaded)
        }))
        .await?;
    let start = Instant::now();
    let mut schedule = Schedule::new(0);
    let mut core_status = (SyncPhase::Syncing, "Syncing".to_owned());
    let mut waiting = false;
    let mut push_budget = PushBudget::new(0);
    let mut delayed_push: Option<(u64, Vec<u8>)> = None;
    let mut sent_push: Option<u64> = None;
    let mut hello_sent = false;
    let mut identity = operations.identity.clone();
    let result = async {
        loop {
            if operations.cancel.is_cancelled() { return Err(stopped()); }
            if tokio::time::Instant::now() >= pump.heartbeat { pump.send(Message::Text("ping".into())).await?; }
            let now = milliseconds(start);
            if delayed_push.as_ref().is_some_and(|(at, _)| *at <= now) {
                let (_, bytes) = delayed_push.take().expect("due push checked above");
                events.push(SyncEvent::Send { bytes });
            }
            if let Some(wake) = schedule.due(now) {
                schedule.completed(wake, now);
                match wake {
                    Wake::Probe => {
                        let info = pump.effect(operations.call(|operation| async move { room_info(&operation.key).await })).await?;
                        let full = full_state(waiting, *retried_full);
                        let mut random = [0u8; 2];
                        pop_core::random(&mut random).map_err(|_| Error::from("Cannot schedule compaction"))?;
                        match maintenance::decide(full, Step::Probe { compact: info.compact }, u16::from_le_bytes(random)) {
                            MaintenanceAction::Outgrown => return Err(outgrown()),
                            MaintenanceAction::Jitter { milliseconds } => schedule.compact_after(now, milliseconds),
                            _ => {},
                        }
                    }
                    Wake::Compact => {
                        let info = pump.effect(operations.call(|operation| async move { room_info(&operation.key).await })).await?;
                        let full = full_state(waiting, *retried_full);
                        let action = maintenance::decide(full, Step::Recheck { compact: info.compact }, 0);
                        if action == MaintenanceAction::Reconnect { *retried_full = true; return Ok(false); }
                        if action == MaintenanceAction::ReadBase {
                            let (send, receive) = oneshot::channel();
                            admission.maintenance_base(move |base| { let _ = send.send(base); });
                            let base = pump.effect(cancelled(&operations.cancel, async {
                                completion(&operations.outcome, receive, Duration::from_secs(20)).await
                            })).await?;
                            let base = match base {
                                Err(pause) => return Err(Error::from(pause)),
                                Ok(pop_core::sync::BaseRead::Pending) => {
                                    if let MaintenanceAction::RetryBase { milliseconds } = maintenance::decide(full, Step::BaseUnavailable, 0) {
                                        schedule.compact_after(now, milliseconds);
                                    }
                                    continue;
                                }
                                Ok(pop_core::sync::BaseRead::TooLarge) => {
                                    if maintenance::decide(full, Step::BaseTooLarge, 0) == MaintenanceAction::Outgrown {
                                        return Err(outgrown());
                                    }
                                    continue;
                                }
                                Ok(pop_core::sync::BaseRead::Ready(base)) => base,
                            };
                            // A 413 is the outgrown pause; a competing fold reconnects to catch up.
                            match pump.effect(operations.call(move |operation| async move { compact(&operation.key, base).await })).await {
                                Ok(()) => {
                                    if maintenance::decide(full, Step::Compacted, 0) == MaintenanceAction::Reconnect {
                                        *retried_full = true; return Ok(false);
                                    }
                                }
                                Err(crate::Error::Sharing { kind: PauseKind::Conflict, .. }) => return Ok(false),
                                Err(error) => return Err(error),
                            }
                        }
                    }
                }
                continue;
            }
            enum Input { Owner(Queue), Socket(Option<Result<Incoming>>), Timer, Identity }
            let heartbeat = pump.heartbeat;
            let input = tokio::select! { biased;
                _ = operations.cancel.cancelled() => return Err(stopped()),
                pending = events.next() => Input::Owner(pending),
                message = pump.next() => Input::Socket(message),
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(start + Duration::from_millis(schedule.next_at_ms().min(delayed_push.as_ref().map_or(u64::MAX, |(at, _)| *at)))).min(heartbeat)) => Input::Timer,
                changed = identity.changed() => { if changed.is_err() { return Err(stopped()); } Input::Identity },
            };
            match input {
                Input::Timer | Input::Identity => {},
                Input::Owner(pending) => {
                    if pending.overflow { return Err(Error::from("Socket queue budget exceeded")); }
                    for event in pending.events {
                        match event {
                            SyncEvent::Reconnect => return Ok(false),
                            SyncEvent::Status { status: reported } => {
                                if let pop_core::sync::Status::Paused { kind, reason } = &reported {
                                    status.paused(*kind, reason.clone());
                                    if *kind == PauseKind::RoomFull {
                                        waiting = true;
                                        schedule.probe_now(milliseconds(start)); continue;
                                    }
                                    return Ok(true);
                                }
                                if reported == pop_core::sync::Status::Synced {
                                    supervisor.step(SupervisorEvent::Live);
                                    let connection = admission.clone();
                                    let confirmed = pump.owner_effect(operations.call(move |operation| async move {
                                        receipts::confirm(&operation.owner, &operation.outcome, &connection, operation.receipt).await
                                    })).await?;
                                    if confirmed { *retried_full = false; waiting = false; }
                                }
                                let phase = reported.state().phase;
                                core_status = (phase, format!("{reported:?}"));
                                if !waiting { status.set(core_status.0, core_status.1.clone()); }
                            }
                            SyncEvent::Send { bytes } => {
                                let frame = Frame::decode(&bytes).map_err(Error::from)?;
                                let push = if let Frame::Push { id, .. } = &frame { Some(*id) } else { None };
                                if matches!(frame, Frame::Hello { .. }) {
                                    if hello_sent { return Ok(false); }
                                    hello_sent = true;
                                    delayed_push = None;
                                    sent_push = None;
                                }
                                trace_frame(&frame, false);
                                if let Some(id) = push {
                                    match push_budget.admit(milliseconds(start), bytes.len()) {
                                        BudgetDecision::Wait { milliseconds: wait } => {
                                            if delayed_push.is_some() { return Err(Error::from("Multiple unsent pushes")); }
                                            delayed_push = Some((milliseconds(start).saturating_add(wait), bytes));
                                            continue;
                                        }
                                        BudgetDecision::TooLarge => return Err(outgrown()),
                                        BudgetDecision::Ready => sent_push = Some(id),
                                    }
                                    status.set(SyncPhase::Syncing, "Syncing files".into());
                                    let result = pump.effect(operations.call(move |operation| async move {
                                        let result = attachments::upload_blobs(&operation.owner, &operation.outcome, &operation.key, &mut uploaded).await;
                                        Ok((uploaded, result))
                                    })).await?;
                                    uploaded = result.0;
                                    // Folding never frees attachment space: FilesFull is terminal.
                                    result.1?;
                                }
                                pump.send(Message::Binary(bytes.into())).await?;
                            }
                        }
                    }
                }
                Input::Socket(Some(Ok(Incoming::Frame(received)))) => {
                    let frame = received.frame().map_err(|error| Error::caused(PauseKind::Refused, error.to_string(), error.clone()))?;
                    trace_frame(frame, true);
                    if let Frame::Ack { id, .. } = &frame
                        && sent_push == Some(*id)
                    {
                        push_budget.acknowledge(milliseconds(start));
                        sent_push = None;
                    }
                    if matches!(&frame, Frame::Error { code, .. } if ErrorCode::try_from(*code) == Ok(ErrorCode::Ended)) {
                        status.set(SyncPhase::Ended, "Ended".into()); return Ok(true);
                    }
                    if matches!(frame, Frame::Records(_)) {
                        status.set(SyncPhase::Syncing, "Receiving files".into());
                        uploaded = pump.effect(operations.call(move |operation| async move {
                            attachments::download_blobs(&operation.owner, &operation.outcome, &operation.key, &mut uploaded).await?;
                            Ok(uploaded)
                        })).await?;
                    }
                    if !admission.frame(received) { return Ok(false); }
                    pump.owner_effect(operations.version()).await?;
                    if !waiting { status.set(core_status.0, core_status.1.clone()); }
                }
                Input::Socket(Some(Ok(Incoming::Control(Message::Close(_))))) | Input::Socket(Some(Err(_))) | Input::Socket(None) => return Ok(false),
                Input::Socket(Some(Ok(Incoming::Control(Message::Ping(bytes))))) => pump.send(Message::Pong(bytes)).await?,
                Input::Socket(Some(Ok(_))) => {},
            }
        }
    }.await;
    // Cancellation has already fenced owner input. Closing is bounded and never holds
    // a native/client mutex, even when a notice stops this task re-entrantly.
    drop(attached);
    let _ = tokio::time::timeout(Duration::from_millis(100), pump.socket.close(None)).await;
    trace("socket_detached");
    match result {
        Err(crate::Error::Sharing { kind: PauseKind::Ended, .. }) => {
            status.set(SyncPhase::Ended, "Ended".into());
            Ok(true)
        }
        Err(pause) if pause.pause_kind().pauses() => {
            status.paused(pause.pause_kind(), pause.to_string());
            Ok(true)
        }
        result => result,
    }
}

#[cfg(test)]
pub(super) fn connected_fixture(
    owner: &Arc<Owner>,
    key: &RoomKey,
    status: &Status,
    retried_full: &mut bool,
    receipt: &receipts::Receipt,
) -> TransportResult<bool> {
    let output = Arc::new(Status {
        value: Mutex::new(status.value.lock().unwrap().clone()),
        notice: status.notice.clone(),
        cancellation: CancellationToken::new(),
    });
    let current = output.clone();
    let initial = Some(key.bearer().to_owned());
    let (_identity, receive) = watch::channel(initial);
    let operation = Operations {
        owner: owner.clone(),
        key: key.clone(),
        receipt: receipt.clone(),
        cancel: CancellationToken::new(),
        identity: receive,
        outcome: OwnerOutcome::default(),
    };
    let mut retried = *retried_full;
    let result = block_on(async move {
        let result = connected(&operation, &output, &Live::default(), &mut Supervisor::new(), &mut retried).await;
        Ok((result, retried))
    })?;
    *status.value.lock().unwrap() = current.value.lock().unwrap().clone();
    *retried_full = result.1;
    result.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use pop_storage::store::Share;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn gap_reconnects_before_sending_another_hello() {
        use pop_contracts::relay::frame::{Ack, Permission};
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let (_directory, owner) = crate::client::tests::fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: owner.app().document_uuid.clone().unwrap(),
            identity: "fixture-identity".into(),
            identity_updates: None,
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        request(&owner, |owner: &Owner, options, callback| {
            owner.set_share(Share { endpoint: key.origin.clone() }, options, callback)
        })
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let info = serde_json::to_vec(&pop_sharing_protocol::http::RoomInfo {
            document: key.room.clone(),
            phase: pop_sharing_protocol::http::RoomPhase::Live,
            seed_hash: None,
            seed_size: None,
            app: None,
            compact: false,
        })
        .unwrap();
        let editing_owner = owner.clone();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            listener.set_nonblocking(true).unwrap();
            let http = std::thread::spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                    let mut header = Vec::new();
                    while !header.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        stream.read_exact(&mut byte).unwrap();
                        header.push(byte[0]);
                    }
                    let header = String::from_utf8(header).unwrap();
                    let size = header
                        .lines()
                        .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned))
                        .unwrap_or_else(|| "0".into())
                        .parse::<usize>()
                        .unwrap();
                    stream.read_exact(&mut vec![0; size]).unwrap();
                    let body =
                        if header.starts_with("GET ") && !header.contains("/blobs ") { info.as_slice() } else { b"[]" };
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                        .unwrap();
                    stream.write_all(body).unwrap();
                }
            });
            let read_frame = |socket: &mut tungstenite::WebSocket<std::net::TcpStream>| loop {
                match socket.read().unwrap() {
                    Message::Binary(bytes) => break envelope::decode(&bytes).unwrap(),
                    Message::Text(_) | Message::Pong(_) => {}
                    other => panic!("expected relay frame, got {other:?}"),
                }
            };
            let Frame::Hello { version, .. } = read_frame(&mut socket) else {
                panic!("connection starts with Hello");
            };
            request(&editing_owner, |owner: &Owner, options, callback| {
                owner.apply(
                    serde_json::from_value(
                        serde_json::json!({"intents":[{"type":"increment","path":["hits"],"by":1}]}),
                    )
                    .unwrap(),
                    pop_core::Origin::Page,
                    options,
                    callback,
                )
            })
            .unwrap();
            socket
                .send(Message::Binary(
                    envelope::encode(Frame::Welcome { permission: Permission::Write, version }).unwrap().into(),
                ))
                .unwrap();
            let Frame::Push { id, .. } = read_frame(&mut socket) else {
                panic!("Welcome must produce a push");
            };
            socket
                .send(Message::Binary(envelope::encode(Frame::Ack { id, status: Ack::Gap }).unwrap().into()))
                .unwrap();
            let next = loop {
                let next = socket.read();
                if matches!(next, Ok(Message::Text(_) | Message::Pong(_))) {
                    continue;
                }
                break next;
            };
            (next, http)
        });
        let status = Status {
            value: Mutex::new((SyncPhase::Offline, String::new(), None)),
            notice: Arc::default(),
            cancellation: CancellationToken::new(),
        };
        let result = connected_fixture(
            &owner,
            &key,
            &status,
            &mut false,
            &crate::client::tests::test_receipt(&owner, &key, true),
        );
        stop.store(true, Ordering::Release);
        let (next, http) = server.join().unwrap();
        http.join().unwrap();
        request(&owner, |owner: &Owner, options, callback| owner.close(None, None, options, callback)).unwrap();
        assert!(matches!(result, Ok(false)), "a gap starts a new connection: {result:?}");
        assert!(matches!(next, Ok(Message::Close(_))), "the old socket must close without a second Hello: {next:?}");
    }

    #[test]
    fn cancellation_returns_while_a_started_effect_finishes_without_late_notices() {
        let cancellation = CancellationToken::new();
        let cancel_effect = cancellation.clone();
        let notices = Arc::new(AtomicUsize::new(0));
        let counted = notices.clone();
        let status = Arc::new(Status {
            value: Mutex::new((SyncPhase::Offline, "Connecting".into(), None)),
            notice: Arc::new(Mutex::new(Some(Arc::new(move || {
                counted.fetch_add(1, Ordering::SeqCst);
            })))),
            cancellation: cancellation.clone(),
        });
        let (entered, started) = mpsc::channel();
        let (release, finish) = mpsc::channel();
        let (finished, completed) = mpsc::channel();
        let caller = std::thread::spawn(move || {
            block_on(async move {
                cancelled(
                    &cancel_effect,
                    blocking(move || {
                        entered.send(()).unwrap();
                        finish.recv().unwrap();
                        status.set(SyncPhase::Synced, "Late result".into());
                        finished.send(()).unwrap();
                        Ok(())
                    }),
                )
                .await
            })
        });
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        cancellation.cancel();
        let result = caller.join().unwrap();
        // Release the already-started effect even if the cancellation assertion fails.
        release.send(()).unwrap();
        completed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(result.is_err());
        assert_eq!(notices.load(Ordering::SeqCst), 0);
    }

    #[test]
    #[expect(
        clippy::result_large_err,
        reason = "tungstenite fixes its handshake callback rejection to an HTTP response"
    )]
    fn slow_inventory_keeps_socket_controls_and_attempt_cancellation_live() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let (_directory, owner) = crate::client::tests::fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let key = RoomKey {
            room: owner.app().document_uuid.clone().unwrap(),
            identity: "fixture-identity".into(),
            identity_updates: None,
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        request(&owner, |owner: &Owner, options, callback| {
            owner.set_share(Share { endpoint: key.origin.clone() }, options, callback)
        })
        .unwrap();
        let receipt = crate::client::tests::test_receipt(&owner, &key, true);
        let route = format!("GET /v1/documents/{}/blobs ", key.room);
        let (observed, controls) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut socket = tungstenite::accept_hdr(
                stream,
                |request: &tungstenite::handshake::server::Request, reply: tungstenite::handshake::server::Response| {
                    assert!(!request.headers().contains_key("Sec-WebSocket-Protocol"));
                    Ok(reply)
                },
            )
            .unwrap();
            let (mut http, _) = listener.accept().unwrap();
            http.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                http.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            assert!(String::from_utf8(header).unwrap().starts_with(&route));
            let payload = b"while-inventory-is-held".to_vec();
            socket.send(Message::Ping(payload.clone().into())).unwrap();
            let pong = loop {
                match socket.read() {
                    Ok(Message::Pong(bytes)) => break bytes.as_ref() == payload.as_slice(),
                    Ok(Message::Text(_)) => {}
                    _ => break false,
                }
            };
            let _ = socket.close(None);
            let cancelled_http = matches!(http.read(&mut [0]), Ok(0));
            observed.send((pong, cancelled_http)).unwrap();
            released.recv().unwrap();
            // A cancelled client can already have closed this HTTP connection.
            let _ = http.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]");
        });
        let (reply, completed) = mpsc::channel();
        let cancel = CancellationToken::new();
        let (_identity, identity) = watch::channel(Some(key.bearer().to_owned()));
        let operation = Operations {
            owner: owner.clone(),
            key,
            receipt,
            cancel: cancel.clone(),
            identity,
            outcome: OwnerOutcome::default(),
        };
        network().unwrap().runtime.spawn(async move {
            let status = Status {
                value: Mutex::new((SyncPhase::Offline, String::new(), None)),
                notice: Arc::default(),
                cancellation: cancel,
            };
            let result = connected(&operation, &status, &Live::default(), &mut Supervisor::new(), &mut false).await;
            let _ = reply.send(result);
        });
        let (pong, cancelled_http) = controls.recv_timeout(Duration::from_secs(8)).unwrap();
        let early = completed.recv_timeout(Duration::from_secs(2));
        // Always release and join the server before asserting the regression oracle.
        release.send(()).unwrap();
        server.join().unwrap();
        if early.is_err() {
            let _ = completed.recv_timeout(Duration::from_secs(5));
        }
        request(&owner, |owner: &Owner, options, callback| owner.close(None, None, options, callback)).unwrap();
        assert!(pong, "server Ping must receive Pong while inventory HTTP is still held");
        assert!(
            early.is_ok_and(|result| matches!(result, Ok(false))),
            "socket close must terminate the attempt before HTTP is released"
        );
        assert!(cancelled_http, "socket close must cancel its in-flight blocking HTTP effect");
    }
}
