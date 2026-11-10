//! Authoritative account feed projection into the offline library index.
use super::*;
use crate::transport::account::{Account, LibraryUpdate};
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
                    index::listed_ids(&inner.db, &self.account)?
                };
                for id in absent {
                    if !present.contains(&id) {
                        self.remove_entry(&id)?;
                    }
                }
                for entry in entries {
                    self.upsert_entry(entry, true)?;
                }
                // The next complete snapshot resolves absence instead of retaining a
                // protected local selection forever.
                index::list_selections(&self.inner.lock().unwrap().db, &self.account)?;
                index::unlist_removed(&self.inner.lock().unwrap().db, &self.account)?;
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
        let prior = index::prior(&inner.db, &self.account, &entry.document)?;
        if let Some((old, deleted, pending, listed)) = prior
            && (pending || (!snapshot && (deleted && listed || old >= revision)))
        {
            return Ok(());
        }
        let path = index::local_path(&inner.db, &entry.document)?
            .map_or_else(|| paths::store(&self.root, &self.cached_path(&entry.document)), Ok)?;
        index::upsert_listed(&inner.db, &self.account, &entry, &path, revision)
    }

    fn remove_entry(&self, id: &str) -> Result<()> {
        let entry = {
            let inner = self.inner.lock().unwrap();
            if !index::is_live(&inner.db, &self.account, id)? {
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
        index::mark_removed(&self.inner.lock().unwrap().db, &self.account, id)
    }
}
