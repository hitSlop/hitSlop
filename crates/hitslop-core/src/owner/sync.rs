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
const HELD_FRAMES: usize = 256;

/// What the owner tells the connection that attached: frames to send, in order, and sync's
/// status whenever it changes.
#[derive(Debug)]
pub enum SyncEvent {
    Send { bytes: Vec<u8> },
    Status { status: Status },
}
pub type SyncSink = Arc<dyn Fn(SyncEvent) + Send + Sync>;

pub(super) enum SyncMessage {
    Attach(SyncSink),
    Frame(Vec<u8>),
    Detach,
}

/// The owner's sync state: the session (kept across connections), the attached
/// connection's sink and the status it last heard, and frames held while a command
/// evaluates.
pub(super) struct Link {
    session: Option<Session>,
    sink: Option<SyncSink>,
    reported: Status,
    held: VecDeque<Vec<u8>>,
}
impl Default for Link {
    fn default() -> Self {
        Self { session: None, sink: None, reported: Status::Offline, held: VecDeque::new() }
    }
}

impl Owner {
    /// A connection to the shared document's relay opened: the owner sends its Hello to
    /// `sink`, then every frame and status change, until the connection detaches.
    pub fn sync_attach(&self, sink: SyncSink) {
        let _ = self.sender.send(Message::Sync(SyncMessage::Attach(sink)));
    }
    /// One frame the relay sent, in the order it arrived.
    pub fn sync_frame(&self, bytes: Vec<u8>) {
        let _ = self.sender.send(Message::Sync(SyncMessage::Frame(bytes)));
    }
    /// The connection closed; the document keeps saving locally.
    pub fn sync_detach(&self) {
        let _ = self.sender.send(Message::Sync(SyncMessage::Detach));
    }
    /// The room this document syncs through, when it is shared.
    pub fn share(&self) -> Result<Option<store::Share>> {
        Ok(self.store.share()?)
    }
}

impl Actor {
    pub(super) fn sync(&mut self, message: SyncMessage) {
        match message {
            SyncMessage::Attach(sink) => {
                self.link.held.clear();
                self.link.sink = Some(sink);
                self.link.reported = Status::Offline;
                if self.mode != store::Mode::Document || !self.store.is_shared() {
                    self.link.session = None;
                    self.report(Status::Paused { reason: "This document is not shared".into() });
                    return;
                }
                if self.link.session.is_none() {
                    // The relay admits only replicas of the app its room was created with.
                    match self.store.app_digest() {
                        Ok(digest) => self.link.session = Some(Session::new(digest)),
                        Err(error) => {
                            self.report(Status::Paused { reason: error.to_string() });
                            return;
                        }
                    }
                }
                let hello = self.link.session.as_mut().expect("created above").connect(&self.core);
                self.send(hello);
                self.reported_status();
            }
            SyncMessage::Frame(bytes) => {
                if self.link.session.is_none() || self.link.sink.is_none() {
                    return;
                }
                // A command reads one version and its intents apply to it: changes from
                // other replicas wait until it finishes, like a held edit.
                if self.evaluating {
                    if self.link.held.len() < HELD_FRAMES {
                        self.link.held.push_back(bytes);
                    } else {
                        self.link.held.clear();
                        self.pause("Too many changes arrived at once; reconnect to catch up");
                    }
                    return;
                }
                self.receive(&bytes);
            }
            SyncMessage::Detach => {
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
            && let Some(bytes) = self.link.held.pop_front()
        {
            self.receive(&bytes);
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
    fn pause(&mut self, reason: &str) {
        if let Some(session) = &mut self.link.session {
            session.pause(reason.into());
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
