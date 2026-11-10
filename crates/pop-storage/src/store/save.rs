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
    pub(super) version: HistoryVersion,
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
fn append(doc: &Document, saved: &HistoryVersion, meta: Metadata) -> Result<Option<Vec<u8>>> {
    let bytes = doc.history_updates_since(saved).map_err(failed)?;
    Ok(within(meta.rows + 1, meta.stored() + bytes.len() as i64).then_some(bytes))
}
/// A checkpoint that fits: the whole history, or only the current state when the history
/// does not fit. The live document keeps its history, so this
/// session's undo still works; only the file forgets it. A shared document always keeps its
/// whole history: another replica's edits can depend on any of it.
fn checkpoint(doc: &mut Document, shared: bool) -> Result<Option<Vec<u8>>> {
    let fits = |bytes: &[u8]| within(0, checkpoint_row(bytes.len()));
    let bytes = doc.checkpoint().map_err(failed)?;
    if fits(&bytes) || shared {
        return Ok(fits(&bytes).then_some(bytes));
    }
    trimmed(doc, fits)
}
/// A checkpoint keeping only the history since `start`, when `accept` takes its bytes.
fn trimmed(doc: &Document, accept: impl Fn(&[u8]) -> bool) -> Result<Option<Vec<u8>>> {
    let bytes = doc.current_history_checkpoint().map_err(failed)?;
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
        let (meta, saved, shared, uncertain) = {
            let account = lock(&self.account);
            (account.meta, account.saved.clone(), account.shared, account.checkpoint_required)
        };
        let version = doc.history_version();
        if version == saved && !uncertain {
            return Ok(None);
        }
        // A close checkpoint can reach disk while the original owner remains live
        // after a lost acknowledgment. Its history cannot append to the rebuilt base.
        let replace = uncertain || !version.includes(&saved);
        // Prefer a checkpoint when the log is due for one.
        let due = meta.rows >= CHECKPOINT_ROWS || meta.update_bytes >= CHECKPOINT_BYTES;
        let order: &[bool] = if replace {
            &[true]
        } else if due {
            &[true, false]
        } else {
            &[false, true]
        };
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
        let bytes =
            if shared { checkpoint(doc, true)? } else { trimmed(doc, |bytes| within(0, checkpoint_row(bytes.len())))? }
                .ok_or(Error::Full)?;
        Ok(SaveJob { rows: Rows::Checkpoint(bytes), version: doc.history_version() })
    }

    /// The checkpoint to write as the owner closes, after its last save: a session that
    /// edited a document larger than `TRIM_BYTES` trims its history, rebuilding an
    /// independent one. None when nothing would shrink. The still-live document keeps its session undo until close succeeds.
    pub fn close_job(&self, doc: &mut Document) -> Result<Option<SaveJob>> {
        let (meta, changed, shared) = {
            let account = lock(&self.account);
            (account.meta, account.changed, account.shared)
        };
        let stored = meta.stored();
        if !changed || shared || stored <= TRIM_BYTES {
            return Ok(None);
        }
        // Only a final unshared close may start an independent history.
        // Ordinary saves and explicit checkpoints retain the live CRDT identities.
        let bytes = doc.clean_checkpoint().map_err(failed)?;
        if bytes.len() as i64 >= stored || !within(0, checkpoint_row(bytes.len())) {
            return Ok(None);
        }
        let version = Document::open(self.app.app.spec(), &bytes, &[]).map_err(failed)?.history_version();
        Ok(Some(SaveJob { rows: Rows::Checkpoint(bytes), version }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuing_after_a_rebuilt_close_write_replaces_history_before_appending() {
        let directory = tempfile::tempdir().unwrap();
        crate::registry::use_folder(&directory.path().join("registry")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/checklist/document");
        let template = directory.path().join("template.pop");
        file::pack(&std::fs::read_to_string(fixture.join("input.json")).unwrap(), &fixture, &template, None).unwrap();
        let path = directory.path().join("document.pop");
        file::create_document(&template, &path).unwrap();
        let store = Store::open(&path, Mode::Document).unwrap();
        let mut live = store.document().unwrap();
        let set = |document: &mut Document, title: &str| {
            let batch =
                serde_json::from_value(serde_json::json!({"intents":[{"type":"set","path":["title"],"value":title}]}))
                    .unwrap();
            document.apply_batch(batch, pop_core::Origin::Page).unwrap();
        };
        set(&mut live, "Before close");
        store.write(&store.job(&mut live).unwrap().unwrap()).unwrap();
        let live_version = live.version();

        // The close checkpoint reached disk, but its acknowledgment did not reach
        // the owner. It must retain the original live document and session undo.
        let bytes = live.clean_checkpoint().unwrap();
        let rebuilt = Document::open(store.app().app.spec(), &bytes, &[]).unwrap();
        let close = SaveJob { rows: Rows::Checkpoint(bytes), version: rebuilt.history_version() };
        store.write(&close).unwrap();
        assert_eq!(live.version(), live_version);
        assert!(live.undo().unwrap().publication.is_some());
        assert!(live.redo().unwrap().publication.is_some());
        set(&mut live, "Continued after lost close acknowledgment");
        let continued = store.job(&mut live).unwrap().unwrap();
        assert!(continued.is_checkpoint(), "old live history cannot append onto the rebuilt durable history");
        store.write(&continued).unwrap();
        // Any transaction error may hide a successful commit at the API boundary.
        // A real SQL refusal exercises the conservative retry fence without a fault hook.
        set(&mut live, "Continued after uncertain write");
        let attempt = store.job(&mut live).unwrap().unwrap();
        assert!(!attempt.is_checkpoint(), "ordinary live deltas still append");
        {
            let backing = lock(&store.backing);
            backing.conn.as_ref().unwrap().execute_batch(
                "CREATE TEMP TRIGGER refuse_history BEFORE INSERT ON history BEGIN SELECT RAISE(ABORT, 'save refused'); END;",
            ).unwrap();
        }
        assert!(store.write(&attempt).is_err());
        {
            let backing = lock(&store.backing);
            backing.conn.as_ref().unwrap().execute_batch("DROP TRIGGER refuse_history;").unwrap();
        }
        let retry = store.job(&mut live).unwrap().unwrap();
        assert!(retry.is_checkpoint(), "an uncertain write must fence the next append");
        store.write(&retry).unwrap();
        store.close().unwrap();
        let reopened = Store::open(&path, Mode::Document).unwrap();
        assert_eq!(reopened.document().unwrap().reading().unwrap().value["title"], "Continued after uncertain write");
    }
}
