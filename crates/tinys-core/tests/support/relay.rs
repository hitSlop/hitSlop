//! The byte relay's rules (docs/architecture.md, Sync) over
//! memory: records in arrival order and the version they add up to. It starts at the
//! seed's version, which every replica already holds.
use tinys_core::sync::{Ack, Frame, PROTOCOL, Permission, Record, Span, Version};

pub struct Relay {
    pub app: [u8; 32],
    pub records: Vec<Record>,
    pub heads: Version,
}
impl Relay {
    /// Same transactional replacement as the opaque relay: retain uncovered records,
    /// then put the base first for the next Hello. Connected clients see no broadcast.
    pub fn compact(&mut self, base: tinys_core::sync::Base) {
        self.records
            .retain(|r| r.spans.iter().any(|s| s.end > base.frontier.get(&s.peer).copied().unwrap_or(0) as u32));
        let spans = base
            .frontier
            .iter()
            .filter_map(|(&peer, &end)| {
                let start = base.start.get(&peer).copied().unwrap_or(0);
                (end > start).then_some(Span { peer, start: start as u32, end: end as u32 })
            })
            .collect();
        self.records.insert(0, Record { spans, bytes: base.bytes });
    }

    /// The frames answering one client frame, and the records it stored, which go to the
    /// room's other joined clients.
    pub fn reply(&mut self, from: &[u8]) -> (Vec<Frame>, Vec<Record>) {
        match Frame::decode(from).expect("a client frame decodes") {
            Frame::Hello { protocol, app_digest, version } => {
                if protocol != PROTOCOL || app_digest != self.app {
                    return (vec![Frame::Error { code: 1, message: "A different app".into() }], vec![]);
                }
                let lacking: Vec<Record> = self
                    .records
                    .iter()
                    .filter(|r| r.spans.iter().any(|s| s.end > version.get(&s.peer).copied().unwrap_or(0)))
                    .cloned()
                    .collect();
                let mut frames = vec![Frame::Welcome { permission: Permission::Write, version: self.heads.clone() }];
                // Backfill in frames of a few records, as the relay cuts them by size.
                frames.extend(lacking.chunks(3).map(|chunk| Frame::Records(chunk.to_vec())));
                frames.push(Frame::CaughtUp);
                (frames, vec![])
            }
            Frame::Push { id, records } => {
                let held = |heads: &Version, s: &Span| heads.get(&s.peer).copied().unwrap_or(0);
                let mut stored = vec![];
                for record in records {
                    if record.spans.iter().any(|s| s.start > held(&self.heads, s)) {
                        return (vec![Frame::Ack { id, status: Ack::Gap }], stored);
                    }
                    if record.spans.iter().all(|s| s.end <= held(&self.heads, s)) {
                        continue;
                    }
                    for s in &record.spans {
                        let end = self.heads.entry(s.peer).or_insert(0);
                        *end = (*end).max(s.end);
                    }
                    self.records.push(record.clone());
                    stored.push(record);
                }
                (vec![Frame::Ack { id, status: Ack::Stored }], stored)
            }
            other => panic!("the relay got {other:?}"),
        }
    }
}
