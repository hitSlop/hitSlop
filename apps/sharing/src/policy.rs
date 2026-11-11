//! Runtime-independent lifecycle, relay admission and compaction decisions.
use crate::{limits, refusal::Refusal};
use pop_contracts::relay::{
    frame::{Record, Span, Version},
    limits::{SOCKET_PUSH_BYTES_PER_MINUTE, SOCKET_PUSHES_PER_MINUTE},
};
use pop_sharing_protocol::{CONTROL_BYTES, http::RoomPhase};

/// The lifecycle advances once through each phase; retries do not transition.
pub fn transition(from: Option<RoomPhase>, to: RoomPhase) -> Result<RoomPhase, Refusal> {
    if matches!(
        (from, to),
        (None, RoomPhase::Uploading)
            | (Some(RoomPhase::Uploading), RoomPhase::Live)
            | (Some(RoomPhase::Live | RoomPhase::Uploading), RoomPhase::Ended)
    ) {
        Ok(to)
    } else {
        Err(Refusal::new(409, "conflict", "Invalid phase transition"))
    }
}

/// Staging has room for two complete seed and attachment sets, independent of Open.
pub fn staging_fits(count: usize, bytes: u64) -> bool {
    count <= limits::STAGING_ALLOWANCES * (pop_contracts::wire::ATTACHMENT_COUNT + 1)
        && bytes
            <= (limits::STAGING_ALLOWANCES * (pop_contracts::wire::ATTACHMENT_BYTES + pop_sharing_protocol::SEED_BYTES))
                as u64
}

/// Parse bounded version headers, including duplicate-peer rejection.
pub fn version_header(text: &str) -> Option<Version> {
    if text.len() > CONTROL_BYTES {
        return None;
    }
    let pairs: Vec<(String, u32)> = serde_json::from_str(text).ok()?;
    let version = version(&pairs).ok()?;
    (version.len() == pairs.len()).then_some(version)
}

/// Decode the hex peer representation used by seed metadata and stored versions.
pub fn version(pairs: &[(String, u32)]) -> Result<Version, &'static str> {
    pairs
        .iter()
        .map(|(peer, end)| {
            if peer.len() != 16 || *end > i32::MAX as u32 {
                return Err("Invalid version");
            }
            Ok((u64::from_str_radix(peer, 16).map_err(|_| "Invalid peer")?, *end))
        })
        .collect()
}

/// Per-socket minute budget, also charging duplicate pushes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PushBudget {
    pub window_start: u64,
    pub bytes: usize,
    pub pushes: usize,
}
impl PushBudget {
    /// Charge before admission, preserving the charged state even on refusal.
    pub fn charge(&mut self, now: u64, bytes: usize) -> bool {
        if now.saturating_sub(self.window_start) >= 60_000 {
            *self = Self { window_start: now, ..Self::default() };
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.pushes = self.pushes.saturating_add(1);
        self.bytes <= SOCKET_PUSH_BYTES_PER_MINUTE && self.pushes <= SOCKET_PUSHES_PER_MINUTE
    }
}

/// Newly accepted records and the resulting heads, or a gap refusal.
pub fn admit_push(mut heads: Version, records: Vec<Record>) -> Option<(Version, Vec<Record>)> {
    let mut added = Vec::new();
    for record in records {
        if record.spans.iter().any(|s| s.start > heads.get(&s.peer).copied().unwrap_or(0)) {
            return None;
        }
        if record.spans.iter().all(|s| s.end <= heads.get(&s.peer).copied().unwrap_or(0)) {
            continue;
        }
        for span in &record.spans {
            heads.entry(span.peer).and_modify(|n| *n = (*n).max(span.end)).or_insert(span.end);
        }
        added.push(record);
    }
    Some((heads, added))
}

/// Duplicate-only pushes succeed even at room quota.
pub fn push_fits(bytes: usize, rows: usize, added: &[Record]) -> bool {
    added.is_empty()
        || (bytes.saturating_add(added.iter().map(|r| r.bytes.len()).sum::<usize>()) <= limits::ROOM_RECORD_BYTES
            && rows.saturating_add(added.len()) <= limits::ROOM_RECORDS
            && rows < limits::ROOM_RECORDS)
}

/// Whether retained updates warrant folding into a base.
pub fn compact_due(bytes: usize, rows: usize) -> bool {
    bytes >= limits::COMPACT_AFTER_BYTES || rows >= limits::COMPACT_AFTER_RECORDS
}

/// Version fences and quotas for a proposed fold.
pub struct Fold<'a> {
    pub root: &'a Version,
    pub previous: &'a Version,
    pub heads: &'a Version,
    pub frontier: &'a Version,
    pub bytes: usize,
    pub rows: usize,
}
impl Fold<'_> {
    /// A fold advances the base, contains the root and never exceeds accepted heads.
    pub fn admitted(&self) -> bool {
        compact_due(self.bytes, self.rows)
            && !self.frontier.iter().any(|(p, n)| *n > self.heads.get(p).copied().unwrap_or(0))
            && !self
                .previous
                .iter()
                .chain(self.root.iter())
                .any(|(p, n)| self.frontier.get(p).copied().unwrap_or(0) < *n)
            && self.frontier.iter().any(|(p, n)| *n > self.previous.get(p).copied().unwrap_or(0))
    }
}

