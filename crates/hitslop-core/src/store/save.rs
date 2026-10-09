//! Save budgets, exported jobs and checkpoint selection.
use super::*;

/// When saves checkpoint and how much history an open document keeps. Hosts use
/// `DEFAULT`; tests shrink it to reach the same paths with small documents.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Budget {
    /// A save checkpoints instead of appending once the log reaches either.
    pub checkpoint_rows: i64,
    pub checkpoint_bytes: i64,
    /// Retained history past this is bounded while the document is open.
    pub session_bytes: usize,
    /// A closing session that edited a document larger than this trims its history.
    /// Trimming is not free: Loro re-encodes what it keeps instead of reusing its cached
    /// snapshot.
    pub trim_bytes: i64,
    /// How long editing must pause before a due history rebuild starts.
    pub rebuild_idle: std::time::Duration,
}
impl Budget {
    pub(crate) const DEFAULT: Self = Self {
        checkpoint_rows: 256,
        checkpoint_bytes: 4 * 1024 * 1024,
        session_bytes: 16 * 1024 * 1024,
        trim_bytes: 4 * 1024 * 1024,
        rebuild_idle: std::time::Duration::from_secs(2),
    };
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
    bytes as i64 + 512
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
    /// A replacement's history start and size, recorded only once it is written.
    pub(super) rebuilt: Option<Rebuilt>,
    /// The retained history this checkpoint measured, when it is past the budget.
    pub(super) rebuild: Option<usize>,
}
pub(super) struct Rebuilt {
    pub(super) opened: Frontiers,
    pub(super) size: usize,
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
    /// The retained history this checkpoint measured, when a live owner should rebuild
    /// it. Measured by the export the checkpoint already made; appends never measure.
    pub(crate) fn rebuild_due(&self) -> Option<usize> {
        self.rebuild
    }
}

/// The updates since `saved`, when appending them keeps the log within the limits.
fn append(doc: &Document, saved: &VersionVector, meta: Metadata) -> Result<Option<Vec<u8>>> {
    let bytes = doc.doc.export(ExportMode::updates(saved)).map_err(failed)?;
    Ok(within(meta.rows + 1, meta.stored() + bytes.len() as i64).then_some(bytes))
}
/// A checkpoint that fits, and the retained size when it asks a live owner to rebuild:
/// the whole history while it is within the budget, else (for an owner that rebuilds)
/// the whole history once more, else the session's own history, else none. Compaction
/// keeps no history.
fn checkpoint(
    doc: &mut Document,
    opened: &Frontiers,
    compact: bool,
    budget: &Budget,
    rebuild_threshold: Option<usize>,
) -> Result<Option<(Vec<u8>, Option<usize>)>> {
    let fits = |bytes: &[u8]| within(0, checkpoint_row(bytes.len()));
    let latest = doc.doc.oplog_frontiers();
    if compact {
        return Ok(trimmed(doc, &latest, fits)?.map(|bytes| (bytes, None)));
    }
    let bytes = doc.doc.export(ExportMode::Snapshot).map_err(failed)?;
    if bytes.len() <= budget.session_bytes && fits(&bytes) {
        return Ok(Some((bytes, None)));
    }
    // An owner that rebuilds keeps writing the whole history it already exported until
    // its rebuild runs: trimming here would export again, only to be replaced.
    let rebuild = rebuild_threshold.filter(|threshold| bytes.len() > *threshold).map(|_| bytes.len());
    if rebuild_threshold.is_some() && fits(&bytes) {
        return Ok(Some((bytes, rebuild)));
    }
    let checkpoint = match trimmed(doc, opened, |b| b.len() <= budget.session_bytes && fits(b))? {
        Some(bytes) => Ok(Some(bytes)),
        None => trimmed(doc, &latest, fits),
    }?;
    Ok(checkpoint.map(|bytes| (bytes, rebuild)))
}
/// A checkpoint keeping only the history since `start`, when `accept` takes its bytes.
/// The document's edits may no longer branch from before `start`.
fn trimmed(doc: &mut Document, start: &Frontiers, accept: impl Fn(&[u8]) -> bool) -> Result<Option<Vec<u8>>> {
    let bytes = doc.doc.export(ExportMode::shallow_snapshot(start)).map_err(failed)?;
    if !accept(&bytes) {
        return Ok(None);
    }
    doc.retain_from(start);
    Ok(Some(bytes))
}

impl Store {
    /// The next write for `doc`, or none when the durable state already covers it and no
    /// checkpoint is requested. Exports only what it writes: the updates since the last
    /// save, or a checkpoint once the log is long, full or a checkpoint is requested.
    pub fn job(&self, doc: &mut Document, force_checkpoint: bool) -> Result<Option<SaveJob>> {
        self.prepare_job(doc, force_checkpoint, false)
    }

