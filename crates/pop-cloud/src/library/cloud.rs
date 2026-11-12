//! Cloud membership: enabling sync, downloads, sharing, joining and leaving.
use super::*;
use crate::transport::account::LibraryUpdate;

impl Library {
    pub fn enable_sync(&self, account: &str, id: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
        let generation = self.operation_for(account)?;
        let entry = self.row(id)?;
        {
            let mut inner = pop_core::lock(&self.inner);
            self.current_operation(generation)?;
            index::begin_enable(
                &mut inner.db,
                &self.account(),
                id,
                &entry.title,
                &entry.template,
                &paths::store(&self.root, Path::new(&entry.path))?,
                &entry.version,
            )?;
        }
        self.notify();
        let outcome = self.with_owner(id, |owner| {
            crate::cloud::bootstrap(owner, token, self.receipt(id, generation)?).map_err(failed)?;
            self.current_operation(generation)?;
            crate::cloud::update_document(id, &entry.title, &entry.template, token).map_err(failed)?;
            // Sharing is a presentation cache. Bootstrap completion must not
            // depend on another request made only to populate that optional hint.
            self.current_operation(generation)
        });
        if let Err(error) = outcome {
            let inner = pop_core::lock(&self.inner);
            self.current_operation(generation)?;
            index::record_problem(&inner.db, &self.account(), id, &error.to_string())?;
            return Err(error);
        }
        let live = {
            let mut inner = pop_core::lock(&self.inner);
            self.current_operation(generation)?;
            index::finish_enable(&mut inner.db, &self.account(), id)?;
            inner.token = Some(token.into());
            inner.live.get(id).and_then(Weak::upgrade)
        };
        if let Some(owner) = live {
            self.attach(id, owner, token)?;
        }
        self.row(id)
    }
    pub fn download(&self, account: &str, id: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
        let generation = self.operation_for(account)?;
        let entry = self.row(id)?;
        if entry.deleted {
            return Err(failed("Cloud document is no longer available"));
        }
        let access = crate::cloud::authorized_access(id, token).map_err(failed)?;
        self.current_operation(generation)?;
        let Some(access) = access else {
            self.apply_at(LibraryUpdate::Remove(id.into()), generation)?;
            return Err(failed("Cloud document access was removed; local edits were retained"));
        };
        self.remember_access(id, &access, generation)?;
        if entry.downloaded {
            return self.row(id);
        }
        let resolved = crate::resolution::resolve(id, token)
            .map_err(failed)?
            .ok_or_else(|| failed("Cloud document is not ready"))?;
        self.current_operation(generation)?;
        resolved
            .install_if_current(Path::new(&entry.path), self.receipt(id, generation)?, || {
                self.current_operation(generation)
            })
            .map_err(failed)?;
        self.current_operation(generation)?;
        self.cache_file(id, None)?;
        self.row(id)
    }
    fn remember_access(&self, id: &str, access: &AccessState, generation: u64) -> Result<()> {
        self.cloud()?;
        Self::valid(id)?;
        if access.document != id {
            return Err(failed("Access response belongs to another document"));
        }
        let inner = pop_core::lock(&self.inner);
        self.current_operation(generation)?;
        index::remember_sharing(&inner.db, &self.account(), &access.document, access.sharing)
    }
    pub fn access(&self, account: &str, id: &str, token: &str) -> Result<AccessState> {
        let _change = self.changing();
        let generation = self.operation_for(account)?;
        let access = crate::cloud::access(id, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access, generation)?;
        Ok(access)
    }
    pub fn sharing(&self, account: &str, id: &str, enabled: bool, token: &str) -> Result<AccessState> {
        let _change = self.changing();
        let generation = self.operation_for(account)?;
        let access = crate::cloud::set_sharing(id, enabled, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access, generation)?;
        Ok(access)
    }
    pub fn join(&self, account: &str, link: &str, token: &str) -> Result<Entry> {
        if self.account().is_empty() || token.is_empty() {
            return Err(Error::Library { issue: Issue::SignInRequired, message: "Sign in to join".into() });
        }
        let _change = self.changing();
        let generation = self.operation_for(account)?;
        let joined = crate::cloud::join_member(link, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_join(&joined, generation)?;
        self.download(account, &joined.access.document, token)
    }
    /// The document decides the role: an owner opening their own invitation stays the owner,
    /// so catalog deletion later deletes rather than leaves.
    fn remember_join(&self, joined: &JoinedDocument, generation: u64) -> Result<()> {
        let id = &joined.access.document;
        let path = self.cached_path(id);
        let inner = pop_core::lock(&self.inner);
        self.current_operation(generation)?;
        index::upsert_joined(&inner.db, &self.account(), joined, &paths::store(&self.root, &path)?)
    }
    pub fn leave(&self, account: &str, id: &str, token: &str) -> Result<()> {
        self.remove_cloud(account, id, token, false)
    }
    pub fn delete_everywhere(&self, account: &str, id: &str, token: &str) -> Result<()> {
        self.remove_cloud(account, id, token, true)
    }
    fn remove_cloud(&self, account: &str, id: &str, token: &str, delete: bool) -> Result<()> {
        let _change = self.changing();
        let generation = self.operation_for(account)?;
        self.unregister(id)?;
        if !delete && self.row(id)?.downloaded {
            self.local_copy(id, "Retained local copy")?;
        }
        pop_core::lock(&self.departing).insert((generation, id.into()));
        let result = (|| {
            index::mark_departing(&pop_core::lock(&self.inner).db, &self.account(), id)?;
            let result = if delete { crate::cloud::delete_document(id, token) } else { crate::cloud::leave(id, token) };
            self.current_operation(generation)?;
            if let Err(error) = result {
                index::restore_departing(&pop_core::lock(&self.inner).db, &self.account(), id)?;
                return Err(failed(error));
            }
            Ok(())
        })();
        pop_core::lock(&self.departing).remove(&(generation, id.into()));
        // Discard the connection's queued frames; its next complete snapshot is authoritative.
        if let Some(feed) = pop_core::lock(&self.feed).as_ref() {
            feed.wake();
        }
        result
    }
    pub fn remove_download(&self, id: &str) -> Result<()> {
        let _change = self.changing();
        self.cloud()?;
        let entry = {
            let inner = pop_core::lock(&self.inner);
            if inner.live.get(id).and_then(Weak::upgrade).is_some() {
                return Err(Error::Library {
                    issue: Issue::Busy,
                    message: "Close this pop before removing its download".into(),
                });
            }
            drop(inner);
            self.row(id)?
        };
        if entry.local_only {
            return Err(failed("Local-only documents have no cloud download"));
        }
        if !entry.downloaded {
            return Ok(());
        }
        if !Path::new(&entry.path).starts_with(self.root.join("accounts").join(self.scope())) {
            return Err(failed("This document is outside the cloud cache; its original file is retained"));
        }
        let confirmed = {
            let inner = pop_core::lock(&self.inner);
            IndexedDocument::one(&inner.db, &self.account(), id)?.confirmed
        };
        let saved = crate::local::inspect(Path::new(&entry.path))?;
        if confirmed.as_deref() != Some(&saved.version) {
            return Err(failed("This download has unconfirmed edits; synchronize it before removal"));
        }
        self.unregister(id)?;
        file::delete_document(Path::new(&entry.path))?;
        Ok(())
    }
}
