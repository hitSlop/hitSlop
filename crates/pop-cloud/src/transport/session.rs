//! Event-driven native driver. Blocking effects have a separate bounded pool;
//! they never occupy the socket runtime or its DNS blocking threads.
use super::*;
use pop_contracts::relay::PauseKind;
use pop_contracts::relay::SyncPhase;
use pop_contracts::relay::frame::ErrorCode;
use pop_sharing_protocol::supervisor::budget::{BudgetDecision, PushBudget};
use pop_sharing_protocol::supervisor::maintenance::{FullState, MaintenanceAction, Step};
use pop_sharing_protocol::supervisor::{Action, Event as SupervisorEvent, Schedule, Supervisor, Wake, maintenance};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Notify, Semaphore, oneshot, watch};

type Identity = watch::Receiver<Option<String>>;
type Live = Arc<Mutex<LiveState>>;
#[derive(Default)]
struct LiveState {
    stopped: bool,
    connection: Option<SyncConnection>,
}
struct BlockingPool {
    runtime: tokio::runtime::Runtime,
    slots: Arc<Semaphore>,
}
fn blocking_pool() -> TransportResult<&'static BlockingPool> {
    static POOL: OnceLock<std::result::Result<BlockingPool, String>> = OnceLock::new();
    POOL.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(2)
            .thread_name("pop.sharing.effects")
            .build()
            .map_err(fail)?;
        Ok(BlockingPool { runtime, slots: Arc::new(Semaphore::new(2)) })
    })
    .as_ref()
    .map_err(|error| HttpFailure::new(PauseKind::Failed, error.clone()))
}
fn stopped() -> HttpFailure {
    HttpFailure::new(PauseKind::Failed, "Sharing stopped".into())
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
/// Runs a synchronous effect on the bounded blocking pool, cancelled with the socket.
async fn blocking<T: Send + 'static>(
    cancel: &CancellationToken,
    identity: Identity,
    run: impl FnOnce() -> TransportResult<T> + Send + 'static,
) -> TransportResult<T> {
    let pool = blocking_pool()?;
    let permit = cancelled(cancel, async { pool.slots.clone().acquire_owned().await.map_err(|_| stopped()) }).await?;
    let context = Operation { cancellation: cancel.clone(), identity };
    let task = pool.runtime.spawn_blocking(move || {
        let _permit = permit;
        scoped(context, run)
    });
    cancelled(cancel, async { task.await.map_err(|_| unconfirmed_owner())? }).await
}

