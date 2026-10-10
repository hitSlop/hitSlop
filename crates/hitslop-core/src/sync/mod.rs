//! Sync through a relay, without IO: a host moves frames between a socket and its owner,
//! and this session decides what they mean (plans/sync-simplification.md §7.3).
//!
//! Every replica is a local owner with its own session peer. The relay stores the Loro
//! updates each replica pushes, indexed by the spans they hold, and serves each replica
//! what its version lacks. Nothing about sync is saved: each connection starts with a
//! Hello carrying the document's version, and the relay's answer says what to send. A lost
//! acknowledgement, a dropped socket or a crash heals with the next Hello.
mod frame;
pub use frame::{Ack, Frame, MAX_FRAME, PROTOCOL, Permission, Record, Span, Version};

use crate::{Applied, Code, Document};
use loro::VersionVector;

/// What a person sees about sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// No connection; edits save locally and sync when one opens.
    Offline,
    /// Connected, and catching up with the relay or sending changes it has not
    /// acknowledged.
    Syncing,
    /// Caught up, and the relay holds every change this copy made.
    Synced,
    /// Sync stopped for a reason a person must act on, until the next connection.
    Paused { reason: String },
}

/// What one frame from the relay did: frames to send back, and the document change it
/// made, which the owner publishes and saves like an edit.
#[derive(Default)]
pub struct Step {
    pub send: Vec<Vec<u8>>,
    pub applied: Option<Applied>,
}

