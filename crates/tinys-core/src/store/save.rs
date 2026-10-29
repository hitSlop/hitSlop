//! Save thresholds, exported jobs and checkpoint selection.
use super::*;

/// A save checkpoints instead of appending once the log reaches either.
const CHECKPOINT_ROWS: i64 = 256;
const CHECKPOINT_BYTES: i64 = 4 * 1024 * 1024;
/// A closing session that edited a document larger than this trims its history. Trimming
/// is not free: Loro re-encodes what it keeps instead of reusing its cached snapshot.
const TRIM_BYTES: i64 = 4 * 1024 * 1024;

/// How close saved history is to the storage format's frozen limits. `Full` means a save
/// found that neither an append nor a checkpoint fits, which usage alone cannot show: an
/// unshared document past the limits still checkpoints. The owner reports it while that
/// failure stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressure {
    Ok,
    Near,
    Full,
}
impl Pressure {
    /// `Near` once the saved history uses three quarters of either limit.
    pub fn from_usage(bytes: u64, rows: u64) -> Self {
        if bytes >= crate::STORAGE_BYTES as u64 * 3 / 4 || rows >= crate::STORAGE_ROWS as u64 * 3 / 4 {
            Self::Near
        } else {
            Self::Ok
        }
    }
}

/// Stored sizes, refreshed by every write, so choosing append or checkpoint needs no
/// database read.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(super) struct Metadata {
    pub(super) rows: i64,
    pub(super) update_bytes: i64,
    pub(super) checkpoint_bytes: i64,
}
impl Metadata {
    /// The checkpoint and update bytes the document holds.
    pub(super) fn stored(&self) -> i64 {
        self.checkpoint_bytes + self.update_bytes
    }
}
/// The storage limits every size decision checks: at most `STORAGE_ROWS` saved updates and
/// `STORAGE_BYTES` of checkpoint and updates.
pub(super) fn within(rows: i64, bytes: i64) -> bool {
    rows <= crate::STORAGE_ROWS as i64 && bytes <= crate::STORAGE_BYTES as i64
}
/// A checkpoint row's size: SQLite bounds the whole row, its header too.
pub(super) fn checkpoint_row(bytes: usize) -> i64 {
    bytes as i64 + crate::wire::STORAGE_ROW_OVERHEAD as i64
}
pub(super) fn bounds(conn: &Connection) -> Result<Metadata> {
    let (rows, update_bytes, checkpoint_bytes) = rows::state_sizes(conn)?;
    Ok(Metadata { rows, update_bytes, checkpoint_bytes })
}
/// The stored sizes, checked against the limits. Read before any blob is.
pub(super) fn checked_bounds(conn: &Connection) -> Result<Metadata> {
    let meta = bounds(conn)?;
    if !within(meta.rows, meta.stored()) {
        return Err(failed(format!(
            "Document exceeds storage limits ({} MiB or {} updates); keep the file for recovery",
            crate::STORAGE_BYTES >> 20,
            crate::STORAGE_ROWS
        )));
    }
    Ok(meta)
}

/// What a save writes, exported on the edit queue and written on the storage queue.
pub struct SaveJob {
    pub(super) rows: Rows,
    pub(super) version: VersionVector,
}
/// A save's Loro bytes: the updates since the last save, or a checkpoint replacing the log.
pub(super) enum Rows {
    Append(Vec<u8>),
    Checkpoint(Vec<u8>),
}
impl SaveJob {
    pub fn is_checkpoint(&self) -> bool {
        matches!(self.rows, Rows::Checkpoint(_))
    }
}

