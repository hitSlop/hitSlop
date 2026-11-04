//! One durable private-CloudKit epoch candidate per document; CAS owns publication.
use crate::{CatalogAuthority, CatalogCheckpointProof, Inner, Library, Result, Upload, failed, snapshot};
use pop_runtime::owner::{Reply, Request};
use pop_storage::store::{EpochLocation, Mode, Pressure, Store};
use rusqlite::{OptionalExtension, params};
use std::path::{Path, PathBuf};

struct Candidate {
    transition: String,
    source: String,
    version: Option<String>,
    filename: String,
    fields: Option<Vec<u8>>,
}
impl Library {
    fn candidate(inner: &Inner, id: &str) -> Result<Option<Candidate>> {
        Ok(inner
            .db
            .query_row(
                "SELECT transition,source_version,version,filename,fields FROM catalog_candidates WHERE document_id=?1",
                [id],
                |row| {
                    Ok(Candidate {
                        transition: row.get(0)?,
                        source: row.get(1)?,
                        version: row.get(2)?,
                        filename: row.get(3)?,
                        fields: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }
    pub fn has_pending_checkpoint(&self, id: &str) -> Result<bool> {
        Self::valid_id(id)?;
        let inner = self.inner.lock().unwrap();
        let actionable = matches!(
            Self::authority_inner(&inner, id)?,
            CatalogAuthority::Snapshot { receipt: None } | CatalogAuthority::Checkpoint { .. }
        );
        // A newer committed relay authority supersedes this attempted publication.
        // Keep its bytes until canonical installation confirms recovery, but do not
        // make joining that authority wait on an upload which is no longer legal.
        Ok(actionable && Self::candidate(&inner, id)?.is_some())
    }
    pub fn pending_checkpoint_new_record(&self, id: &str) -> Result<bool> {
        Self::valid_id(id)?;
        Ok(Self::candidate(&self.inner.lock().unwrap(), id)?.is_some_and(|candidate| candidate.fields.is_none()))
    }
    pub(crate) fn checkpoint_pending_inner(inner: &Inner, id: &str) -> Result<bool> {
        Ok(Self::candidate(inner, id)?.is_some())
    }
    pub(crate) fn own_checkpoint_source(
        inner: &Inner,
        id: &str,
        authority: &CatalogAuthority,
    ) -> Result<Option<String>> {
        let CatalogAuthority::Checkpoint { proof: CatalogCheckpointProof { transition, source_version, version } } =
            authority
        else {
            return Ok(None);
        };
        Ok(Self::candidate(inner, id)?
            .filter(|candidate| {
                candidate.transition == *transition
                    && candidate.source == *source_version
                    && candidate.version.as_ref() == Some(version)
            })
            .map(|candidate| candidate.source))
    }
    pub(crate) fn stage_checkpoint(&self, id: &str) -> Result<Option<Upload>> {
        let mut inner = self.inner.lock().unwrap();
        let mut entry = self.entry(&inner, id)?;
        if entry.deleted || entry.local_only {
            return Ok(None);
        }
        let current = Self::authority_inner(&inner, id)?;
        if !matches!(current, CatalogAuthority::Snapshot { receipt: None } | CatalogAuthority::Checkpoint { .. }) {
            return Ok(None);
        }
        let mut candidate = match Self::candidate(&inner, id)? {
            Some(candidate) => candidate,
            None => {
                let saved = Store::open(&self.path(id), Mode::Snapshot)?;
                let exceeds_upload = std::fs::metadata(self.path(id))?.len() > snapshot::MAX_FILE_BYTES;
                if !exceeds_upload
                    && saved.retained_history_bytes()? <= 4 * 1024 * 1024
                    && saved.pressure() == Pressure::Ok
                {
                    return Ok(None);
                }
                let source = self.with_owner(&mut inner, id, |owner| {
                    let Reply::State { reading, .. } = snapshot::request_typed(owner, Request::State)? else {
                        return Err(failed("Missing checkpoint source"));
                    };
                    Ok(reading.version)
                })?;
                let transition = crate::random_hex(16);
                let filename = format!("{id}-{transition}.pop");
                let fields: Option<Vec<u8>> =
                    inner.db.query_row("SELECT fields FROM documents WHERE id=?1", [id], |row| row.get(0))?;
                inner.db.execute("INSERT INTO catalog_candidates(document_id,transition,source_version,filename,fields) VALUES(?1,?2,?3,?4,?5)", params![id,transition,source,filename,fields])?;
                Candidate { transition, source, version: None, filename, fields }
            }
        };
        if !crate::catalog::nonce(&candidate.transition)
            || candidate.filename != format!("{id}-{}.pop", candidate.transition)
        {
            return Err(failed("Invalid pending checkpoint location"));
        }
        let path = self.root.join("candidates").join(&candidate.filename);
        if candidate.version.is_none() {
            // No file/version has ever been eligible for a CAS. An interrupted
            // reservation may safely follow the current owner instead of fencing
            // every later edit behind its obsolete expected version.
            let source = self.with_owner(&mut inner, id, |owner| {
                let Reply::State { reading, .. } = snapshot::request_typed(owner, Request::State)? else {
                    return Err(failed("Missing checkpoint source"));
                };
                Ok(reading.version)
            })?;
            if source != candidate.source {
                if path.symlink_metadata().is_ok() {
                    // Only an accepted, unpublished derived file at our controlled
                    // reservation may be replaced, and never behind a live writer.
                    let metadata = std::fs::symlink_metadata(&path)?;
                    if !metadata.is_file() || metadata.file_type().is_symlink() {
                        return Err(failed("Invalid pending checkpoint file"));
                    }
                    let lease = pop_storage::registry::Lease::acquire(&path)?;
                    let unpublished = Store::open(&path, Mode::Snapshot)?;
                    unpublished.document()?;
                    let original = Store::open(&self.path(id), Mode::Snapshot)?;
                    if unpublished.app().document_uuid.as_deref() != Some(id)
                        || unpublished.app_digest()? != original.app_digest()?
                    {
                        return Err(failed("Unpublished checkpoint belongs to another document"));
                    }
                    drop(unpublished);
                    lease.check()?;
                    std::fs::remove_file(&path)?;
                }
                candidate.source = source;
                candidate.fields =
                    inner.db.query_row("SELECT fields FROM documents WHERE id=?1", [id], |row| row.get(0))?;
                inner.db.execute("UPDATE catalog_candidates SET source_version=?2,fields=?3 WHERE document_id=?1 AND version IS NULL", params![id,candidate.source,candidate.fields])?;
            }
        }
        if !path.exists() {
            self.with_owner(&mut inner, id, |owner| {
                let reply = snapshot::request_typed(
                    owner,
                    Request::PrepareEpoch {
                        destination: EpochLocation::Path(path.clone()),
                        expected_version: candidate.source.clone(),
                        required_heads: None,
                    },
                )?;
                let Reply::EpochCandidate { source_version, path: actual, .. } = reply else {
                    return Err(failed("Missing prepared checkpoint"));
                };
                if source_version != candidate.source || actual.native_path() != path {
                    return Err(failed("Checkpoint preparation changed its binding"));
                }
                Ok(())
            })?;
        }
        let info = self.inspect(&path)?;
        let original = Store::open(&self.path(id), Mode::Snapshot)?;
        if info.id != id
            || info.app_digest != original.app_digest()?
            || candidate.version.as_ref().is_some_and(|version| version != &info.version)
        {
            return Err(failed("Pending checkpoint no longer matches its document"));
        }
        let authority = CatalogAuthority::Checkpoint {
            proof: CatalogCheckpointProof {
                transition: candidate.transition,
                source_version: candidate.source,
                version: info.version.clone(),
            },
        };
        let encoded = authority.encode()?;
        let tx = inner.db.transaction()?;
        tx.execute("UPDATE catalog_candidates SET version=?2 WHERE document_id=?1", params![id, info.version])?;
        tx.execute("INSERT INTO catalog(id,committed,pending) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET pending=excluded.pending",params![id,current.encode()?,encoded])?;
        tx.commit()?;
        entry.version = info.version;
        entry.artwork_version.clear();
        entry.asset_revision = snapshot::asset_revision(&entry.version, "", &info.artwork_digest);
        Ok(Some(Upload { entry, asset_path: Some(path.to_string_lossy().into()), fields: candidate.fields, authority }))
    }
    pub(crate) fn retire_checkpoint(inner: &mut Inner, id: &str) -> Result<Option<PathBuf>> {
        let candidate = Self::candidate(inner, id)?;
        if candidate.as_ref().is_some_and(|candidate| {
            !crate::catalog::nonce(&candidate.transition)
                || candidate.filename != format!("{id}-{}.pop", candidate.transition)
        }) {
            return Err(failed("Invalid pending checkpoint location"));
        }
        let filename = candidate.map(|candidate| candidate.filename);
        let clear = inner
            .db
            .query_row("SELECT pending FROM catalog WHERE id=?1", [id], |row| row.get::<_, Option<String>>(0))
            .optional()?
            .flatten()
            .map(|json| CatalogAuthority::decode(crate::CATALOG_FORMAT, Some(&json)))
            .transpose()?
            .is_some_and(|authority| matches!(authority, CatalogAuthority::Checkpoint { .. }));
        inner.db.execute("DELETE FROM catalog_candidates WHERE document_id=?1", [id])?;
        if clear {
            inner.db.execute("UPDATE catalog SET pending=NULL WHERE id=?1", [id])?;
        }
        Ok(filename.map(PathBuf::from))
    }
    pub(crate) fn candidate_is_retained(&self, path: &Path) -> Result<bool> {
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(failed("Invalid candidate path"));
        };
        Ok(self.inner.lock().unwrap().db.query_row(
            "SELECT EXISTS(SELECT 1 FROM catalog_candidates WHERE filename=?1)",
            [filename],
            |row| row.get(0),
        )?)
    }
}
