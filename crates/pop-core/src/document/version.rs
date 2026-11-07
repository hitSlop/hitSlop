//! Opaque version tokens: the document's frontiers, compared for equality only.
use loro::{Frontiers, ID};
use pop_contracts::hex;
/// Version tokens are the document's frontiers: the IDs of its latest operations,
/// sorted, as 12-byte big-endian (peer, counter) records. They grow with concurrent
/// heads, not with every peer ever seen, and are stable across import and reopen. A
/// command's `ifVersion` compares one for equality; nothing decodes them.
pub(crate) fn version_token(frontiers: &Frontiers) -> String {
    let mut ids: Vec<ID> = frontiers.iter().collect();
    ids.sort();
    hex(&ids
        .iter()
        .flat_map(|id| id.peer.to_be_bytes().into_iter().chain(id.counter.to_be_bytes()))
        .collect::<Vec<_>>())
}
