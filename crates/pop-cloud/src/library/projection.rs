//! Entries as callers see them: index rows joined with file and connection state.
use super::*;
use pop_contracts::relay::SyncPhase;

impl Library {
    pub(super) fn row(&self, id: &str) -> Result<Entry> {
        let generation = self.generation.load(Ordering::Acquire);
        Self::valid(id)?;
        let (row, client) = {
            let inner = pop_core::lock(&self.inner);
            (IndexedDocument::one(&inner.db, &self.account(), id)?, inner.clients.get(id).cloned())
        };
        let entry = self.project(row, client.as_deref())?;
        self.current_operation(generation)?;
        Ok(entry)
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
        let confirmed = row.confirmed;
        let mut status = client.map(Client::status);
        let client_phase = status.as_ref().map(|status| status.phase);
        let client_synced = client_phase == Some(SyncPhase::Synced);
        if client_synced && let Some(confirmed) = &confirmed {
            version = confirmed.clone();
        }
        let dirty = role.is_some()
            && (row.pending
                || downloaded
                    && (confirmed.as_deref() != Some(&version)
                        || (!client_synced && stamp.as_deref() != Some(&row.file_stamp))
                        || client_phase.is_some_and(|phase| phase != SyncPhase::Synced)));
        if status.is_none() && role.is_some() {
            let feed = pop_core::lock(&self.feed).clone();
            status = feed.map(|feed| feed.status()).filter(|status| status.phase != SyncPhase::Synced);
        }
        if status.is_none() && role.is_some() && row.pending {
            status = Some(crate::client::ClientStatus {
                phase: if row.problem.is_some() { SyncPhase::Paused } else { SyncPhase::Syncing },
                pause_kind: row.problem.as_ref().map(|_| pop_contracts::relay::PauseKind::Failed),
                detail: row.problem.clone().unwrap_or_else(|| "Syncing…".into()),
            });
        }
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
            problem: row.problem,
            role,
            phase: status.as_ref().map_or(SyncPhase::Offline, |status| status.phase),
            pause_kind: status.as_ref().and_then(|status| status.pause_kind),
            detail: status.map_or_else(|| "Offline".into(), |status| status.detail),
            revision: u64::try_from(row.revision).map_err(|e| failed(e.to_string()))?,
        })
    }
    pub fn entries(&self) -> Result<Vec<Entry>> {
        let generation = self.generation.load(Ordering::Acquire);
        self.check()?;
        let (rows, clients) = {
            let inner = pop_core::lock(&self.inner);
            (IndexedDocument::all(&inner.db, &self.account())?, inner.clients.clone())
        };
        let entries = rows
            .into_iter()
            .map(|row| {
                let client = clients.get(&row.id).map(Arc::as_ref);
                self.project(row, client)
            })
            .collect::<Result<Vec<_>>>()?;
        self.current_operation(generation)?;
        Ok(entries)
    }
    pub fn account_paths(&self) -> Result<Vec<String>> {
        let stored = index::account_paths(&pop_core::lock(&self.inner).db)?;
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