/// One owner's sync state, in memory only.
pub struct Session {
    app_digest: [u8; 32],
    status: Status,
    /// Set by the relay's Welcome: what it holds, as far as this connection knows. Records
    /// received and pushes sent add to it.
    relay: Option<VersionVector>,
    write: bool,
    /// The one push the relay has not acknowledged; the next waits for it.
    in_flight: Option<u64>,
    /// The relay has sent everything this copy lacked on this connection.
    caught_up: bool,
    next_id: u64,
}
impl Session {
    /// A session for a document whose app definition hashes to `app_digest`: a relay
    /// admits only replicas of the app its room was created with.
    pub fn new(app_digest: [u8; 32]) -> Self {
        Self {
            app_digest,
            status: Status::Offline,
            relay: None,
            write: false,
            in_flight: None,
            caught_up: false,
            next_id: 1,
        }
    }
    pub fn status(&self) -> &Status {
        &self.status
    }
    /// A connection opened: the Hello to send first.
    pub fn connect(&mut self, doc: &Document) -> Vec<u8> {
        self.relay = None;
        self.in_flight = None;
        self.caught_up = false;
        self.status = Status::Syncing;
        Frame::Hello { protocol: PROTOCOL, app_digest: self.app_digest, version: to_wire(&doc.version_vector()) }
            .encode()
    }
    /// The connection closed. A pause keeps its reason until the next connection.
    pub fn disconnect(&mut self) {
        self.relay = None;
        self.in_flight = None;
        self.caught_up = false;
        if !matches!(self.status, Status::Paused { .. }) {
            self.status = Status::Offline;
        }
    }
    /// After every local commit: a push of what the relay lacks, unless one is in flight.
    /// Updates are exported after commits, so every pushed span ends at a commit boundary.
    pub fn push(&mut self, doc: &Document) -> Option<Vec<u8>> {
        if !self.write || self.in_flight.is_some() {
            return None;
        }
        let relay = self.relay.as_ref()?;
        let local = doc.version_vector();
        let spans: Vec<Span> = local
            .iter()
            .filter_map(|(&peer, &end)| {
                let start = relay.get(&peer).copied().unwrap_or(0);
                (end > start).then_some(Span { peer, start: start as u32, end: end as u32 })
            })
            .collect();
        if spans.is_empty() {
            return None;
        }
        let bytes = match doc.updates_since(relay) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.pause(format!("This document's changes could not be exported: {}", error.message));
                return None;
            }
        };
        let id = self.next_id;
        let frame = Frame::Push { id, records: vec![Record { spans: spans.clone(), bytes }] }.encode();
        if frame.len() > MAX_FRAME {
            self.pause("A change is too large to sync".into());
            return None;
        }
        cover(self.relay.as_mut()?, &spans);
        self.next_id += 1;
        self.in_flight = Some(id);
        self.settle();
        Some(frame)
    }
    /// One frame from the relay. A paused session ignores everything until it reconnects.
    pub fn receive(&mut self, doc: &mut Document, bytes: &[u8]) -> Step {
        if matches!(self.status, Status::Paused { .. }) || self.status == Status::Offline {
            return Step::default();
        }
        let frame = match Frame::decode(bytes) {
            Ok(frame) => frame,
            Err(error) => {
                self.pause(error.message);
                return Step::default();
            }
        };
        let mut step = Step::default();
        match frame {
            Frame::Welcome { permission, version } => {
                self.relay = Some(from_wire(&version));
                self.write = permission == Permission::Write;
                step.send.extend(self.push(doc));
            }
            Frame::Records(records) => {
                if self.relay.is_none() {
                    // A broadcast may already be in flight when a Gap triggers another
                    // Hello on this socket. Its Welcome backfills everything we lack.
                    return step;
                }
                let updates: Vec<&[u8]> = records.iter().map(|r| r.bytes.as_slice()).collect();
                match doc.import_remote(&updates) {
                    Ok(applied) => {
                        let relay = self.relay.as_mut().expect("admitted above");
                        for record in &records {
                            cover(relay, &record.spans);
                        }
                        step.applied = applied;
                    }
                    Err(error) => self.pause(refused(&error)),
                }
            }
            Frame::CaughtUp => {
                // Records whose dependencies never arrived are held by Loro, not applied:
                // the relay lacks history they depend on, and this is the only sign of it.
                let local = doc.version_vector();
                let missing = self
                    .relay
                    .as_ref()
                    .is_some_and(|relay| relay.iter().any(|(peer, end)| local.get(peer).copied().unwrap_or(0) < *end));
                if missing {
                    self.pause("The shared document is missing history; this copy keeps its own".into());
                } else if self.relay.is_some() {
                    self.caught_up = true;
                }
            }
            Frame::Ack { id, status } => {
                if self.in_flight != Some(id) {
                    return step;
                }
                self.in_flight = None;
                match status {
                    Ack::Stored => step.send.extend(self.push(doc)),
                    Ack::Gap => step.send.push(self.connect(doc)),
                    Ack::TooLarge => self.pause("A change is too large to sync".into()),
                    Ack::Denied => self.pause("This copy may not change the shared document".into()),
                    Ack::Quota => self.pause("The shared document is full".into()),
                }
            }
            Frame::Error { message, .. } => self.pause(message),
            Frame::Hello { .. } | Frame::Push { .. } => self.pause("The relay sent a client's frame".into()),
        }
        self.settle();
        step
    }
    /// Stops syncing until the next connection, for `reason`.
    pub fn pause(&mut self, reason: String) {
        self.relay = None;
        self.in_flight = None;
        self.caught_up = false;
        self.status = Status::Paused { reason };
    }
    /// While connected: synced once caught up with nothing unacknowledged. A push sends
    /// everything the relay lacks, so none in flight means the relay holds every change.
    fn settle(&mut self) {
        if matches!(self.status, Status::Syncing | Status::Synced) {
            self.status = if self.caught_up && self.in_flight.is_none() { Status::Synced } else { Status::Syncing };
        }
    }
}
fn refused(error: &crate::Error) -> String {
    match error.code {
        Code::RequiresUpdate => format!("Another copy needs a newer hitSlop: {}", error.message),
        _ => format!("A shared change was refused: {}", error.message),
    }
}
/// Adds `spans` to what `version` covers.
fn cover(version: &mut VersionVector, spans: &[Span]) {
    for span in spans {
        let end = span.end as i32;
        if version.get(&span.peer).is_none_or(|known| *known < end) {
            version.insert(span.peer, end);
        }
    }
}
fn to_wire(version: &VersionVector) -> Version {
    version.iter().map(|(&peer, &end)| (peer, end as u32)).collect()
}
fn from_wire(version: &Version) -> VersionVector {
    version.iter().map(|(&peer, &end)| (peer, end as i32)).collect()
}
