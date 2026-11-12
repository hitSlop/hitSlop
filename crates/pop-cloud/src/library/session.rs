//! Live owners and their cloud connections.
use super::*;

impl Library {
    pub(super) fn with_owner<T>(&self, id: &str, work: impl FnOnce(&Arc<Owner>) -> Result<T>) -> Result<T> {
        let generation = self.generation.load(Ordering::Acquire);
        let entry = self.row(id)?;
        let (existing, temporary) = {
            let inner = pop_core::lock(&self.inner);
            let retained = inner.retained.get(id).cloned();
            let live = inner.live.get(id).and_then(Weak::upgrade);
            (retained.clone().or(live.clone()), retained.is_some() || live.is_none())
        };
        let existing = existing.filter(|owner| owner.path() == Path::new(&entry.path)).filter(|owner| !matches!(crate::local::request(owner, Owner::version), Err(error) if error.owner_kind() == Some(FailureKind::Closed)));
        let temporary = temporary || existing.is_none();
        let owner = match existing {
            Some(owner) => owner,
            None => Arc::new(Owner::open(Path::new(&entry.path), Mode::Document, Arc::new(|_| {}))?),
        };
        {
            let mut inner = pop_core::lock(&self.inner);
            if let Err(error) = self.current_operation(generation) {
                drop(inner);
                if temporary {
                    let _ = crate::local::request(&owner, |owner: &Owner, options, callback| {
                        owner.close(None, None, options, callback)
                    });
                }
                return Err(error);
            }
            if temporary {
                inner.retained.insert(id.into(), owner.clone());
            }
        }
        let result = work(&owner);
        if temporary {
            match crate::local::request(&owner, |owner: &Owner, options, callback| {
                owner.close(None, None, options, callback)
            }) {
                Ok(_) => {
                    let mut inner = pop_core::lock(&self.inner);
                    if inner.retained.get(id).is_some_and(|retained| Arc::ptr_eq(retained, &owner)) {
                        inner.retained.remove(id);
                    }
                }
                Err(error) => {
                    pop_core::lock(&self.inner).retained.insert(id.into(), owner);
                    return Err(error);
                }
            }
        }
        result
    }
    pub fn register(&self, owner: &Arc<Owner>) -> Result<()> {
        let _change = self.changing();
        let Some(id) = owner.app().document_uuid.as_deref() else { return Ok(()) };
        let (entry, token) = {
            let mut inner = pop_core::lock(&self.inner);
            inner.live.insert(id.into(), Arc::downgrade(owner));
            if !self.active.load(Ordering::Acquire) {
                return Ok(());
            }
            let Ok(entry) = IndexedDocument::one(&inner.db, &self.account(), id) else { return Ok(()) };
            if entry.deleted || paths::resolve(&self.root, &entry.path)? != owner.path() {
                return Ok(());
            }
            index::mark_opened(&inner.db, &entry.account, id)?;
            (entry, inner.token.clone())
        };
        if entry.role.is_some()
            && let Some(token) = token
        {
            let generation = self.operation()?;
            let weak = self.weak.clone();
            let id = id.to_owned();
            let owner = owner.clone();
            crate::transport::spawn_library(move || {
                if let Some(library) = weak.upgrade()
                    && library.current_operation(generation).is_ok()
                {
                    let _ = library.attach(&id, owner, &token);
                    library.notify();
                }
            })?;
        }
        Ok(())
    }
    pub fn unregister(&self, id: &str) -> Result<()> {
        let _change = self.changing();
        let deleted = self.row(id).is_ok_and(|entry| entry.deleted);
        let (client, owner) = {
            let inner = pop_core::lock(&self.inner);
            (inner.clients.get(id).cloned(), inner.live.get(id).and_then(Weak::upgrade))
        };
        if let Some(client) = client {
            client.pause();
        }
        if let Some(owner) = owner {
            match crate::local::request(&owner, Owner::flush) {
                Ok(_) => {}
                Err(error) if error.owner_kind() == Some(FailureKind::Closed) => {}
                Err(error) => return Err(error),
            }
        }
        let downloaded = self.row(id).is_ok_and(|entry| entry.downloaded);
        if downloaded {
            self.cache_file(id, None)?;
        }
        if deleted && downloaded {
            self.account_recovery(id, "Recovered edits")?;
        }
        let mut inner = pop_core::lock(&self.inner);
        inner.live.remove(id);
        inner.clients.remove(id);
        Ok(())
    }
    pub(super) fn attach(&self, id: &str, owner: Arc<Owner>, token: &str) -> Result<Arc<Client>> {
        let generation = self.operation()?;
        let owner_path = owner.path().to_owned();
        let previous = {
            let mut inner = pop_core::lock(&self.inner);
            self.current_operation(generation)?;
            let entry = IndexedDocument::one(&inner.db, &self.account(), id)?;
            if entry.deleted || entry.role.is_none() || paths::resolve(&self.root, &entry.path)? != owner_path {
                return Err(failed("Cloud access was removed"));
            }
            inner.clients.remove(id)
        };
        if let Some(previous) = previous {
            previous.pause();
            previous.flush().map_err(failed)?;
        }
        let client = Arc::new(
            Client::cloud(owner, id, token, self.receipt(id, generation)?, self.notice.clone()).map_err(failed)?,
        );
        client.connect().map_err(failed)?;
        self.current_operation(generation)?;
        let mut inner = pop_core::lock(&self.inner);
        self.current_operation(generation)?;
        let entry = IndexedDocument::one(&inner.db, &self.account(), id)?;
        if entry.deleted || entry.role.is_none() || paths::resolve(&self.root, &entry.path)? != owner_path {
            drop(inner);
            client.pause();
            return Err(failed("Cloud access was removed"));
        }
        inner.clients.insert(id.into(), client.clone());
        Ok(client)
    }
    pub fn sync(self: &Arc<Self>, account: &str, token: &str) -> Result<()> {
        let generation = self.operation_for(account)?;
        {
            let mut inner = pop_core::lock(&self.inner);
            self.current_operation(generation)?;
            inner.token = Some(token.to_owned());
            if inner.syncing {
                return Ok(());
            }
            inner.syncing = true;
        }
        let library = self.clone();
        let token = token.to_owned();
        crate::transport::spawn_library(move || {
            if library.current_operation(generation).is_err() {
                return;
            }
            if let Err(error) = library.sync_work(&token, generation) {
                let inner = pop_core::lock(&library.inner);
                if library.current_operation(generation).is_ok() {
                    let _ = inner.db.execute(
                        "UPDATE documents SET problem=?2 WHERE account=?1 AND role IS NOT NULL",
                        params![library.account(), error.to_string()],
                    );
                }
            }
            let latest = {
                let mut inner = pop_core::lock(&library.inner);
                if library.current_operation(generation).is_ok() {
                    inner.syncing = false;
                    inner.token.clone().filter(|latest| latest != &token)
                } else {
                    None
                }
            };
            library.notify();
            if let Some(token) = latest {
                let _ = library.sync(&library.account(), &token);
            }
        })
    }
    fn sync_work(self: &Arc<Self>, token: &str, generation: u64) -> Result<()> {
        let _change = self.changing();
        self.current_operation(generation)?;
        let account = self.account();
        self.reconcile()?;
        let pending = {
            let inner = pop_core::lock(&self.inner);
            index::pending_enable(&inner.db, &self.account())?
        };
        let mut failure = None;
        let owners = {
            let inner = pop_core::lock(&self.inner);
            self.current_operation(generation)?;
            inner
                .live
                .iter()
                .filter_map(|(id, owner)| owner.upgrade().map(|owner| (id.clone(), owner)))
                .collect::<Vec<_>>()
        };
        for (id, owner) in owners {
            let Ok(entry) = self.row(&id) else { continue };
            if Path::new(&entry.path) != owner.path() {
                continue;
            }
            if entry.local_only || entry.deleted {
                continue;
            }
            self.current_operation(generation)?;
            let existing = self.connection(&id).filter(|client| client.is_running());
            let refreshed = existing.map_or_else(
                || self.attach(&id, owner, token).map(|_| ()),
                |client| client.set_identity(token).map_err(failed),
            );
            self.current_operation(generation)?;
            if let Err(error) = refreshed {
                failure = Some(error);
            }
        }
        for id in pending {
            self.current_operation(generation)?;
            if let Err(error) = self.enable_sync(&account, &id, token) {
                failure = Some(error);
            }
        }
        self.current_operation(generation)?;
        self.connect_feed(token)?;
        failure.map_or(Ok(()), Err)
    }
    pub fn connection(&self, id: &str) -> Option<Arc<Client>> {
        pop_core::lock(&self.inner).clients.get(id).cloned()
    }
    pub fn set_account_active(&self, active: bool) {
        self.feed_active.store(active, Ordering::Release);
        if !active && let Some(feed) = pop_core::lock(&self.feed).take() {
            feed.stop();
        }
    }
    /// Lost credentials stop transport before the host tries to drain and close its pages.
    pub fn suspend_account(&self) -> Result<()> {
        let _change = self.changing();
        let clients = {
            let mut inner = pop_core::lock(&self.inner);
            if inner.switching {
                return Err(failed("Account change in progress"));
            }
            if !self.active.swap(false, Ordering::AcqRel) {
                return Ok(());
            }
            self.generation.fetch_add(1, Ordering::AcqRel);
            inner.token = None;
            inner.syncing = false;
            std::mem::take(&mut inner.clients)
        };
        let feed = pop_core::lock(&self.feed).take();
        if let Some(feed) = feed {
            feed.stop();
        }
        for client in clients.into_values() {
            client.pause();
        }
        Ok(())
    }
    pub fn set_account(&self, account: Option<String>) -> Result<()> {
        let account = account.unwrap_or_default();
        let (clients, owners, previous) = {
            let mut inner = pop_core::lock(&self.inner);
            if inner.switching {
                return Err(failed("Account change in progress"));
            }
            if self.account() == account {
                self.active.store(true, Ordering::Release);
                drop(inner);
                self.notify();
                return Ok(());
            }
            let previous = self.account();
            let bound = IndexedDocument::all(&inner.db, &previous)?
                .into_iter()
                .filter(|row| !row.account.is_empty())
                .map(|row| row.id)
                .collect::<std::collections::HashSet<_>>();
            self.active.store(false, Ordering::Release);
            inner.switching = true;
            self.generation.fetch_add(1, Ordering::AcqRel);
            pop_core::lock(&self.departing).clear();
            let owners = inner
                .live
                .iter()
                .filter(|(id, _)| bound.contains(*id))
                .filter_map(|(_, owner)| owner.upgrade())
                .chain(inner.retained.values().cloned())
                .collect::<Vec<_>>();
            inner.token = None;
            inner.syncing = false;
            (std::mem::take(&mut inner.clients), owners, previous)
        };
        let feed = pop_core::lock(&self.feed).take();
        if let Some(feed) = feed {
            feed.stop();
        }
        for client in clients.into_values() {
            client.pause();
        }
        let result = (|| {
            for owner in owners {
                match crate::local::request(&owner, |owner: &Owner, options, callback| owner.flush(options, callback)) {
                    Ok(_) => {}
                    Err(error) if error.owner_kind() == Some(FailureKind::Closed) => {}
                    Err(error) => return Err(error),
                }
            }
            let retained = pop_core::lock(&self.inner).retained.values().cloned().collect::<Vec<_>>();
            for owner in retained {
                match crate::local::request(&owner, |owner: &Owner, options, callback| {
                    owner.close(None, None, options, callback)
                }) {
                    Ok(_) => {}
                    Err(error) if error.owner_kind() == Some(FailureKind::Closed) => {}
                    Err(error) => return Err(error),
                }
            }
            let scope = hex(&Sha256::digest(account.as_bytes()));
            std::fs::create_dir_all(self.root.join("accounts").join(scope).join("documents"))?;
            Ok(())
        })();
        {
            let mut inner = pop_core::lock(&self.inner);
            *pop_core::lock(&self.account) = if result.is_ok() { account } else { previous };
            if result.is_ok() {
                inner.retained.clear();
            }
            inner.switching = false;
            self.active.store(true, Ordering::Release);
        }
        self.notify();
        result
    }
}
