//! Opaque frontier tokens shared by document and text operations.
use crate::{Code, Result, engine, err};
use loro::{Frontiers, ID, LoroDoc, VersionVector};

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 15) as usize] as char);
    }
    out
}
/// Version tokens name at most 1,024 frontier IDs of 12 bytes each.
const MAX_TOKEN_BYTES: usize = 12 * 1024;
fn unhex(s: &str) -> Result<Vec<u8>> {
    if s.len() > 2 * MAX_TOKEN_BYTES || !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(err(Code::InvalidVersion, "Expected an opaque version token"));
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(engine)).collect()
}
/// Version tokens are the document's frontiers: the IDs of its latest operations,
/// sorted, as 12-byte big-endian (peer, counter) records. They grow with concurrent
/// heads, not with every peer ever seen, and are stable across import and reopen until a
/// checkpoint trims the history they name.
pub(crate) fn version_token(frontiers: &Frontiers) -> String {
    let mut ids: Vec<ID> = frontiers.iter().collect();
    ids.sort();
    hex(&ids
        .iter()
        .flat_map(|id| id.peer.to_be_bytes().into_iter().chain(id.counter.to_be_bytes()))
        .collect::<Vec<_>>())
}
/// Decodes a token and proves every ID is in this document's history before any Loro
/// API sees it; unknown operations must never reach a panicking conversion.
pub(crate) fn decode_version(doc: &LoroDoc, s: &str) -> Result<(Frontiers, VersionVector)> {
    let bytes = unhex(s)?;
    if bytes.is_empty() || bytes.len() % 12 != 0 {
        return Err(err(Code::InvalidVersion, "Expected an opaque version token"));
    }
    let (known, trimmed) = (doc.oplog_vv(), doc.shallow_since_vv().to_vv());
    let mut ids = Vec::with_capacity(bytes.len() / 12);
    for record in bytes.chunks_exact(12) {
        let peer = u64::from_be_bytes(record[..8].try_into().expect("8 bytes"));
        let counter = i32::from_be_bytes(record[8..].try_into().expect("4 bytes"));
        if counter < 0 {
            return Err(err(Code::InvalidVersion, "Negative counter"));
        }
        let id = ID::new(peer, counter);
        if !known.includes_id(id) {
            return Err(err(Code::StaleBase, "Version names operations this document does not have"));
        }
        // Loro still resolves some trimmed IDs, but cannot branch or diff from them.
        if trimmed.includes_id(id) {
            return Err(err(Code::StaleBase, "Version precedes this document's retained history"));
        }
        ids.push(id);
    }
    let frontiers = Frontiers::from(ids);
    let vv = doc
        .frontiers_to_vv(&frontiers)
        .ok_or_else(|| err(Code::StaleBase, "Version is not in this document's history"))?;
    Ok((frontiers, vv))
}