/// Bases always fold; an update folds only when every span is covered.
pub fn folds_record(seq: i64, spans: &[Span], frontier: &Version) -> bool {
    seq == 0 || spans.iter().all(|s| s.end <= frontier.get(&s.peer).copied().unwrap_or(0))
}

/// Fixed-width stored spans avoid JSON and hex expansion of record metadata.
pub fn encode_spans(spans: &[Span]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(spans.len() * 16);
    for span in spans {
        bytes.extend_from_slice(&span.peer.to_be_bytes());
        bytes.extend_from_slice(&span.start.to_be_bytes());
        bytes.extend_from_slice(&span.end.to_be_bytes());
    }
    bytes
}

/// Read the Worker's binary span column, refusing incomplete stored metadata.
pub fn decode_spans(bytes: &[u8]) -> Result<Vec<Span>, &'static str> {
    if !bytes.len().is_multiple_of(16) {
        return Err("Invalid stored spans");
    }
    Ok(bytes
        .chunks_exact(16)
        .map(|bytes| Span {
            peer: u64::from_be_bytes(bytes[..8].try_into().expect("eight peer bytes")),
            start: u32::from_be_bytes(bytes[8..12].try_into().expect("four start bytes")),
            end: u32::from_be_bytes(bytes[12..].try_into().expect("four end bytes")),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staging_admission_bounds_count_and_bytes_independently() {
        let count = limits::STAGING_ALLOWANCES * (pop_contracts::wire::ATTACHMENT_COUNT + 1);
        let bytes = (limits::STAGING_ALLOWANCES
            * (pop_contracts::wire::ATTACHMENT_BYTES + pop_sharing_protocol::SEED_BYTES)) as u64;
        for (count, bytes, fits) in [
            (0, 0, true),
            (count, bytes, true),
            (count + 1, 0, false),
            (0, bytes + 1, false),
            (usize::MAX, u64::MAX, false),
        ] {
            assert_eq!(staging_fits(count, bytes), fits);
        }
    }

    #[test]
    fn lifecycle_accepts_only_forward_transitions() {
        use RoomPhase::*;
        for from in [None, Some(Uploading), Some(Live), Some(Ended)] {
            for to in [Uploading, Live, Ended] {
                let legal =
                    matches!((from, to), (None, Uploading) | (Some(Uploading), Live | Ended) | (Some(Live), Ended));
                assert_eq!(transition(from, to).is_ok(), legal, "{from:?} -> {to:?}");
            }
        }
    }
    #[test]
    fn version_headers_refuse_duplicates_overflow_and_malformed_peers() {
        assert_eq!(version_header("[[\"0000000000000001\",1]]"), Some(Version::from([(1, 1)])));
        for text in [
            "bad",
            "[[\"1\",1]]",
            "[[\"0000000000000001\",2147483648]]",
            "[[\"0000000000000001\",1],[\"0000000000000001\",2]]",
        ] {
            assert!(version_header(text).is_none(), "{text}");
        }
        assert!(version_header(&" ".repeat(CONTROL_BYTES + 1)).is_none());
    }
    #[test]
    fn budget_boundaries_charge_refusals_and_reset_after_hibernation() {
        let mut b = PushBudget {
            window_start: 1,
            bytes: SOCKET_PUSH_BYTES_PER_MINUTE - 1,
            pushes: SOCKET_PUSHES_PER_MINUTE - 1,
        };
        assert!(b.charge(2, 1));
        assert!(!b.charge(2, 0));
        assert!(b.charge(60_001, 1));
        assert_eq!(b.pushes, 1);
        b.bytes = usize::MAX;
        assert!(!b.charge(60_002, 1));
    }
    #[test]
    fn push_admission_handles_duplicates_gaps_and_quota_edges() {
        let record = |start, end| Record { spans: vec![Span { peer: 1, start, end }], bytes: vec![1] };
        assert!(admit_push(Version::new(), vec![record(1, 2)]).is_none());
        let (heads, added) = admit_push(Version::from([(1, 1)]), vec![record(0, 1), record(1, 2)]).unwrap();
        assert_eq!(heads[&1], 2);
        assert_eq!(added.len(), 1);
        assert!(push_fits(limits::ROOM_RECORD_BYTES, limits::ROOM_RECORDS, &[]));
        assert!(push_fits(limits::ROOM_RECORD_BYTES - 1, limits::ROOM_RECORDS - 1, &added));
        assert!(!push_fits(limits::ROOM_RECORD_BYTES, 0, &added));
        assert!(!push_fits(0, limits::ROOM_RECORDS, &added));
    }
    #[test]
    fn folds_respect_size_row_and_frontier_boundaries() {
        let root = Version::from([(1, 1)]);
        let heads = Version::from([(1, 3), (2, 2)]);
        for (bytes, rows, due) in [
            (limits::COMPACT_AFTER_BYTES - 1, limits::COMPACT_AFTER_RECORDS - 1, false),
            (limits::COMPACT_AFTER_BYTES, 0, true),
            (0, limits::COMPACT_AFTER_RECORDS, true),
        ] {
            for (frontier, valid) in [
                (Version::from([(1, 2)]), true),
                (root.clone(), false),
                (Version::new(), false),
                (Version::from([(1, 4)]), false),
            ] {
                assert_eq!(
                    Fold { root: &root, previous: &root, heads: &heads, frontier: &frontier, bytes, rows }.admitted(),
                    due && valid
                );
            }
        }
        let spans = [Span { peer: 1, start: 1, end: 2 }, Span { peer: 2, start: 0, end: 2 }];
        assert!(!folds_record(1, &spans, &Version::from([(1, 2), (2, 1)])));
        assert!(folds_record(1, &spans, &heads));
        assert!(folds_record(0, &spans, &Version::new()));
    }
}

/// Account entries, including removal high-water marks, accept only newer document revisions.
pub fn apply_library_revision(current: Option<u64>, incoming: u64) -> bool {
    incoming > 0 && current.is_none_or(|revision| incoming > revision)
}

#[cfg(test)]
mod library_tests {
    use super::apply_library_revision;

    #[test]
    fn late_and_duplicate_deliveries_cannot_resurrect_removal_or_drop_rejoin() {
        let mut current = None;
        let mut present = false;
        for (revision, entry, expected_revision, expected_present) in [
            (1, true, 1, true),
            (2, false, 2, false),
            (1, true, 2, false),
            (2, false, 2, false),
            (3, true, 3, true),
            (2, false, 3, true),
            (3, true, 3, true),
            (4, false, 4, false),
        ] {
            if apply_library_revision(current, revision) {
                current = Some(revision);
                present = entry;
            }
            assert_eq!((current, present), (Some(expected_revision), expected_present));
        }
    }

    #[test]
    fn reordering_converges_to_latest_revision() {
        for order in [[1, 2, 3], [3, 2, 1], [2, 3, 1], [2, 1, 3], [1, 3, 2], [3, 1, 2]] {
            let mut current = None;
            for revision in order {
                if apply_library_revision(current, revision) {
                    current = Some(revision);
                }
            }
            assert_eq!(current, Some(3));
        }
        assert!(!apply_library_revision(None, 0));
        assert!(!apply_library_revision(Some(u64::MAX), u64::MAX));
    }
}
