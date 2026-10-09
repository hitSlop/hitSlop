//! Replication: what one document needs to follow another whose edits it does not make.
//! An authority accepts edits and sends what it accepted; a replica installs those updates,
//! or a checkpoint when it fell too far behind. Every install is checked in full, as
//! opening is, and a refused one leaves the replica as it was. These are document
//! operations only: transport, rooms and identity belong to the host.

use super::*;
use crate::PatchOp;

impl Document {
    /// Binary Loro version vector. It is transport-internal, never a page version token.
    pub fn version_vector(&self) -> Vec<u8> {
        // Loro serializes a hash map; rebuild in a fixed order so equal vectors have
        // equal bytes regardless of import order or prior map capacity.
        let version = self.doc.oplog_vv();
        let mut entries: Vec<_> = version.iter().map(|(&peer, &counter)| (peer, counter)).collect();
        entries.sort_unstable();
        entries.into_iter().collect::<VersionVector>().encode()
    }

    /// A transferable checkpoint always fits the existing storage budget. Retain full
    /// history when it fits; a large authority offers its current shallow state without
    /// trimming the live authority or changing its undo/text retention policy.
    pub fn transfer_snapshot(&self) -> Result<Vec<u8>> {
        self.intact()?;
        let full = self.checkpoint()?;
        if full.len() <= STORAGE_BYTES.saturating_sub(512) {
            return Ok(full);
        }
        let state = self.doc.export(ExportMode::shallow_snapshot(&self.doc.oplog_frontiers())).map_err(engine)?;
        if state.len() > STORAGE_BYTES.saturating_sub(512) {
            return Err(err(Code::TooLarge, "Replica checkpoint"));
        }
        Ok(state)
    }

    /// Accepted history since a replica's vector. An ahead/divergent replica may not
    /// silently discard its edits; an outdated replica needs a snapshot instead.
    pub fn updates_since(&self, encoded: &[u8]) -> Result<Vec<u8>> {
        self.intact()?;
        if encoded.len() > STORAGE_BYTES {
            return Err(err(Code::TooLarge, "Version vector"));
        }
        let version = VersionVector::decode(encoded).map_err(|e| err(Code::InvalidVersion, e))?;
        if !self.doc.oplog_vv().includes_vv(&version) {
            return Err(err(Code::StaleBase, "Replica has history absent from the authority"));
        }
        if !version.includes_vv(&self.doc.shallow_since_vv().to_vv()) {
            return Err(err(Code::MissingDependencies, "Replica requires a checkpoint"));
        }
        self.doc.export(ExportMode::updates(&version)).map_err(engine)
    }

    /// Import one accepted authority update. A refusal reconstructs unconditionally:
    /// Loro can retain missing-dependency operations without changing its frontiers.
    pub fn import_accepted(&mut self, bytes: &[u8]) -> Result<Applied> {
        self.intact()?;
        if bytes.len() > STORAGE_BYTES {
            return Err(err(Code::TooLarge, "Accepted update"));
        }
        // A shallow checkpoint can discard history needed for rollback. It must go
        // through candidate validation and forced persistence, never in-place import.
        let metadata = LoroDoc::decode_import_blob_meta(bytes, true).map_err(|e| err(Code::InvalidBytes, e))?;
        if metadata.mode != loro::EncodedBlobMode::Updates {
            return Err(err(Code::InvalidBytes, "Accepted updates must not contain a checkpoint"));
        }
        let before = self.doc.oplog_frontiers();
        let version = self.doc.oplog_vv();
        let floor = self.doc.shallow_since_vv();
        let sequence = self.sequence;
        let result = (|| {
            imported(self.doc.import(bytes))?;
            if self.doc.shallow_since_vv() != floor {
                return Err(err(Code::InvalidBytes, "Only a checkpoint may trim history"));
            }
            check_layout(&self.doc)?;
            check::stored(
                &self.app.schema,
                Some(ValueOrContainer::Container(Container::Map(self.doc.get_map("data")))),
            )?;
            self.app.theme.check_stored(&self.doc.get_map(theme::ROOT))?;
            if self.doc.oplog_vv() == version {
                lock(&self.events).clear();
                return Ok(Applied { sequence, ids: vec![], publication: None, theme_changed: false, text: None });
            }
            let publication = self.publish_with(true)?.ok_or_else(|| err(Code::EngineError, "Publication"))?;
            // Shared undo is unavailable; no imported operation becomes a local step.
            self.undo.clear();
            self.redo.clear();
            self.run = None;
            Ok(Applied {
                sequence: self.sequence,
                ids: vec![],
                theme_changed: publication.theme,
                publication: Some(publication.json),
                text: None,
            })
        })();
        if result.is_err() {
            let peer = self.doc.peer_id();
            self.rebuild_at(&before)?;
            self.doc.set_peer_id(peer).map_err(engine)?;
            self.sequence = sequence;
        }
        result
    }

    /// Validate a replacement before saving it. The owner installs this candidate only
    /// after its forced checkpoint job succeeds, then sends a snapshot reset to views.
    pub fn snapshot_candidate(&self, bytes: &[u8]) -> Result<Self> {
        self.intact()?;
        let mut candidate = Self::open(&self.app, bytes, &[])?;
        if !candidate.doc.oplog_vv().includes_vv(&self.doc.oplog_vv()) {
            return Err(err(Code::StaleBase, "Snapshot would discard replica history"));
        }
        candidate.doc.set_peer_id(self.doc.peer_id()).map_err(engine)?;
        candidate.sequence = self.sequence.checked_add(1).ok_or_else(|| err(Code::TooLarge, "Publication sequence"))?;
        candidate.floor = candidate.doc.shallow_since_vv().to_vv();
        Ok(candidate)
    }

    /// A snapshot installation uses the existing whole-root Set publication, so the
    /// experimental transport introduces no new page wire variant.
    pub fn reset_publication(&self, previous: u64) -> Result<String> {
        if previous.checked_add(1) != Some(self.sequence) {
            return Err(err(Code::InvalidRequest, "Snapshot reset sequence"));
        }
        let reading = self.reading()?;
        Ok(encode(&Publication {
            previous,
            sequence: self.sequence,
            version: reading.version,
            ops: vec![PatchOp::Set { path: vec![], value: reading.value }],
            theme: Some(reading.theme),
        }))
    }
}
