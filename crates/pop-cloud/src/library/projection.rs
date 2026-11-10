//! Entries as callers see them: index rows joined with file and connection state.
use super::*;
use crate::client::Phase;

impl Library {
    pub(super) fn row(&self, inner: &Inner, id: &str) -> Result<Entry> {
        Self::valid(id)?;
        self.project(IndexedDocument::one(&inner.db, &self.account, id)?, inner.clients.get(id).map(Arc::as_ref))
    }
    pub(super) fn project(&self, row: IndexedDocument, client: Option<&Client>) -> Result<Entry> {
        let id = row.id.as_str();
        let resolved_path = paths::resolve(&self.root, &row.path)?;
        let downloaded = resolved_path.is_file();
        let role = match row.role.as_deref() {
            Some("owner") => Some(DocumentRole::Owner),
            Some("editor") => Some(DocumentRole::Editor),
            _ => None,
        };
        let mut version = row.version;
        let stamp = if downloaded { file_stamp(&resolved_path).ok() } else { None };
        let (confirmed, receipt_problem) = if role.is_some() {
            match crate::client::confirmed_saved_version(&self.state_dir(), id) {
                Ok(version) => (version, None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };
        let client_phase = client.map(|client| client.status().phase);
        let client_synced = client_phase == Some(Phase::Synced);
        if client_synced && let Some(confirmed) = &confirmed {
            version = confirmed.clone();
        }
        let dirty = role.is_some()
            && (row.pending
                || downloaded
                    && (confirmed.as_deref() != Some(&version)
                        || (!client_synced && stamp.as_deref() != Some(&row.file_stamp))
                        || client_phase.is_some_and(|phase| phase != Phase::Synced)));
        let asset_revision = crate::local::asset_revision(&version, &row.artwork_version, &row.artwork_digest);
        let artwork_version = if row.artwork_version == version { row.artwork_version } else { String::new() };
        Ok(Entry {
            id: id.into(),
            title: row.title,
            template: row.template,
            path: resolved_path.to_string_lossy().into_owned(),
            version,
            asset_revision,
            artwork_version,
            dirty,
            deleted: row.deleted,
            local_only: role.is_none(),
            account_bound: !row.account.is_empty(),
            downloaded,
            sharing_enabled: row.sharing_enabled,
            inbox: !row.opened && !row.account.is_empty() && role.is_some() && !row.deleted,
            problem: row.problem.or(receipt_problem),
            role,
            revision: u64::try_from(row.revision).map_err(failed)?,
        })
    }
    pub fn entries(&self) -> Result<Vec<Entry>> {
        self.check()?;
        let (rows, clients) = {
            let inner = self.inner.lock().unwrap();
            (IndexedDocument::all(&inner.db, &self.account)?, inner.clients.clone())
        };
        rows.into_iter()
            .map(|row| {
                let client = clients.get(&row.id).map(Arc::as_ref);
                self.project(row, client)
            })
            .collect()
    }
    pub fn account_paths(&self) -> Result<Vec<String>> {
        let stored = index::account_paths(&self.inner.lock().unwrap().db)?;
        stored.into_iter().map(|path| Ok(paths::resolve(&self.root, &path)?.to_string_lossy().into_owned())).collect()
    }
}
pub(super) fn file_stamp(path: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    let mut stamp = format!(
        "{}:{}:{}:{}:{}",
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec()
    );
    let wal = PathBuf::from(format!("{}-wal", path.to_string_lossy()));
    if let Ok(metadata) = std::fs::metadata(wal) {
        stamp.push_str(&format!(":{}:{}:{}", metadata.len(), metadata.mtime(), metadata.mtime_nsec()));
    }
    Ok(stamp)
}