#[derive(Clone)]
struct Operations {
    owner: Arc<Owner>,
    key: RoomKey,
    journal: Arc<journal::Journal>,
    cancel: CancellationToken,
    identity: Identity,
}
impl Operations {
    fn current_key(&self) -> RoomKey {
        let mut key = self.key.clone();
        if let Some(identity) = self.identity.borrow().as_ref() {
            key.identity = identity.clone();
        }
        key
    }
    async fn call<T: Send + 'static>(
        &self,
        run: impl FnOnce(Self) -> TransportResult<T> + Send + 'static,
    ) -> TransportResult<T> {
        let mut current = self.clone();
        current.key = self.current_key();
        blocking(&self.cancel, self.identity.clone(), move || run(current)).await
    }
    async fn request(&self, request: Request) -> TransportResult<Reply> {
        if self.cancel.is_cancelled() {
            return Err(stopped());
        }
        let effect = owner_effect(&request);
        let (send, reply) = oneshot::channel();
        self.owner.submit(
            request,
            None,
            Box::new(move |result| {
                let _ = send.send(result);
            }),
        );
        cancelled(&self.cancel, async {
            tokio::time::timeout(Duration::from_secs(20), reply)
                .await
                .map_err(|_| unconfirmed_owner())?
                .map_err(|_| unconfirmed_owner())?
                .map_err(|error| owner_failure(error, effect))
        })
        .await
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
    pub fn start(owner: Arc<Owner>, key: RoomKey, notice: Notice, journal: Arc<journal::Journal>) -> Self {
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
                let operations = Operations { owner, key, journal, cancel: cancellation.clone(), identity: receive };
                let output = status.clone();
                let attached = live.clone();
                running.store(true, Ordering::Release);
                let active = running.clone();
                network.runtime.spawn(async move {
                    let _running = Running(active);
                    supervise(operations, output, attached).await;
                });
            }
            Err(error) => status.paused(PauseKind::Refused, String::from(error)),
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
            Err(HttpFailure { kind: PauseKind::Ended, .. }) => {
                status.set(SyncPhase::Ended, "Ended".into());
                break;
            }
            Err(pause) if pause.kind.pauses() => {
                status.paused(pause.kind, pause.reason);
                break;
            }
            Err(error) => status.set(SyncPhase::Offline, format!("Offline; reconnecting: {}", String::from(error))),
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
            .map_err(|_| HttpFailure::from("Socket send timed out"))?
            .map_err(|_| HttpFailure::from("Socket send failed"))
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
fn outgrown() -> HttpFailure {
    HttpFailure::from(pop_core::sync::Pause { kind: PauseKind::Outgrown, reason: PauseKind::Outgrown.message(413) })
}

struct AttemptCancellation(CancellationToken);
impl Drop for AttemptCancellation {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// While an ordered owner/file/HTTP effect runs, only transport controls can bypass
/// it. Document frames stay bounded and ordered until resource preparation finishes.
struct SocketPump {
    socket: AsyncSocket,
    cancel: CancellationToken,
    events: Arc<Events>,
    incoming: VecDeque<Message>,
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
    async fn next(&mut self) -> Option<std::result::Result<Message, tungstenite::Error>> {
        if let Some(message) = self.incoming.pop_front() {
            if let Message::Binary(bytes) = &message {
                self.incoming_bytes -= bytes.len();
            }
            return Some(Ok(message));
        }
        self.socket.next().await
    }
    fn interrupt(&self, reason: &'static str) -> HttpFailure {
        self.cancel.cancel();
        HttpFailure::new(PauseKind::Failed, reason.into())
    }
    async fn effect<T>(&mut self, work: impl Future<Output = TransportResult<T>>) -> TransportResult<T> {
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
                message = self.socket.next() => Input::Socket(message),
                result = &mut work => Input::Result(result),
                _ = self.events.ready.notified() => Input::Owner,
                _ = tokio::time::sleep_until(self.heartbeat) => Input::Heartbeat,
            };
            match input {
                Input::Result(result) => return result,
                Input::Heartbeat => self.send(Message::Text("ping".into())).await?,
                Input::Owner => {
                    if self.events.queue.lock().unwrap().overflow {
                        return Err(self.interrupt("Owner socket queue budget exceeded"));
                    }
                }
                Input::Socket(Some(Ok(Message::Ping(bytes)))) => self.send(Message::Pong(bytes)).await?,
                Input::Socket(Some(Ok(Message::Binary(bytes)))) => {
                    let frame = match envelope::decode(&bytes) {
                        Ok(frame) => frame,
                        Err(error) => {
                            self.cancel.cancel();
                            return Err(HttpFailure::from(pop_core::sync::Pause {
                                kind: PauseKind::Refused,
                                reason: error.to_string(),
                            }));
                        }
                    };
                    if let Frame::Error { code, message } = frame {
                        self.cancel.cancel();
                        return Err(match ErrorCode::try_from(code) {
                            Ok(ErrorCode::Ended) => HttpFailure::new(PauseKind::Ended, PauseKind::Ended.message(410)),
                            Ok(ErrorCode::Protocol) => HttpFailure::from(pop_core::sync::Pause {
                                kind: PauseKind::NeedsUpdate,
                                reason: message,
                            }),
                            Ok(ErrorCode::App | ErrorCode::Malformed) | Err(_) => {
                                HttpFailure::from(pop_core::sync::Pause { kind: PauseKind::Refused, reason: message })
                            }
                        });
                    }
                    if self.incoming.len() >= pop_contracts::relay::limits::QUEUED_FRAMES
                        || self.incoming_bytes.saturating_add(bytes.len()) > pop_contracts::relay::limits::QUEUED_BYTES
                    {
                        return Err(self.interrupt("Deferred socket queue budget exceeded"));
                    }
                    self.incoming_bytes += bytes.len();
                    self.incoming.push_back(Message::Binary(bytes));
                }
                Input::Socket(Some(Ok(Message::Close(_)))) | Input::Socket(None) => {
                    return Err(self.interrupt("Relay socket closed"));
                }
                Input::Socket(Some(Err(_))) => {
                    return Err(self.interrupt("Relay socket read failed"));
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
    let cancel = attempt.cancel.clone();
    let _scope = AttemptCancellation(cancel.clone());
    let result = connected_attempt(&attempt, status, live, supervisor, retried_full).await;
    match result {
        Err(HttpFailure { kind: PauseKind::Failed, .. })
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
                HttpFailure { kind: PauseKind::Ended, .. } => status.set(SyncPhase::Ended, "Ended".into()),
                pause if pause.kind.pauses() => status.paused(pause.kind, pause.reason),
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
    pump.effect(operations.request(Request::State)).await?;
    trace("owner_attached");
    let mut uploaded = pump
        .effect(operations.call(|operation| {
            trace("inventory_start");
            let mut uploaded = Vec::new();
            attachments::download_blobs(&operation.owner, &operation.key, &mut uploaded)?;
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
    let mut identity = operations.identity.clone();
    let result = async {
        loop {
            if operations.cancel.is_cancelled() { return Err(stopped()); }
            let now = milliseconds(start);
            if delayed_push.as_ref().is_some_and(|(at, _)| *at <= now) {
                let (_, bytes) = delayed_push.take().expect("due push checked above");
                events.push(SyncEvent::Send { bytes });
            }
            if let Some(wake) = schedule.due(now) {
                schedule.completed(wake, now);
                match wake {
                    Wake::Ping => pump.send(Message::Text("ping".into())).await?,
                    Wake::Probe => {
                        let info = pump.effect(operations.call(|operation| room_info(&operation.key, None))).await?;
                        let full = full_state(waiting, *retried_full);
                        let mut random = [0u8; 2];
                        pop_core::random(&mut random).map_err(|_| HttpFailure::from("Cannot schedule compaction"))?;
                        match maintenance::decide(full, Step::Probe { compact: info.compact }, u16::from_le_bytes(random)) {
                            MaintenanceAction::Outgrown => return Err(outgrown()),
                            MaintenanceAction::Jitter { milliseconds } => schedule.compact_after(now, milliseconds),
                            _ => {},
                        }
                    }
                    Wake::Compact => {
                        let info = pump.effect(operations.call(|operation| room_info(&operation.key, None))).await?;
                        let full = full_state(waiting, *retried_full);
                        let action = maintenance::decide(full, Step::Recheck { compact: info.compact }, 0);
                        if action == MaintenanceAction::Reconnect { *retried_full = true; return Ok(false); }
                        if action == MaintenanceAction::ReadBase {
                            let (send, receive) = oneshot::channel();
                            admission.maintenance_base(move |base| { let _ = send.send(base); });
                            let base = pump.effect(cancelled(&operations.cancel, async {
                                tokio::time::timeout(Duration::from_secs(30), receive).await
                                    .map_err(|_| unconfirmed_owner())?.map_err(|_| unconfirmed_owner())
                            })).await?;
                            let base = match base {
                                Err(pause) => return Err(HttpFailure::from(pause)),
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
                            match pump.effect(operations.call(move |operation| compact(&operation.key, &base))).await {
                                Ok(()) => {
                                    if maintenance::decide(full, Step::Compacted, 0) == MaintenanceAction::Reconnect {
                                        *retried_full = true; return Ok(false);
                                    }
                                }
                                Err(HttpFailure { kind: PauseKind::Conflict, .. }) => return Ok(false),
                                Err(error) => return Err(error),
                            }
                        }
                    }
                }
                continue;
            }
            enum Input { Owner(Queue), Socket(Option<std::result::Result<Message, tungstenite::Error>>), Timer, Identity }
            let input = tokio::select! { biased;
                _ = operations.cancel.cancelled() => return Err(stopped()),
                pending = events.next() => Input::Owner(pending),
                message = pump.next() => Input::Socket(message),
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(start + Duration::from_millis(schedule.next_at_ms().min(delayed_push.as_ref().map_or(u64::MAX, |(at, _)| *at))))) => Input::Timer,
                changed = identity.changed() => { if changed.is_err() { return Err(stopped()); } Input::Identity },
            };
            match input {
                Input::Timer | Input::Identity => {},
                Input::Owner(pending) => {
                    if pending.overflow { return Err(HttpFailure::from("Socket queue budget exceeded")); }
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
                                    let confirmed = pump.effect(operations.call(move |operation| {
                                        journal::confirm(&operation.owner, &connection, &operation.journal)
                                    })).await?;
                                    if confirmed { *retried_full = false; waiting = false; }
                                }
                                let phase = reported.state().phase;
                                core_status = (phase, format!("{reported:?}"));
                                if !waiting { status.set(core_status.0, core_status.1.clone()); }
                            }
                            SyncEvent::Send { bytes } => {
                                let frame = Frame::decode(&bytes).map_err(fail)?;
                                let push = if let Frame::Push { id, .. } = &frame { Some(*id) } else { None };
                                if matches!(frame, Frame::Hello { .. }) { delayed_push = None; sent_push = None; }
                                trace_frame(&frame, false);
                                let message = envelope::encode(frame).map_err(fail)?;
                                if let Some(id) = push {
                                    match push_budget.admit(milliseconds(start), message.len()) {
                                        BudgetDecision::Wait { milliseconds: wait } => {
                                            if delayed_push.is_some() { return Err(HttpFailure::from("Multiple unsent pushes")); }
                                            delayed_push = Some((milliseconds(start).saturating_add(wait), bytes));
                                            continue;
                                        }
                                        BudgetDecision::TooLarge => return Err(outgrown()),
                                        BudgetDecision::Ready => sent_push = Some(id),
                                    }
                                    status.set(SyncPhase::Syncing, "Syncing files".into());
                                    let result = pump.effect(operations.call(move |operation| {
                                        let result = attachments::upload_blobs(&operation.owner, &operation.key, &mut uploaded);
                                        Ok((uploaded, result))
                                    })).await?;
                                    uploaded = result.0;
                                    // Folding never frees attachment space: FilesFull is terminal.
                                    result.1?;
                                }
                                pump.send(Message::Binary(message.into())).await?;
                            }
                        }
                    }
                }
                Input::Socket(Some(Ok(Message::Binary(bytes)))) => {
                    let frame = envelope::decode(&bytes).map_err(|error| HttpFailure::from(pop_core::sync::Pause {
                        kind: PauseKind::Refused, reason: fail(error),
                    }))?;
                    trace_frame(&frame, true);
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
                        uploaded = pump.effect(operations.call(move |operation| {
                            attachments::download_blobs(&operation.owner, &operation.key, &mut uploaded)?;
                            Ok(uploaded)
                        })).await?;
                    }
                    if !admission.frame(frame.encode()) { return Ok(false); }
                    pump.effect(operations.request(Request::State)).await?;
                    if !waiting { status.set(core_status.0, core_status.1.clone()); }
                }
                Input::Socket(Some(Ok(Message::Close(_)))) | Input::Socket(Some(Err(_))) | Input::Socket(None) => return Ok(false),
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
        Err(HttpFailure { kind: PauseKind::Ended, .. }) => {
            status.set(SyncPhase::Ended, "Ended".into());
            Ok(true)
        }
        Err(pause) if pause.kind.pauses() => {
            status.paused(pause.kind, pause.reason);
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
    journal: &Arc<journal::Journal>,
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
        journal: journal.clone(),
        cancel: CancellationToken::new(),
        identity: receive,
    };
    let mut retried = *retried_full;
    let result = run_network(async move {
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
    fn cancellation_returns_while_a_started_effect_finishes_without_late_notices() {
        let cancellation = CancellationToken::new();
        let cancelled = cancellation.clone();
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
        let (_identity, identity) = watch::channel(None);
        let caller = std::thread::spawn(move || {
            run_network(async move {
                blocking(&cancelled, identity, move || {
                    entered.send(()).unwrap();
                    finish.recv().unwrap();
                    status.set(SyncPhase::Synced, "Late result".into());
                    finished.send(()).unwrap();
                    Ok(())
                })
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
            origin: format!("http://{}", listener.local_addr().unwrap()),
        };
        request(&owner, Request::Share { share: Share { endpoint: key.origin.clone() } }).unwrap();
        let journal = crate::client::tests::test_journal(&owner, &key, true);
        let route = format!("GET /v1/documents/{}/blobs ", key.room);
        let (observed, controls) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut socket = tungstenite::accept_hdr(
                stream,
                |_: &tungstenite::handshake::server::Request, mut reply: tungstenite::handshake::server::Response| {
                    reply
                        .headers_mut()
                        .insert("Sec-WebSocket-Protocol", pop_sharing_protocol::SOCKET_PROTOCOL.parse().unwrap());
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
        let operation = Operations { owner: owner.clone(), key, journal, cancel: cancel.clone(), identity };
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
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
        assert!(pong, "server Ping must receive Pong while inventory HTTP is still held");
        assert!(
            early.is_ok_and(|result| matches!(result, Ok(false))),
            "socket close must terminate the attempt before HTTP is released"
        );
        assert!(cancelled_http, "socket close must cancel its in-flight blocking HTTP effect");
    }
}
