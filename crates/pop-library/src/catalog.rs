//! Discovery hints never replace pending local intent or establish access denial.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Discovery,
    Authority,
}

pub(super) enum Resolution {
    Pending,
    Denied,
    Live { title: String, template: String, role: DocumentRole },
}

impl Library {
    pub fn receive_catalog(&self, entries: &[DocumentEntry]) -> Result<CatalogOutcome> {
        let _change = self.changing();
        self.cloud()?;
        let _projection = self.projections.lock().unwrap();
        self.apply_catalog(entries, Source::Discovery)
    }

    pub(super) fn apply_authoritative_catalog(&self, entries: &[DocumentEntry]) -> Result<CatalogOutcome> {
        self.cloud()?;
        let _projection = self.projections.lock().unwrap();
        self.apply_catalog(entries, Source::Authority)
    }

    // Caller serializes projection and local catalog intent with `projections`.
    fn apply_catalog(&self, entries: &[DocumentEntry], source: Source) -> Result<CatalogOutcome> {
        let mut outcome = CatalogOutcome::default();
        let mut accepted = Vec::new();
        for entry in entries {
            if Self::valid(&entry.document_id).is_err()
                || entry.catalog_revision > i64::MAX as u64
                || entry.title.len() > 4096
                || entry.template_id.len() > 1024
            {
                outcome.rejected = outcome.rejected.saturating_add(1);
                continue;
            }
            let protected: bool = self.inner.lock().unwrap().db.query_row(
                "SELECT EXISTS(SELECT 1 FROM catalog_writes WHERE account=?1 AND id=?2) OR EXISTS(SELECT 1 FROM documents WHERE account=?1 AND id=?2 AND pending_enable=1)",
                params![self.account, entry.document_id], |row| row.get(0),
            )?;
            // Pending metadata wins over hints, but a tombstone still needs an
            // authority check. A definite denial overrides catalog intent only
            // after local work has been preserved below.
            if protected && !entry.deleted {
                continue;
            }
            let prior = self
                .inner
                .lock()
                .unwrap()
                .db
                .query_row(
                    "SELECT deleted,path,title FROM documents WHERE account=?1 AND id=?2",
                    params![self.account, entry.document_id],
                    |row| Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
                )
                .optional()?;
            if source == Source::Discovery && (entry.deleted || prior.as_ref().is_some_and(|(deleted, _, _)| *deleted))
            {
                self.inner.lock().unwrap().db.execute(
                    "INSERT INTO catalog_tombstones VALUES(?1,?2,?3) ON CONFLICT(account,id) DO UPDATE SET payload=excluded.payload",
                    params![self.account, entry.document_id, serde_json::to_string(entry).map_err(failed)?],
                )?;
                outcome.deferred = outcome.deferred.saturating_add(1);
                continue;
            }
            if entry.deleted
                && let Some((false, path, title)) = prior
                && paths::resolve(&self.root, &path)?.is_file()
            {
                if let Some(client) = self.connection(&entry.document_id) {
                    client.pause();
                }
                self.account_recovery(&entry.document_id, &format!("Recovered edits — {title}"))?;
            }
            accepted.push(entry);
        }
        let mut inner = self.inner.lock().unwrap();
        let tx = inner.db.transaction()?;
        for entry in &accepted {
            let path: String = tx
                .query_row("SELECT path FROM documents WHERE id=?1 AND account=''", [&entry.document_id], |row| {
                    row.get(0)
                })
                .optional()?
                .map_or_else(|| paths::store(&self.root, &self.cached_path(&entry.document_id)), Ok)?;
            let role = stored_role(entry.role);
            tx.execute("INSERT INTO documents(account,id,title,template,path,role,revision,deleted,catalog_seen,opened) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,1,coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0)) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role=excluded.role,revision=excluded.revision,deleted=excluded.deleted,catalog_seen=1",params![self.account,entry.document_id,entry.title,entry.template_id,path,role,i64::try_from(entry.catalog_revision).map_err(failed)?,entry.deleted])?;
            if entry.deleted && source == Source::Authority {
                tx.execute(
                    "DELETE FROM catalog_writes WHERE account=?1 AND id=?2",
                    params![self.account, entry.document_id],
                )?;
            }
            tx.execute(
                "DELETE FROM catalog_tombstones WHERE account=?1 AND id=?2",
                params![self.account, entry.document_id],
            )?;
            outcome.applied = outcome.applied.saturating_add(1);
        }
        tx.commit()?;
        let stopped: Vec<_> = accepted
            .iter()
            .filter(|entry| entry.deleted)
            .filter_map(|entry| inner.clients.remove(&entry.document_id))
            .collect();
        drop(inner);
        for client in stopped {
            client.pause();
            client.flush().map_err(failed)?;
        }
        Ok(outcome)
    }

    pub(super) fn resolve_catalog_tombstones(&self, token: &str) -> Result<()> {
        self.resolve_catalog_with(|id| {
            let Some(access) = pop_sharing_client::authorized_access(id, token).map_err(failed)? else {
                return Ok(Resolution::Denied);
            };
            if access.document != id {
                return Err(failed("Access response belongs to another document"));
            }
            let Some(metadata) = pop_sharing_client::ready_document_metadata(id, token).map_err(failed)? else {
                return Ok(Resolution::Pending);
            };
            Ok(Resolution::Live { title: metadata.title, template: metadata.template_id, role: access.role })
        })
    }

    pub(super) fn resolve_catalog_with(&self, resolve: impl Fn(&str) -> Result<Resolution>) -> Result<()> {
        let generation = self.operation()?;
        let hints = {
            let inner = self.inner.lock().unwrap();
            inner
                .db
                .prepare("SELECT id,payload FROM catalog_tombstones WHERE account=?1")?
                .query_map([&self.account], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut failure = None;
        for (id, payload) in hints {
            let resolved = (|| {
                let mut entry: DocumentEntry = serde_json::from_str(&payload).map_err(failed)?;
                let accessible = match resolve(&id)? {
                    Resolution::Pending => {
                        self.current_operation(generation)?;
                        return Ok(());
                    }
                    Resolution::Denied => {
                        entry.deleted = true;
                        false
                    }
                    Resolution::Live { title, template, role } => {
                        entry.title = title;
                        entry.template_id = template;
                        entry.role = role;
                        entry.deleted = false;
                        true
                    }
                };
                self.current_operation(generation)?;
                let _projection = self.projections.lock().unwrap();
                let current: Option<String> = self
                    .inner
                    .lock()
                    .unwrap()
                    .db
                    .query_row(
                        "SELECT payload FROM catalog_tombstones WHERE account=?1 AND id=?2",
                        params![self.account, id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if current.as_deref() != Some(&payload) {
                    return Ok(());
                }
                let applied = self.apply_catalog(&[entry], Source::Authority)?;
                if applied.applied > 0 {
                    self.queue_catalog_locked(&id, !accessible)?;
                }
                Ok(())
            })();
            if let Err(error) = resolved {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