    /// The next write for a live owner that rebuilds its history (`rebuilds`): past the
    /// budget, its checkpoints keep the whole history and say when to rebuild, instead of
    /// trimming. The hard storage limit never changes.
    pub(crate) fn owner_job(&self, doc: &mut Document, rebuilds: bool) -> Result<Option<SaveJob>> {
        self.prepare_job(doc, false, rebuilds)
    }

    fn prepare_job(&self, doc: &mut Document, force_checkpoint: bool, rebuilds: bool) -> Result<Option<SaveJob>> {
        let (meta, saved, opened, budget, rebuilt_size) = {
            let account = lock(&self.account);
            (account.meta, account.saved.clone(), account.opened.clone(), account.budget, account.rebuilt_size)
        };
        let rebuild_threshold =
            rebuilds.then(|| budget.session_bytes.max(rebuilt_size.saturating_add(budget.checkpoint_bytes as usize)));
        let version = doc.doc.oplog_vv();
        if version == saved && !force_checkpoint {
            return Ok(None);
        }
        // A checkpoint first when one is requested or the log is due for one; a due
        // rebuild never prevents an append that still fits.
        let due = meta.rows >= budget.checkpoint_rows || meta.update_bytes >= budget.checkpoint_bytes;
        let order: &[bool] = if force_checkpoint {
            &[true]
        } else if due {
            &[true, false]
        } else {
            &[false, true]
        };
        for &as_checkpoint in order {
            let bytes = if as_checkpoint {
                checkpoint(doc, &opened, force_checkpoint, &budget, rebuild_threshold)?
            } else {
                append(doc, &saved, meta)?.map(|bytes| (bytes, None))
            };
            if let Some((bytes, rebuild)) = bytes {
                let rows = if as_checkpoint { Rows::Checkpoint(bytes) } else { Rows::Append(bytes) };
                return Ok(Some(SaveJob { rows, version, rebuilt: None, rebuild }));
            }
        }
        Err(Error::Full)
    }

    /// The checkpoint to write as the owner closes, after its last save: a session that
    /// edited a document larger than the budget's `trim_bytes` leaves no history. Undo covers the open
    /// session only, so nothing reads it later. None when nothing would shrink.
    pub fn close_job(&self, doc: &mut Document) -> Result<Option<SaveJob>> {
        let (meta, changed, trim_bytes) = {
            let account = lock(&self.account);
            (account.meta, account.changed, account.budget.trim_bytes)
        };
        let stored = meta.stored();
        if !changed || stored <= trim_bytes {
            return Ok(None);
        }
        let latest = doc.doc.oplog_frontiers();
        let smaller = |bytes: &[u8]| (bytes.len() as i64) < stored && within(0, checkpoint_row(bytes.len()));
        let version = doc.doc.oplog_vv();
        let bytes = trimmed(doc, &latest, smaller)?;
        Ok(bytes.map(|bytes| SaveJob { rows: Rows::Checkpoint(bytes), version, rebuilt: None, rebuild: None }))
    }

    /// The checkpoint that replaces the saved state with `candidate`, a validated document
    /// whose checkpoint is `bytes`, written even when its version equals the saved one (a
    /// rebuild keeps the version and drops history). Changes neither the live document nor
    /// this store's accounting until it is written.
    pub(crate) fn replacement_job(&self, candidate: &Document, bytes: Vec<u8>) -> Result<SaveJob> {
        self.check(true)?;
        if !within(0, checkpoint_row(bytes.len())) {
            return Err(Error::Full);
        }
        Ok(SaveJob {
            rebuilt: Some(Rebuilt { opened: candidate.doc.oplog_frontiers(), size: bytes.len() }),
            rows: Rows::Checkpoint(bytes),
            version: candidate.doc.oplog_vv(),
            rebuild: None,
        })
    }

    /// The checkpoint that replaces the saved state with `candidate`, a document already
    /// checked against this store's app: a replica's installed snapshot.
    pub fn replacement(&self, candidate: &Document) -> Result<SaveJob> {
        let bytes = candidate.checkpoint().map_err(load_failure)?;
        self.replacement_job(candidate, bytes)
    }

    /// A rebuild of `size` retained bytes failed: the next is due only after another
    /// checkpoint's worth of history.
    pub(crate) fn defer_rebuild(&self, size: usize) {
        lock(&self.account).rebuilt_size = size;
    }

    pub(crate) fn budget(&self) -> Budget {
        lock(&self.account).budget
    }

    #[cfg(test)]
    pub(crate) fn set_budget(&self, budget: Budget) {
        lock(&self.account).budget = budget;
    }

    #[cfg(test)]
    pub(crate) fn rebuilt_size(&self) -> usize {
        lock(&self.account).rebuilt_size
    }
}
