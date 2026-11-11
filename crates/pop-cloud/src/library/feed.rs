//! Authoritative account feed projection into the offline library index.
use super::*;
use crate::transport::account::{Account, LibraryUpdate};
use pop_sharing_protocol::library::AccountEntry;
use std::collections::HashSet;

impl Library {
    pub(super) fn connect_feed(self: &Arc<Self>, token: &str) -> Result<()> {
        let generation = self.operation()?;
        let mut feed = pop_core::lock(&self.feed);
        self.current_operation(generation)?;
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
                        library.apply_at(update, generation)
                    } else {
                        Err("Library detached".into())
                    }
                }),
                self.notice.clone(),
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
        let generation = self.operation()?;
        self.apply_at(update, generation)
    }
    pub(super) fn apply_at(&self, update: LibraryUpdate, generation: u64) -> Result<()> {
        let _change = self.changing();
        self.current_operation(generation)?;
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
                    let inner = pop_core::lock(&self.inner);
                    index::listed_ids(&inner.db, &self.account())?
                };
                for id in absent {
                    if !present.contains(&id) {
                        self.remove_entry(&id, generation)?;
                    }
                }
                for entry in entries {
                    self.upsert_entry(entry, true, generation)?;
                }
                // The next complete snapshot resolves absence instead of retaining a
                // protected local selection forever.
                let inner = pop_core::lock(&self.inner);
                self.current_operation(generation)?;
                index::list_selections(&inner.db, &self.account())?;
                index::unlist_removed(&inner.db, &self.account())?;
            }
            LibraryUpdate::Upsert(entry) => {
                entry.validate().map_err(failed)?;
                self.upsert_entry(entry, false, generation)?;
            }
            LibraryUpdate::Remove(id) => {
                Self::valid(&id)?;
                self.remove_entry(&id, generation)?;
            }
        }
        Ok(())
    }

    fn upsert_entry(&self, entry: AccountEntry, snapshot: bool, generation: u64) -> Result<()> {
        if pop_core::lock(&self.departing).contains(&(generation, entry.document.clone())) {
            return Ok(());
        }
        let revision = i64::try_from(entry.revision).map_err(|e| failed(e.to_string()))?;
        let inner = pop_core::lock(&self.inner);
        self.current_operation(generation)?;
        let prior = index::prior(&inner.db, &self.account(), &entry.document)?;
        if let Some((old, deleted, pending, listed)) = prior
            && (pending || (!snapshot && (deleted && listed || old >= revision)))
        {
            return Ok(());
        }
        let path = index::local_path(&inner.db, &entry.document)?
            .map_or_else(|| paths::store(&self.root, &self.cached_path(&entry.document)), Ok)?;
        index::upsert_listed(&inner.db, &self.account(), &entry, &path, revision)
    }

    fn remove_entry(&self, id: &str, generation: u64) -> Result<()> {
        let entry = {
            let inner = pop_core::lock(&self.inner);
            if !index::is_live(&inner.db, &self.account(), id)? {
                return Ok(());
            }
            drop(inner);
            self.row(id)?
        };
        self.current_operation(generation)?;
        if let Some(client) = self.connection(id) {
            client.pause();
            client.flush().map_err(failed)?;
        }
        if entry.downloaded {
            self.account_recovery(id, &format!("Recovered edits — {}", entry.title))?;
        }
        let inner = pop_core::lock(&self.inner);
        self.current_operation(generation)?;
        if index::prior(&inner.db, &self.account(), id)?
            .is_some_and(|(revision, _, _, _)| revision != entry.revision as i64)
        {
            return Ok(());
        }
        index::mark_removed(&inner.db, &self.account(), id)
    }
}
