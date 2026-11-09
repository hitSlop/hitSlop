//! Authoritative account feed projection into the offline library index.
use super::*;
use pop_sharing_client::{Account, LibraryUpdate};
use pop_sharing_protocol::library::AccountEntry;
use std::collections::HashSet;

impl Library {
    pub(super) fn connect_feed(self: &Arc<Self>, token: &str) -> Result<()> {
        let mut feed = self.feed.lock().unwrap();
        if !self.feed_active.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(feed) = feed.as_ref() {
            feed.set_identity(token);
            return Ok(());
        }
        let weak = Arc::downgrade(self);
        *feed = Some(
            Account::start(
                token,
                Arc::new(move |update| {
                    if let Some(library) = weak.upgrade() {
                        library.apply(update).map_err(|error| error.message)
                    } else {
                        Err("Library detached".into())
                    }
                }),
            )
            .map_err(failed)?,
        );
        Ok(())
    }

    /// Apply one complete account snapshot or a subsequent update.
    ///
    /// # Errors
    /// Refuses detached libraries, invalid metadata and failed recovery saves.
    pub fn apply(&self, update: LibraryUpdate) -> Result<()> {
        let _change = self.changing();
        self.cloud()?;
        let _projection = self.projections.lock().unwrap();
        self.cloud()?;
        match update {
            LibraryUpdate::Snapshot(entries) => {
                let mut present = HashSet::new();
                for entry in &entries {
                    entry.validate().map_err(failed)?;
                    if !present.insert(entry.document.clone()) {
                        return Err(failed("Duplicate library document"));
                    }
                }
                let absent = {
                    let inner = self.inner.lock().unwrap();
                    inner
                        .db
                        .prepare(
                            "SELECT id FROM documents WHERE account=?1 AND listed=1 AND deleted=0 AND pending_enable=0",
                        )?
                        .query_map([&self.account], |r| r.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?
                };
                for id in absent {
                    if !present.contains(&id) {
                        self.remove_entry(&id)?;
                    }
                }
                for entry in entries {
                    self.upsert_entry(entry, true)?;
                }
                // An upload can finish while an earlier snapshot is queued. Protect an
                // unlisted local selection for that first snapshot, then let the next
                // complete snapshot resolve absence instead of retaining it forever.
                self.inner.lock().unwrap().db.execute("UPDATE documents SET listed=1 WHERE account=?1 AND listed=0 AND pending_enable=0 AND role IS NOT NULL AND deleted=0", [&self.account])?;
                self.inner
                    .lock()
                    .unwrap()
                    .db
                    .execute("UPDATE documents SET listed=0 WHERE account=?1 AND deleted=1", [&self.account])?;
            }
            LibraryUpdate::Upsert(entry) => {
                entry.validate().map_err(failed)?;
                self.upsert_entry(entry, false)?;
            }
            LibraryUpdate::Remove(id) => {
                Self::valid(&id)?;
                self.remove_entry(&id)?;
            }
        }
        Ok(())
    }

    fn upsert_entry(&self, entry: AccountEntry, snapshot: bool) -> Result<()> {
        if self.departing.lock().unwrap().contains(&entry.document) {
            return Ok(());
        }
        let revision = i64::try_from(entry.revision).map_err(failed)?;
        let inner = self.inner.lock().unwrap();
        let prior = inner
            .db
            .query_row(
                "SELECT revision,deleted,pending_enable,listed FROM documents WHERE account=?1 AND id=?2",
                params![self.account, entry.document],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?, r.get::<_, bool>(2)?, r.get::<_, bool>(3)?)),
            )
            .optional()?;
        if let Some((old, deleted, pending, listed)) = prior
            && (pending || (!snapshot && (deleted && listed || old >= revision)))
        {
            return Ok(());
        }
        let path = inner
            .db
            .query_row("SELECT path FROM documents WHERE account='' AND id=?1", [&entry.document], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .map_or_else(|| paths::store(&self.root, &self.cached_path(&entry.document)), Ok)?;
        inner.db.execute("INSERT INTO documents(account,id,title,template,path,role,revision,listed,opened,sharing_enabled) VALUES(?1,?2,?3,?4,?5,?6,?7,1,coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0),?8) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role=excluded.role,revision=excluded.revision,listed=1,deleted=0,sharing_enabled=excluded.sharing_enabled",
            params![self.account, entry.document, entry.title, entry.template_id, path, stored_role(entry.role), revision, entry.sharing])?;
        Ok(())
    }

    fn remove_entry(&self, id: &str) -> Result<()> {
        let entry = {
            let inner = self.inner.lock().unwrap();
            let exists = inner.db.query_row(
                "SELECT count(*) FROM documents WHERE account=?1 AND id=?2 AND deleted=0",
                params![self.account, id],
                |r| r.get::<_, i64>(0),
            )? > 0;
            if !exists {
                return Ok(());
            }
            self.row(&inner, id)?
        };
        if let Some(client) = self.connection(id) {
            client.pause();
            client.flush().map_err(failed)?;
        }
        if entry.downloaded {
            self.account_recovery(id, &format!("Recovered edits — {}", entry.title))?;
        }
        self.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE documents SET deleted=1,listed=0 WHERE account=?1 AND id=?2", params![self.account, id])?;
        Ok(())
    }
}
