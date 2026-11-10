//! Live owners and their cloud connections.
use super::*;

impl Library {
    pub(super) fn with_owner<T>(&self, id: &str, work: impl FnOnce(&Arc<Owner>) -> Result<T>) -> Result<T> {
        let (entry, existing, temporary) = {
            let inner = self.inner.lock().unwrap();
            let retained = inner.retained.get(id).cloned();
            let live = inner.live.get(id).and_then(Weak::upgrade);
            (self.row(&inner, id)?, retained.clone().or(live.clone()), retained.is_some() || live.is_none())
        };
        let existing = existing.filter(|owner| !matches!(crate::local::request_typed(owner, Request::State), Err(error) if error.kind == FailureKind::Closed));
        let temporary = temporary || existing.is_none();
        let owner = match existing {
            Some(owner) => owner,
            None => Arc::new(Owner::open(Path::new(&entry.path), Mode::Document, Arc::new(|_| {}))?),
        };
        let result = work(&owner);
        if temporary {
            match crate::local::request_typed(&owner, Request::Close { preview: None, icon: None }) {
                Ok(_) => {
                    self.inner.lock().unwrap().retained.remove(id);
                }
                Err(error) => {
                    self.inner.lock().unwrap().retained.insert(id.into(), owner);
                    return Err(error.into());
                }
            }
        }
        result
    }
    pub fn register(&self, owner: &Arc<Owner>) -> Result<()> {
        let _change = self.changing();
        self.check()?;
        let Some(id) = owner.app().document_uuid.as_deref() else { return Ok(()) };
        let token = {
            let mut inner = self.inner.lock().unwrap();
            let Ok(entry) = self.row(&inner, id) else { return Ok(()) };
            if entry.deleted || Path::new(&entry.path) != owner.path() {
                return Ok(());
            }
            index::mark_opened(&inner.db, if entry.account_bound { self.account.as_str() } else { "" }, id)?;
            inner.live.insert(id.into(), Arc::downgrade(owner));
            if entry.local_only { None } else { inner.token.clone() }
        };
        if let Some(token) = token {
            if let Some(client) = self.connection(id).filter(|client| client.is_running()) {
                client.set_identity(&token).map_err(failed)?;
            } else {
                self.attach(id, owner.clone(), &token)?;
            }
        }
        Ok(())
    }
    pub fn unregister(&self, id: &str) -> Result<()> {
        let _change = self.changing();
        let (client, owner, deleted) = {
            let inner = self.inner.lock().unwrap();
            (
                inner.clients.get(id).cloned(),
                inner.live.get(id).and_then(Weak::upgrade),
                self.row(&inner, id).is_ok_and(|entry| entry.deleted),
            )
        };
        if let Some(client) = client {
            client.pause();
        }
        if let Some(owner) = owner {
            match crate::local::request_typed(&owner, Request::Flush) {
                Ok(_) => {}
                Err(error) if error.kind == FailureKind::Closed => {}
                Err(error) => return Err(error.into()),
            }
        }
        let downloaded = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id).is_ok_and(|entry| entry.downloaded)
        };
        if downloaded {
            self.cache_file(id, None)?;
        }
        if deleted && downloaded {
            self.account_recovery(id, "Recovered edits")?;
        }
        let mut inner = self.inner.lock().unwrap();
        inner.live.remove(id);
        inner.clients.remove(id);
        Ok(())
    }
    pub(super) fn attach(&self, id: &str, owner: Arc<Owner>, token: &str) -> Result<Arc<Client>> {
        let generation = self.operation()?;
        let previous = self.inner.lock().unwrap().clients.remove(id);
        if let Some(previous) = previous {
            previous.pause();
            previous.flush().map_err(failed)?;
        }
        let client = Arc::new(Client::cloud(owner, id, token, &self.state_dir()).map_err(failed)?);
        let notice = Arc::downgrade(&self.notice);
        client.observe(Arc::new(move || {
            if let Some(notice) = notice.upgrade() {
                changes::notify(&notice);
            }
        }));
        client.connect().map_err(failed)?;
        self.current_operation(generation)?;
        let mut inner = self.inner.lock().unwrap();
        let entry = self.row(&inner, id)?;
        if entry.deleted || entry.local_only {
            drop(inner);
            client.pause();
            return Err(failed("Cloud access was removed"));
        }
        inner.clients.insert(id.into(), client.clone());
        Ok(client)
    }
    pub fn sync(self: &Arc<Self>, token: &str) -> Result<()> {
        let _change = self.changing();
        let generation = self.operation()?;
        self.reconcile()?;
        let pending = {
            let inner = self.inner.lock().unwrap();
            index::pending_enable(&inner.db, &self.account)?
        };
        let mut failure = None;
        let owners = {
            let mut inner = self.inner.lock().unwrap();
            inner.token = Some(token.into());
            inner
                .live
                .iter()
                .filter_map(|(id, owner)| owner.upgrade().map(|owner| (id.clone(), owner)))
                .collect::<Vec<_>>()
        };
        for (id, owner) in owners {
            let entry = {
                let inner = self.inner.lock().unwrap();
                self.row(&inner, &id)?
            };
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
            if let Err(error) = self.enable_sync(&id, token) {
                failure = Some(error);
            }
        }
        self.connect_feed(token)?;
        let feed = self.feed.lock().unwrap().clone();
        if let Some(feed) = feed
            && !feed.wait_snapshot(std::time::Duration::from_secs(20))
        {
            failure.get_or_insert_with(|| {
                failed(
                    feed.last_error()
                        .unwrap_or_else(|| "The cloud library is unavailable. Local files are retained.".into()),
                )
            });
        }
        failure.map_or(Ok(()), Err)
    }
    pub fn connection(&self, id: &str) -> Option<Arc<Client>> {
        self.inner.lock().unwrap().clients.get(id).cloned()
    }
    pub fn set_account_active(&self, active: bool) {
        self.feed_active.store(active, Ordering::Release);
        if !active && let Some(feed) = self.feed.lock().unwrap().take() {
            feed.stop();
        }
    }
    pub fn detach(&self) -> Result<()> {
        let _change = self.changing();
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.active.store(false, Ordering::Release);
        if let Some(feed) = self.feed.lock().unwrap().take() {
            feed.stop();
        }
        let (clients, owners) = {
            let mut inner = self.inner.lock().unwrap();
            inner.token = None;
            (
                std::mem::take(&mut inner.clients),
                inner
                    .live
                    .values()
                    .filter_map(Weak::upgrade)
                    .chain(inner.retained.values().cloned())
                    .collect::<Vec<_>>(),
            )
        };
        for client in clients.into_values() {
            client.pause();
        }
        let mut failure = None;
        for owner in owners {
            if let Err(error) = crate::local::request_typed(&owner, Request::Flush)
                && error.kind != FailureKind::Closed
            {
                failure.get_or_insert(error);
            }
        }
        if let Some(error) = failure {
            self.active.store(true, Ordering::Release);
            return Err(error.into());
        }
        Ok(())
    }
}
