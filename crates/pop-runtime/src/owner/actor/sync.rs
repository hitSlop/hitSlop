//! Sync hooks. A host opens a socket to a shared document's relay and attaches with a
//! sink, hands the owner each frame it receives, and sends each frame the sink receives,
//! in order. The session (`pop_core::sync`) decides what frames mean; this module orders them
//! with edits: a remote change is published and saved like an edit, and each local edit is
//! pushed as it commits. The document's own listener hears none of this, only the changes.
use super::*;
use pop_core::sync::{Session, Status};
use std::collections::VecDeque;

/// How many relay frames may wait while a command evaluates; past it the connection
/// pauses and the next one catches up.
const HELD_FRAMES: usize = pop_contracts::relay::limits::QUEUED_FRAMES;

/// What the owner tells the connection that attached: frames to send, in order, and sync's
/// status whenever it changes.
#[derive(Debug)]
pub enum SyncEvent {
    Send { bytes: Vec<u8> },
    Status { status: Status },
    Reconnect,
}
pub type SyncSink = Arc<dyn Fn(SyncEvent) + Send + Sync>;

#[derive(Default)]
pub(in crate::owner) struct Admission {
    connection: u64,
    bytes: usize,
    frames: usize,
    overflow: bool,
}
pub(in crate::owner) struct Reservation {
    admission: Arc<std::sync::Mutex<Admission>>,
    connection: u64,
    size: usize,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut a = self.admission.lock().expect("sync admission");
        a.bytes -= self.size;
        a.frames -= 1;
    }
}
impl Reservation {
    fn current(&self) -> bool {
        let a = self.admission.lock().expect("sync admission");
        a.connection == self.connection && !a.overflow
    }
}
/// A socket's generation and bounded admission handle. Old sockets cannot feed or detach a newer one.
#[derive(Clone)]
pub struct SyncConnection {
    sender: std::sync::mpsc::Sender<Message>,
    admission: Arc<std::sync::Mutex<Admission>>,
    connection: u64,
}
impl SyncConnection {
    /// Confirmation belongs to this exact connection, not a replacement transport.
    pub fn confirmed_version(&self, reply: impl FnOnce(Option<pop_core::sync::Confirmation>) + Send + 'static) {
        let reply = PendingConfirmation(Some(Box::new(reply)));
        let _ = self.sender.send(Message::Sync(SyncMessage::ConfirmedVersion {
            connection: self.connection,
            durable: false,
            reply,
        }));
    }
    /// The exact current state only when the relay and the local save both acknowledge it.
    pub fn durable_confirmed_version(&self, reply: impl FnOnce(Option<pop_core::sync::Confirmation>) + Send + 'static) {
        let reply = PendingConfirmation(Some(Box::new(reply)));
        let _ = self.sender.send(Message::Sync(SyncMessage::ConfirmedVersion {
            connection: self.connection,
            durable: true,
            reply,
        }));
    }
    pub fn frame(&self, bytes: Vec<u8>) -> bool {
        let mut a = self.admission.lock().expect("sync admission");
        if a.connection != self.connection || a.overflow {
            return false;
        }
        if bytes.len() > pop_contracts::relay::frame::MAX_FRAME
            || a.bytes.saturating_add(bytes.len()) > pop_contracts::relay::limits::QUEUED_BYTES
            || a.frames >= pop_contracts::relay::limits::QUEUED_FRAMES
        {
            a.overflow = true;
            drop(a);
            let _ = self.sender.send(Message::Sync(SyncMessage::Overflow { connection: self.connection }));
            return false;
        }
        a.bytes += bytes.len();
        a.frames += 1;
        drop(a);
        let ticket = Reservation { admission: self.admission.clone(), connection: self.connection, size: bytes.len() };
        self.sender.send(Message::Sync(SyncMessage::Frame { bytes, ticket })).is_ok()
    }
    /// Export in the owner queue, ordered with local and remote edits.
    pub fn base(
        &self,
        reply: impl FnOnce(std::result::Result<Option<pop_core::sync::Base>, pop_core::sync::Pause>) + Send + 'static,
    ) {
        self.maintenance_base(move |result| {
            reply(result.and_then(|base| match base {
                pop_core::sync::BaseRead::Pending => Ok(None),
                pop_core::sync::BaseRead::Ready(base) => Ok(Some(base)),
                pop_core::sync::BaseRead::TooLarge => Err(pop_core::sync::Pause {
                    kind: pop_contracts::relay::PauseKind::Outgrown,
                    reason: "This document has outgrown sharing".into(),
                }),
            }))
        });
    }
    /// Export maintenance state; a frontier over `BASE_BYTES` reads as `TooLarge`, which
    /// the shared maintenance policy turns into the `Outgrown` pause.
    pub fn maintenance_base(
        &self,
        reply: impl FnOnce(std::result::Result<pop_core::sync::BaseRead, pop_core::sync::Pause>) + Send + 'static,
    ) {
        let pending = PendingBase(Some(Box::new(reply)));
        let reply: BaseReply = Box::new(move |result| pending.finish(result));
        if let Err(std::sync::mpsc::SendError(Message::Sync(SyncMessage::Base { reply, .. }))) =
            self.sender.send(Message::Sync(SyncMessage::Base { connection: self.connection, reply }))
        {
            complete_base(reply, Ok(pop_core::sync::BaseRead::Pending));
        }
    }
    pub fn pause(&self, kind: pop_contracts::relay::PauseKind, reason: String) {
        let _ = self.sender.send(Message::Sync(SyncMessage::Pause {
            connection: self.connection,
            pause: pop_core::sync::Pause { kind, reason },
        }));
    }
    pub fn detach(&self) {
        let mut a = self.admission.lock().expect("sync admission");
        if a.connection != self.connection {
            return;
        }
        a.connection += 1;
        let connection = a.connection;
        drop(a);
        let _ = self.sender.send(Message::Sync(SyncMessage::Detach { connection }));
    }
}
type BaseReply = Box<dyn FnOnce(std::result::Result<pop_core::sync::BaseRead, pop_core::sync::Pause>) + Send>;
struct PendingBase(Option<BaseReply>);
impl PendingBase {
    fn finish(mut self, result: std::result::Result<pop_core::sync::BaseRead, pop_core::sync::Pause>) {
        let reply = self.0.take().expect("pending base is answered once");
        complete_base(reply, result);
    }
}
impl Drop for PendingBase {
    fn drop(&mut self) {
        if let Some(reply) = self.0.take() {
            complete_base(reply, Ok(pop_core::sync::BaseRead::Pending));
        }
    }
}
fn complete_base(reply: BaseReply, result: std::result::Result<pop_core::sync::BaseRead, pop_core::sync::Pause>) {
    let _ = catch_unwind(AssertUnwindSafe(|| reply(result)));
}
fn notify(sink: &SyncSink, event: SyncEvent) {
    let _ = catch_unwind(AssertUnwindSafe(|| sink(event)));
}
pub(in crate::owner) enum SyncMessage {
    ConfirmedVersion { connection: u64, durable: bool, reply: PendingConfirmation },
    Base { connection: u64, reply: BaseReply },
    Pause { connection: u64, pause: pop_core::sync::Pause },
    Attach { sink: SyncSink, connection: u64, admission: Arc<std::sync::Mutex<Admission>> },
    Frame { bytes: Vec<u8>, ticket: Reservation },
    Detach { connection: u64 },
    Overflow { connection: u64 },
}
type ConfirmationReply = Box<dyn FnOnce(Option<pop_core::sync::Confirmation>) + Send>;
pub(in crate::owner) struct PendingConfirmation(Option<ConfirmationReply>);
impl PendingConfirmation {
    fn finish(mut self, version: Option<pop_core::sync::Confirmation>) {
        if let Some(reply) = self.0.take() {
            let _ = catch_unwind(AssertUnwindSafe(|| reply(version)));
        }
    }
}
impl Drop for PendingConfirmation {
    fn drop(&mut self) {
        if let Some(reply) = self.0.take() {
            let _ = catch_unwind(AssertUnwindSafe(|| reply(None)));
        }
    }
}

