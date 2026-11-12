//! Bounded relay exports stop at causal change boundaries, never inside opaque Loro bytes.
use super::*;
use loro::{ChangeMeta, ID, VersionVector};

const CHANGES_PER_BATCH: usize = 128;
const FRAME_METADATA_BYTES: usize = 4096;

impl Document {
    /// Canonical relay frontier of accepted visible state, as the seed route sends it;
    /// unintegrated operations are excluded.
    pub fn wire_history(&self) -> Vec<(String, u32)> {
        pop_contracts::relay::frame::version_pairs(self.doc.state_vv().iter().map(|(&peer, &end)| (peer, end as u32)))
    }
    /// Every operation this replica holds, by peer.
    pub(crate) fn version_vector(&self) -> loro::VersionVector {
        self.doc.oplog_vv()
    }
    /// The operations this replica holds past `version`, as one Loro update.
    pub(crate) fn updates_since(&self, version: &loro::VersionVector) -> Result<Vec<u8>> {
        self.doc.export(ExportMode::updates(version)).map_err(engine)
    }
    /// Opaque room base: retained operations, never a shallow snapshot. Seed state is
    /// already present in every room replica; snapshots would leave pending imports.
    pub fn base(&self, frontier: &loro::VersionVector) -> Result<Vec<u8>> {
        let start = self.base_start();
        if !frontier.includes_vv(&start) || !self.doc.oplog_vv().includes_vv(frontier) {
            return Err(err(Code::MissingDependencies, "The base frontier is outside retained history"));
        }
        let bytes = if self.doc.oplog_vv() == *frontier {
            self.doc.export(ExportMode::updates(&start))
        } else {
            self.doc.export(ExportMode::updates_in_range(frontier.sub_iter(&start).collect::<Vec<_>>()))
        }
        .map_err(engine)?;
        let meta = LoroDoc::decode_import_blob_meta(&bytes, false).map_err(engine)?;
        if meta.mode != loro::EncodedBlobMode::Updates
            || meta.partial_end_vv != *frontier
            || !start.includes_vv(&meta.partial_start_vv)
        {
            return Err(err(Code::InvalidBytes, "The base does not cover its declared history"));
        }
        if bytes.len() > pop_contracts::relay::limits::BASE_BYTES {
            return Err(err(Code::TooLarge, "This document has outgrown sharing"));
        }
        Ok(bytes)
    }
    pub fn base_start(&self) -> loro::VersionVector {
        self.doc.shallow_since_vv().to_vv()
    }
    /// Changes other replicas made, as Loro updates from a relay: imported, checked
    /// against the app as a saved document is, and published like an edit. An update
    /// whose dependencies have not arrived waits inside Loro until they do. A change the
    /// app cannot accept is refused and leaves the document as it was, though this
    /// session's undo starts over. `None` when the updates held nothing new.
    pub fn import_remote(&mut self, updates: &[&[u8]]) -> Result<Option<Applied>> {
        self.intact()?;
        let before = self.doc.state_frontiers();
        let version = self.doc.oplog_vv();
        let root = history_root(&self.doc)?;
        let imported = updates
            .iter()
            .try_for_each(|bytes| self.doc.import(bytes).map(drop))
            .map_err(|e| err(Code::InvalidBytes, e))
            .and_then(|()| check_history_root(&self.doc, root, &version))
            .and_then(|()| acceptable(&self.doc, &self.app));
        if let Err(error) = imported {
            self.abort(&before)?;
            return Err(error);
        }
        if self.doc.oplog_vv() == version {
            return Ok(None);
        }
        // Published even when nothing visible changed, so the page's version moves on.
        let published = match self.publish_with(true) {
            Ok(published) => published,
            Err(error) => {
                self.abort(&before)?;
                return Err(error);
            }
        };
        Ok(Some(Self::applied(self.sequence, vec![], published, None)))
    }

    pub(crate) fn bounded_updates(&self, from: &VersionVector) -> Result<Option<(VersionVector, Vec<u8>)>> {
        use pop_contracts::relay::limits::{PUSH_BATCH_BYTES, SOCKET_PUSH_BYTES_PER_MINUTE};
        let local = self.version_vector();
        let mut end = from.clone();
        let mut batch = None;
        for _ in 0..CHANGES_PER_BATCH {
            let Some(change) = self.next_change(&local, &end)? else { break };
            let length = i32::try_from(change.len).map_err(|_| err(Code::TooLarge, "Change is too large"))?;
            let counter =
                change.id.counter.checked_add(length).ok_or_else(|| err(Code::TooLarge, "Change is too large"))?;
            let mut next = end.clone();
            next.set_end(ID::new(change.id.peer, counter));
            let bytes = self
                .doc
                .export(ExportMode::updates_in_range(next.sub_iter(from).collect::<Vec<_>>()))
                .map_err(engine)?;
            if bytes.len() > PUSH_BATCH_BYTES - FRAME_METADATA_BYTES && batch.is_some() {
                break;
            }
            if bytes.len() > SOCKET_PUSH_BYTES_PER_MINUTE - FRAME_METADATA_BYTES {
                return Err(err(Code::TooLarge, "This change is too large to share"));
            }
            end = next;
            let large = bytes.len() > PUSH_BATCH_BYTES - FRAME_METADATA_BYTES;
            batch = Some((end.clone(), bytes));
            if large {
                break;
            }
        }
        let Some((end, bytes)) = batch else { return Ok(None) };
        // Imported history can contain intermediate states that were never validated
        // alone. Such a prefix is refused rather than reaching a replica as an invalid
        // document. This does not mutate the live document.
        let local_end: VersionVector = end
            .iter()
            .filter_map(|(&peer, &counter)| {
                let counter = counter.min(local.get(&peer).copied().unwrap_or(0));
                (counter > 0).then_some((peer, counter))
            })
            .collect();
        let frontiers = self.doc.try_vv_to_frontiers(&local_end).map_err(engine)?;
        let candidate = replica_at(&self.doc, &frontiers)?;
        if acceptable(&candidate, &self.app).is_err() {
            return Err(err(Code::TooLarge, "The retained changes cannot be shared"));
        }
        Ok(Some((end, bytes)))
    }

    fn next_change(&self, local: &VersionVector, known: &VersionVector) -> Result<Option<ChangeMeta>> {
        let mut selected: Option<ChangeMeta> = None;
        let mut missing = false;
        for (&peer, &end) in local.iter() {
            let start = known.get(&peer).copied().unwrap_or(0);
            if start >= end {
                continue;
            }
            missing = true;
            let change =
                self.doc.try_get_change(ID::new(peer, start)).map_err(engine)?.ok_or_else(|| {
                    err(Code::MissingDependencies, "Sharing needs history outside the retained history")
                })?;
            if change.deps.iter().all(|id| known.includes_id(id))
                && selected.as_ref().is_none_or(|old| (change.lamport, change.id.peer) < (old.lamport, old.id.peer))
            {
                selected = Some(change);
            }
        }
        if missing && selected.is_none() {
            return Err(err(Code::MissingDependencies, "Sharing needs missing causal history"));
        }
        Ok(selected)
    }
}
