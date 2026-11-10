//! Cloud membership: enabling sync, downloads, sharing, joining and leaving.
use super::*;
use crate::transport::account::LibraryUpdate;

impl Library {
    pub fn enable_sync(&self, id: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
        let generation = self.operation()?;
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        {
            let mut inner = self.inner.lock().unwrap();
            index::begin_enable(
                &mut inner.db,
                &self.account,
                id,
                &entry.title,
                &entry.template,
                &paths::store(&self.root, Path::new(&entry.path))?,
                &entry.version,
            )?;
        }
        let outcome = self.with_owner(id, |owner| {
            crate::cloud::bootstrap(owner, token, &self.state_dir()).map_err(failed)?;
            self.current_operation(generation)?;
            crate::cloud::update_document(id, &entry.title, &entry.template, token).map_err(failed)?;
            // Sharing is a presentation cache. Bootstrap completion must not
            // depend on another request made only to populate that optional hint.
            self.current_operation(generation)
        });
        if let Err(error) = outcome {
            index::record_problem(&self.inner.lock().unwrap().db, &self.account, id, &error.message)?;
            return Err(error);
        }
        let live = {
            let mut inner = self.inner.lock().unwrap();
            index::finish_enable(&mut inner.db, &self.account, id)?;
            inner.token = Some(token.into());
            inner.live.get(id).and_then(Weak::upgrade)
        };
        if let Some(owner) = live {
            self.attach(id, owner, token)?;
        }
        let inner = self.inner.lock().unwrap();
        self.row(&inner, id)
    }
    pub fn download(&self, id: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
        let generation = self.operation()?;
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        if entry.deleted {
            return Err(failed("Cloud document is no longer available"));
        }
        let access = crate::cloud::authorized_access(id, token).map_err(failed)?;
        self.current_operation(generation)?;
        let Some(access) = access else {
            self.apply(LibraryUpdate::Remove(id.into()))?;
            return Err(failed("Cloud document access was removed; local edits were retained"));
        };
        self.remember_access(id, &access)?;
        if entry.downloaded {
            return self.row(&self.inner.lock().unwrap(), id);
        }
        let resolved = crate::resolution::resolve(id, token)
            .map_err(failed)?
            .ok_or_else(|| failed("Cloud document is not ready"))?;
        self.current_operation(generation)?;
        resolved
            .install_if_current(Path::new(&entry.path), &self.state_dir(), || {
                self.current_operation(generation).map_err(|error| error.message)
            })
            .map_err(failed)?;
        self.current_operation(generation)?;
        self.cache_file(id, None)?;
        let inner = self.inner.lock().unwrap();
        self.row(&inner, id)
    }
    fn remember_access(&self, id: &str, access: &AccessState) -> Result<()> {
        self.cloud()?;
        Self::valid(id)?;
        if access.document != id {
            return Err(failed("Access response belongs to another document"));
        }
        index::remember_sharing(&self.inner.lock().unwrap().db, &self.account, &access.document, access.sharing)
    }
    pub fn access(&self, id: &str, token: &str) -> Result<AccessState> {
        let _change = self.changing();
        let generation = self.operation()?;
        let access = crate::cloud::access(id, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access)?;
        Ok(access)
    }
    pub fn sharing(&self, id: &str, enabled: bool, token: &str) -> Result<AccessState> {
        let _change = self.changing();
        let generation = self.operation()?;
        let access = crate::cloud::set_sharing(id, enabled, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_access(id, &access)?;
        Ok(access)
    }
    pub fn join(&self, link: &str, token: &str) -> Result<Entry> {
        let _change = self.changing();
        let generation = self.operation()?;
        let access = crate::cloud::join_member(link, token).map_err(failed)?;
        let metadata = crate::cloud::document_metadata(&access.document, token).map_err(failed)?;
        self.current_operation(generation)?;
        self.remember_join(&access, &metadata)?;
        self.download(&access.document, token)
    }
    /// The document decides the role: an owner opening their own invitation stays the owner,
    /// so catalog deletion later deletes rather than leaves.
    fn remember_join(&self, access: &AccessState, metadata: &DocumentMetadata) -> Result<()> {
        let id = &access.document;
        let path = self.cached_path(id);
        let inner = self.inner.lock().unwrap();
        index::upsert_joined(&inner.db, &self.account, access, metadata, &paths::store(&self.root, &path)?)
    }
    pub fn leave(&self, id: &str, token: &str) -> Result<()> {
        self.remove_cloud(id, token, false)
    }
    pub fn delete_everywhere(&self, id: &str, token: &str) -> Result<()> {
        self.remove_cloud(id, token, true)
    }
    fn remove_cloud(&self, id: &str, token: &str, delete: bool) -> Result<()> {
        let _change = self.changing();
        let generation = self.operation()?;
        self.unregister(id)?;
        if !delete && self.row(&self.inner.lock().unwrap(), id)?.downloaded {
            self.local_copy(id, "Retained local copy")?;
        }
        self.departing.lock().unwrap().insert(id.into());
        let result = (|| {
            index::mark_departing(&self.inner.lock().unwrap().db, &self.account, id)?;
            let result = if delete { crate::cloud::delete_document(id, token) } else { crate::cloud::leave(id, token) };
            self.current_operation(generation)?;
            if let Err(error) = result {
                index::restore_departing(&self.inner.lock().unwrap().db, &self.account, id)?;
                return Err(failed(error));
            }
            Ok(())
        })();
        self.departing.lock().unwrap().remove(id);
        // Discard the connection's queued frames; its next complete snapshot is authoritative.
        if let Some(feed) = self.feed.lock().unwrap().as_ref() {
            feed.wake();
        }
        result
    }
    pub fn remove_download(&self, id: &str) -> Result<()> {
        let _change = self.changing();
        self.cloud()?;
        let entry = {
            let inner = self.inner.lock().unwrap();
            if inner.live.get(id).and_then(Weak::upgrade).is_some() {
                return Err(Error {
                    issue: Issue::Busy,
                    message: "Close this pop before removing its download".into(),
                });
            }
            self.row(&inner, id)?
        };
        if entry.local_only {
            return Err(failed("Local-only documents have no cloud download"));
        }
        if !entry.downloaded {
            return Ok(());
        }
        if !Path::new(&entry.path).starts_with(self.root.join("accounts").join(&self.scope)) {
            return Err(failed("This document is outside the cloud cache; its original file is retained"));
        }
        if !crate::client::saved_confirmed(Path::new(&entry.path), &self.state_dir(), id).map_err(failed)? {
            return Err(failed("This download has unconfirmed edits; synchronize it before removal"));
        }
        self.unregister(id)?;
        file::delete_document(Path::new(&entry.path))?;
        Ok(())
    }
}