/// The owner's sync state: the session (kept across connections), the attached
/// connection's sink and the status it last heard, and frames held while a command
/// evaluates.
pub(super) struct Link {
    session: Option<Session>,
    sink: Option<SyncSink>,
    reported: Status,
    held: VecDeque<(Vec<u8>, Reservation)>,
    connection: u64,
    durable: Option<(u64, String)>,
    synced_reported_this_step: bool,
}
impl Default for Link {
    fn default() -> Self {
        Self {
            session: None,
            sink: None,
            reported: Status::Offline,
            held: VecDeque::new(),
            connection: 0,
            durable: None,
            synced_reported_this_step: false,
        }
    }
}

impl Owner {
    /// A connection to the shared document's relay opened: the owner sends its Hello to
    /// `sink`, then every frame and status change, until the connection detaches.
    pub fn sync_connection(&self, sink: SyncSink) -> SyncConnection {
        let mut a = self.sync_admission.lock().expect("sync admission");
        a.connection += 1;
        a.overflow = false;
        let connection = a.connection;
        drop(a);
        let _ = self.sender.send(Message::Sync(SyncMessage::Attach {
            sink,
            connection,
            admission: self.sync_admission.clone(),
        }));
        SyncConnection { sender: self.sender.clone(), admission: self.sync_admission.clone(), connection }
    }
    /// The room this document syncs through, when it is shared.
    pub fn share(&self) -> Result<Option<store::Share>> {
        Ok(self.store.share()?)
    }
}