/// The updates since `saved`, when appending them keeps the log within the limits.
fn append(doc: &Document, saved: &VersionVector, meta: Metadata) -> Result<Option<Vec<u8>>> {
    let bytes = doc.doc.export(ExportMode::updates(saved)).map_err(failed)?;
    Ok(within(meta.rows + 1, meta.stored() + bytes.len() as i64).then_some(bytes))
}
/// A checkpoint that fits: the whole history, or only the current state when the history
/// does not fit. The live document keeps its history, so this
/// session's undo still works; only the file forgets it. A shared document always keeps its
/// whole history: another replica's edits can depend on any of it.
fn checkpoint(doc: &mut Document, shared: bool) -> Result<Option<Vec<u8>>> {
    let fits = |bytes: &[u8]| within(0, checkpoint_row(bytes.len()));
    let bytes = doc.doc.export(ExportMode::Snapshot).map_err(failed)?;
    if fits(&bytes) || shared {
        return Ok(fits(&bytes).then_some(bytes));
    }
    let latest = doc.doc.oplog_frontiers();
    trimmed(doc, &latest, fits)
}
/// A checkpoint keeping only the history since `start`, when `accept` takes its bytes.
fn trimmed(doc: &Document, start: &Frontiers, accept: impl Fn(&[u8]) -> bool) -> Result<Option<Vec<u8>>> {
    let bytes = doc.doc.export(ExportMode::shallow_snapshot(start)).map_err(failed)?;
    Ok(accept(&bytes).then_some(bytes))
}

impl Store {
    /// Saved history pressure; attachments and immutable app assets have separate limits.
    pub fn pressure(&self) -> Pressure {
        let meta = lock(&self.account).meta;
        Pressure::from_usage(meta.stored() as u64, meta.rows as u64)
    }

    /// The next write, or none when already durable. Exports only updates since the
    /// last save, or a checkpoint once the log is long or full.
    pub fn job(&self, doc: &mut Document) -> Result<Option<SaveJob>> {
        let (meta, saved, shared) = {
            let account = lock(&self.account);
            (account.meta, account.saved.clone(), account.shared)
        };
        let version = doc.doc.oplog_vv();
        if version == saved {
            return Ok(None);
        }
        // Prefer a checkpoint when the log is due for one.
        let due = meta.rows >= CHECKPOINT_ROWS || meta.update_bytes >= CHECKPOINT_BYTES;
        let order: &[bool] = if due { &[true, false] } else { &[false, true] };
        for &as_checkpoint in order {
            let bytes = if as_checkpoint { checkpoint(doc, shared)? } else { append(doc, &saved, meta)? };
            if let Some(bytes) = bytes {
                let rows = if as_checkpoint { Rows::Checkpoint(bytes) } else { Rows::Append(bytes) };
                return Ok(Some(SaveJob { rows, version }));
            }
        }
        Err(Error::Full)
    }

    /// An explicit checkpoint, even when clean. Unshared documents retain only current
    /// state; shared documents must retain all history other replicas can reference.
    pub fn checkpoint_job(&self, doc: &mut Document) -> Result<SaveJob> {
        let shared = lock(&self.account).shared;
        let bytes = if shared {
            checkpoint(doc, true)?
        } else {
            trimmed(doc, &doc.doc.oplog_frontiers(), |bytes| within(0, checkpoint_row(bytes.len())))?
        }
        .ok_or(Error::Full)?;
        Ok(SaveJob { rows: Rows::Checkpoint(bytes), version: doc.doc.oplog_vv() })
    }

    /// The checkpoint to write as the owner closes, after its last save: a session that
    /// edited a document larger than `TRIM_BYTES` leaves no history. Undo covers the open
    /// session only, so nothing reads it later. None when nothing would shrink.
    pub fn close_job(&self, doc: &mut Document) -> Result<Option<SaveJob>> {
        let (meta, changed, shared) = {
            let account = lock(&self.account);
            (account.meta, account.changed, account.shared)
        };
        let stored = meta.stored();
        if !changed || shared || stored <= TRIM_BYTES {
            return Ok(None);
        }
        let latest = doc.doc.oplog_frontiers();
        let smaller = |bytes: &[u8]| (bytes.len() as i64) < stored && within(0, checkpoint_row(bytes.len()));
        let version = doc.doc.oplog_vv();
        let bytes = trimmed(doc, &latest, smaller)?;
        Ok(bytes.map(|bytes| SaveJob { rows: Rows::Checkpoint(bytes), version }))
    }
}
