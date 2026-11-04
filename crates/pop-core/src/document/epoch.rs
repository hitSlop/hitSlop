//! Explicit authority changes; ordinary imports never replace a replica's history.
use super::*;

impl Document {
    /// Canonical frontier of accepted visible state; unintegrated operations are excluded.
    pub fn wire_history(&self) -> pop_sharing_protocol::lifecycle::WireVersion {
        pop_sharing_protocol::frame::version_pairs(self.doc.state_vv().iter().map(|(&peer, &end)| (peer, end as u32)))
    }
    /// Whether the accepted visible state covers the canonical relay frontier. Pending
    /// operations do not count: a handoff snapshot must contain every confirmed edit.
    pub fn covers_wire_history(&self, frontier: &[(String, u32)]) -> Result<bool> {
        pop_sharing_protocol::lifecycle::validate_version(frontier)
            .map_err(|message| err(Code::InvalidRequest, message))?;
        let visible = self.doc.state_vv();
        Ok(frontier.iter().all(|(peer, end)| {
            let peer = u64::from_str_radix(peer, 16).expect("canonical peer validated");
            visible.get(&peer).copied().unwrap_or(0) >= *end as i32
        }))
    }

    /// Installs an accepted checkpoint after the owner has durably published it under
    /// the writer lock. Authority/receipt and recovery checks belong to that owner.
    /// A replacement clears session undo and emits one complete, ordered publication.
    pub fn adopt_checkpoint(&mut self, checkpoint: &[u8]) -> Result<Applied> {
        self.intact()?;
        let mut next = Self::open(&self.app, checkpoint, &[])?;
        if Layout::of(&self.doc) != Layout::of(&next.doc) {
            return Err(err(Code::InvalidBytes, "An authority checkpoint cannot change the document layout"));
        }
        let reading = next.reading()?;
        if self.version() == reading.version {
            if self.projected() != reading.value || self.theme_state()?.overrides != next.theme_state()?.overrides {
                return Err(err(Code::InvalidBytes, "Conflicting state has the same history identity"));
            }
            return Ok(Applied {
                sequence: self.sequence,
                ids: vec![],
                publication: None,
                theme_changed: false,
                text: None,
            });
        }
        let sequence = self.sequence.checked_add(1).ok_or_else(|| err(Code::TooLarge, "Publication sequence"))?;
        let publication = encode(&Publication {
            previous: self.sequence,
            sequence,
            version: reading.version,
            ops: vec![wire::PatchOp::Set { path: vec![], value: reading.value }],
            theme: Some(reading.theme),
        });
        next.sequence = sequence;
        *self = next;
        Ok(Applied { sequence, ids: vec![], publication: Some(publication), theme_changed: true, text: None })
    }
}