impl Actor {
    pub(super) fn begin_sync_step(&mut self) {
        self.link.synced_reported_this_step = false;
    }
    fn durable_sync_version(&self) -> Option<pop_core::sync::Confirmation> {
        if self.link.sink.is_none()
            || self.link.session.is_none()
            || self.lifecycle != Lifecycle::Open
            || self.invalidated
            || self.discarding
            || self.writing
            || self.failure.is_some()
            || self.revision != self.saved
            || !self.store.confirms_saved(&self.core.history_version())
        {
            return None;
        }
        self.confirmed_sync_version()
    }
    /// A save may finish after the last relay acknowledgement. Wake the existing
    /// transport on that newly durable frontier without forcing another save.
    pub(super) fn report_durable_sync(&mut self) {
        if self.link.durable.as_ref().is_some_and(|(revision, _)| *revision == self.revision) {
            return;
        }
        let Some(confirmation) = self.durable_sync_version() else { return };
        let changed = self.link.durable.as_ref().is_none_or(|(_, version)| *version != confirmation.version);
        self.link.durable = Some((self.revision, confirmation.version));
        if changed && !self.link.synced_reported_this_step {
            self.tell(SyncEvent::Status { status: Status::Synced });
        }
    }
    pub(super) fn confirmed_sync_version(&self) -> Option<pop_core::sync::Confirmation> {
        self.link.session.as_ref()?.confirmed_version(&self.core)
    }
    pub(super) fn clear_sync_for_restore(&mut self) {
        self.link.session = None;
        self.link.held.clear();
        self.link.durable = None;
    }
    /// A reload replaced the document under any attached connection: start it over.
    pub(super) fn reconnect_after_reload(&mut self) {
        self.clear_sync_for_restore();
        if let Some(sink) = &self.link.sink {
            notify(sink, SyncEvent::Reconnect);
        }
    }
    pub(super) fn sync(&mut self, message: SyncMessage) {
        match message {
            SyncMessage::ConfirmedVersion { connection, durable, reply } => {
                let version = (connection == self.link.connection && self.lifecycle == Lifecycle::Open)
                    .then(|| if durable { self.durable_sync_version() } else { self.confirmed_sync_version() })
                    .flatten();
                reply.finish(version);
            }
            SyncMessage::Base { connection, reply } => {
                let eligible = connection == self.link.connection
                    && self.lifecycle == Lifecycle::Open
                    && !self.evaluating
                    && !self.discarding
                    && !self.invalidated;
                let result = if eligible {
                    self.link
                        .session
                        .as_ref()
                        .map_or(Ok(pop_core::sync::BaseRead::Pending), |s| s.maintenance_base(&self.core))
                } else {
                    Ok(pop_core::sync::BaseRead::Pending)
                };
                complete_base(reply, result);
            }
            SyncMessage::Pause { connection, pause } => {
                if connection == self.link.connection {
                    if let Some(session) = &mut self.link.session {
                        session.pause(pause.kind, pause.reason);
                    }
                    self.reported_status();
                }
            }
            SyncMessage::Attach { sink, connection, admission } => {
                if admission.lock().expect("sync admission").connection != connection {
                    return;
                }
                self.link.connection = connection;
                self.link.held.clear();
                self.link.durable = None;
                self.link.sink = Some(sink);
                self.link.reported = Status::Offline;
                if self.mode != store::Mode::Document || !self.store.is_shared() {
                    self.link.session = None;
                    self.report(Status::Paused {
                        kind: pop_contracts::relay::PauseKind::Refused,
                        reason: "This document is not shared".into(),
                    });
                    return;
                }
                if self.link.session.is_none() {
                    // The relay admits only replicas of the app its room was created with.
                    match self.store.app_digest() {
                        Ok(digest) => self.link.session = Some(Session::new(digest)),
                        Err(error) => {
                            self.report(Status::Paused {
                                kind: pop_contracts::relay::PauseKind::Refused,
                                reason: error.to_string(),
                            });
                            return;
                        }
                    }
                }
                let hello = self.link.session.as_mut().expect("created above").connect(&self.core);
                self.send(hello);
                self.reported_status();
            }
            SyncMessage::Frame { bytes, ticket } => {
                if !ticket.current() || ticket.connection != self.link.connection {
                    return;
                }
                if self.link.session.is_none() || self.link.sink.is_none() {
                    return;
                }
                // A command reads one version and its intents apply to it: changes from
                // other replicas wait until it finishes, like a held edit.
                if self.evaluating {
                    if self.link.held.len() < HELD_FRAMES {
                        self.link.held.push_back((bytes, ticket));
                    } else {
                        self.link.held.clear();
                        if let Some(session) = &mut self.link.session {
                            session.disconnect();
                        }
                        self.reported_status();
                        if let Some(sink) = self.link.sink.take() {
                            notify(&sink, SyncEvent::Reconnect);
                        }
                    }
                    return;
                }
                self.receive(&bytes);
            }
            SyncMessage::Overflow { connection } => {
                if connection != self.link.connection {
                    return;
                }
                self.link.held.clear();
                if let Some(session) = &mut self.link.session {
                    session.disconnect();
                }
                self.reported_status();
                if let Some(sink) = self.link.sink.take() {
                    notify(&sink, SyncEvent::Reconnect);
                }
            }
            SyncMessage::Detach { connection } => {
                if connection < self.link.connection {
                    return;
                }
                self.link.connection = connection;
                self.link.held.clear();
                if let Some(session) = &mut self.link.session {
                    session.disconnect();
                }
                self.reported_status();
                self.link.sink = None;
            }
        }
    }
    /// Frames held while a command evaluated, once it has finished.
    pub(super) fn sync_resume(&mut self) {
        while !self.evaluating
            && let Some((bytes, ticket)) = self.link.held.pop_front()
        {
            if ticket.current() {
                self.receive(&bytes);
            }
        }
    }
    /// After every local commit: what the relay lacks, unless a push is in flight.
    pub(super) fn sync_push(&mut self) {
        let Some(session) = &mut self.link.session else { return };
        if let Some(push) = session.push(&self.core) {
            self.send(push);
        }
        self.reported_status();
    }
    fn receive(&mut self, bytes: &[u8]) {
        // The document takes no change while closing, discarding or after a fault; the
        // relay resends whatever this connection misses after its next Hello.
        if self.lifecycle != Lifecycle::Open || self.discarding || self.invalidated {
            return;
        }
        let Some(session) = &mut self.link.session else { return };
        let core = &mut self.core;
        let step = catch_unwind(AssertUnwindSafe(|| session.receive(core, bytes)));
        let Ok(step) = step else {
            self.invalidated = true;
            self.fail(poisoned(), u64::MAX);
            return;
        };
        if self.core.is_broken() {
            self.invalidated = true;
            self.fail(poisoned(), u64::MAX);
        }
        for bytes in step.send {
            self.send(bytes);
        }
        if let Some(applied) = step.applied {
            self.accepted(applied.sequence, applied.publication, applied.theme_changed);
        }
        self.reported_status();
    }
    fn reported_status(&mut self) {
        let status = self.link.session.as_ref().map_or(Status::Offline, |s| s.status().clone());
        self.report(status);
    }
    fn report(&mut self, status: Status) {
        if status != self.link.reported {
            self.link.synced_reported_this_step |= status == Status::Synced;
            self.link.reported = status.clone();
            self.tell(SyncEvent::Status { status });
        }
    }
    fn send(&self, bytes: Vec<u8>) {
        self.tell(SyncEvent::Send { bytes });
    }
    fn tell(&self, event: SyncEvent) {
        if let Some(sink) = &self.link.sink {
            notify(sink, event);
        }
    }
}
