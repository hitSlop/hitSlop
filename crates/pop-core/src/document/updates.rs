//! Bounded relay exports stop at causal change boundaries, never inside opaque Loro bytes.
use super::*;
use loro::{ChangeMeta, ID, VersionVector};

const CHANGES_PER_BATCH: usize = 128;
const FRAME_METADATA_BYTES: usize = 4096;

impl Document {
    pub(crate) fn bounded_updates(&self, from: &VersionVector) -> Result<Option<(VersionVector, Vec<u8>)>> {
        use pop_sharing_protocol::limits::{PUSH_BATCH_BYTES, SOCKET_PUSH_BYTES_PER_MINUTE};
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
