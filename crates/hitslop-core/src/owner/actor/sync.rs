//! Sync hooks. A host opens a socket to a shared document's relay and attaches with a
//! sink, hands the owner each frame it receives, and sends each frame the sink receives,
//! in order. The session (`crate::sync`) decides what frames mean; this module orders them
//! with edits: a remote change is published and saved like an edit, and each local edit is
//! pushed as it commits. The document's own listener hears none of this, only the changes.
use super::*;
use crate::sync::{Session, Status};
use std::collections::VecDeque;

/// How many relay frames may wait while a command evaluates; past it the connection
/// pauses and the next one catches up.
const HELD_FRAMES: usize = hitslop_sharing_protocol::QUEUED_FRAMES;

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
    epoch: u64,
    bytes: usize,
    frames: usize,
    overflow: bool,
}
pub(in crate::owner) struct Reservation {
    admission: Arc<std::sync::Mutex<Admission>>,
    epoch: u64,
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
        a.epoch == self.epoch && !a.overflow
    }
}
/// A socket's generation and bounded admission handle. Old sockets cannot feed or detach a newer one.
#[derive(Clone)]
pub struct SyncConnection {
    sender: std::sync::mpsc::Sender<Message>,
    admission: Arc<std::sync::Mutex<Admission>>,
    epoch: u64,
}
impl SyncConnection {
    pub fn frame(&self, bytes: Vec<u8>) -> bool {
        let mut a = self.admission.lock().expect("sync admission");
        if a.epoch != self.epoch || a.overflow {
            return false;
        }
        if bytes.len() > crate::sync::MAX_FRAME
            || a.bytes.saturating_add(bytes.len()) > hitslop_sharing_protocol::QUEUED_BYTES
            || a.frames >= hitslop_sharing_protocol::QUEUED_FRAMES
        {
            a.overflow = true;
            drop(a);
            let _ = self.sender.send(Message::Sync(SyncMessage::Overflow { epoch: self.epoch }));
            return false;
        }
        a.bytes += bytes.len();
        a.frames += 1;
        drop(a);
        let ticket = Reservation { admission: self.admission.clone(), epoch: self.epoch, size: bytes.len() };
        self.sender.send(Message::Sync(SyncMessage::Frame { bytes, ticket })).is_ok()
    }
    /// Export in the owner queue, ordered with local and remote edits.
    pub fn base(
        &self,
        reply: impl FnOnce(std::result::Result<Option<crate::sync::Base>, crate::sync::Pause>) + Send + 'static,
    ) {
        let _ = self.sender.send(Message::Sync(SyncMessage::Base { epoch: self.epoch, reply: Box::new(reply) }));
    }
    pub fn pause(&self, kind: crate::sync::PauseKind, reason: String) {
        let _ = self
            .sender
            .send(Message::Sync(SyncMessage::Pause { epoch: self.epoch, pause: crate::sync::Pause { kind, reason } }));
    }
    pub fn detach(&self) {
        let mut a = self.admission.lock().expect("sync admission");
        if a.epoch != self.epoch {
            return;
        }
        a.epoch += 1;
        let epoch = a.epoch;
        drop(a);
        let _ = self.sender.send(Message::Sync(SyncMessage::Detach { epoch }));
    }
}
type BaseReply = Box<dyn FnOnce(std::result::Result<Option<crate::sync::Base>, crate::sync::Pause>) + Send>;
pub(in crate::owner) enum SyncMessage {
    Base { epoch: u64, reply: BaseReply },
    Pause { epoch: u64, pause: crate::sync::Pause },
    Attach { sink: SyncSink, epoch: u64, admission: Arc<std::sync::Mutex<Admission>> },
    Frame { bytes: Vec<u8>, ticket: Reservation },
    Detach { epoch: u64 },
    Overflow { epoch: u64 },
}

/// The owner's sync state: the session (kept across connections), the attached
/// connection's sink and the status it last heard, and frames held while a command
/// evaluates.
pub(super) struct Link {
    session: Option<Session>,
    sink: Option<SyncSink>,
    reported: Status,
    held: VecDeque<(Vec<u8>, Reservation)>,
    epoch: u64,
}
impl Default for Link {
    fn default() -> Self {
        Self { session: None, sink: None, reported: Status::Offline, held: VecDeque::new(), epoch: 0 }
    }
}

impl Owner {
    /// A connection to the shared document's relay opened: the owner sends its Hello to
    /// `sink`, then every frame and status change, until the connection detaches.
    pub fn sync_connection(&self, sink: SyncSink) -> SyncConnection {
        let mut a = self.sync_admission.lock().expect("sync admission");
        a.epoch += 1;
        a.overflow = false;
        let epoch = a.epoch;
        drop(a);
        let _ = self.sender.send(Message::Sync(SyncMessage::Attach {
            sink,
            epoch,
            admission: self.sync_admission.clone(),
        }));
        SyncConnection { sender: self.sender.clone(), admission: self.sync_admission.clone(), epoch }
    }
    pub fn sync_attach(&self, sink: SyncSink) {
        self.sync_connection(sink);
    }
    pub fn sync_frame(&self, bytes: Vec<u8>) {
        let epoch = self.sync_admission.lock().expect("sync admission").epoch;
        SyncConnection { sender: self.sender.clone(), admission: self.sync_admission.clone(), epoch }.frame(bytes);
    }
    pub fn sync_detach(&self) {
        let epoch = self.sync_admission.lock().expect("sync admission").epoch;
        SyncConnection { sender: self.sender.clone(), admission: self.sync_admission.clone(), epoch }.detach();
    }
    /// The room this document syncs through, when it is shared.
    pub fn share(&self) -> Result<Option<store::Share>> {
        Ok(self.store.share()?)
    }
}

impl Actor {
    pub(super) fn sync(&mut self, message: SyncMessage) {
        match message {
            SyncMessage::Base { epoch, reply } => {
                let eligible = epoch == self.link.epoch
                    && self.lifecycle == Lifecycle::Open
                    && !self.evaluating
                    && !self.discarding
                    && !self.invalidated;
                let result = if eligible {
                    self.link.session.as_ref().map_or(Ok(None), |s| s.base(&self.core))
                } else {
                    Ok(None)
                };
                reply(result);
            }
            SyncMessage::Pause { epoch, pause } => {
                if epoch == self.link.epoch {
                    if let Some(session) = &mut self.link.session {
                        session.pause(pause.kind, pause.reason);
                    }
                    self.reported_status();
                }
            }
            SyncMessage::Attach { sink, epoch, admission } => {
                if admission.lock().expect("sync admission").epoch != epoch {
                    return;
                }
                self.link.epoch = epoch;
                self.link.held.clear();
                self.link.sink = Some(sink);
                self.link.reported = Status::Offline;
                if self.mode != store::Mode::Document || !self.store.is_shared() {
                    self.link.session = None;
                    self.report(Status::Paused {
                        kind: crate::sync::PauseKind::Refused,
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
                                kind: crate::sync::PauseKind::Refused,
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
                if !ticket.current() || ticket.epoch != self.link.epoch {
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
                            sink(SyncEvent::Reconnect);
                        }
                    }
                    return;
                }
                self.receive(&bytes);
            }
            SyncMessage::Overflow { epoch } => {
                if epoch != self.link.epoch {
                    return;
                }
                self.link.held.clear();
                if let Some(session) = &mut self.link.session {
                    session.disconnect();
                }
                self.reported_status();
                if let Some(sink) = self.link.sink.take() {
                    sink(SyncEvent::Reconnect);
                }
            }
            SyncMessage::Detach { epoch } => {
                if epoch < self.link.epoch {
                    return;
                }
                self.link.epoch = epoch;
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
    #[cfg(not(target_arch = "wasm32"))]
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
            self.link.reported = status.clone();
            self.tell(SyncEvent::Status { status });
        }
    }
    fn send(&self, bytes: Vec<u8>) {
        self.tell(SyncEvent::Send { bytes });
    }
    fn tell(&self, event: SyncEvent) {
        if let Some(sink) = &self.link.sink {
            let _ = catch_unwind(AssertUnwindSafe(|| sink(event)));
        }
    }
}
