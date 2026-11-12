//! Recovery copies of edits a cloud document could not keep.
use super::*;

impl Library {
    /// Index completed recovery copies at explicit lifecycle/refresh boundaries.
    /// Snapshot reads do not mutate the index or create files.
    pub fn reconcile(&self) -> Result<()> {
        self.check()?;
        let before = pop_core::lock(&self.inner).db.total_changes();
        let result = self.discover_recoveries();
        let changed = pop_core::lock(&self.inner).db.total_changes() != before;
        if changed {
            self.notify();
        }
        result
    }

    fn discover_recoveries(&self) -> Result<()> {
        if self.account().is_empty() {
            return Ok(());
        }
        let directory = self.state_dir().join("Recovered");
        let paths = match std::fs::read_dir(directory) {
            Ok(paths) => paths,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        for path in paths {
            let path = path?.path();
            if !path.is_file() || path.extension().is_none_or(|extension| extension != "pop") {
                continue;
            }
            let known = index::path_known(&pop_core::lock(&self.inner).db, &paths::store(&self.root, &path)?)?;
            if !known {
                self.index_recovery(&path, "Recovered edits")?;
            }
        }
        Ok(())
    }
    fn index_recovery(&self, path: &Path, title: &str) -> Result<Entry> {
        let generation = self.generation.load(Ordering::Acquire);
        let info = crate::local::inspect(path).map_err(failed)?;
        let store = Store::open(path, Mode::Snapshot)?;
        if store.share()?.is_some() {
            return Err(failed("Recovery still belongs to a cloud document"));
        }
        let template = store.app().app.metadata().slug.clone();
        store.close()?;
        let inner = pop_core::lock(&self.inner);
        self.current_operation(generation)?;
        index::insert_recovery(
            &inner.db,
            &self.account(),
            &info.id,
            title,
            &template,
            &paths::store(&self.root, path)?,
            &info.version,
        )?;
        drop(inner);
        self.row(&info.id)
    }
    pub(super) fn account_recovery(&self, id: &str, title: &str) -> Result<Entry> {
        let generation = self.operation()?;
        let directory = self.state_dir().join("Recovered");
        let info = self.with_owner(id, |owner| {
            let state = crate::local::request(owner, Owner::version).map_err(failed)?;
            let reading = state;
            Ok(reading.version)
        })?;
        self.current_operation(generation)?;
        std::fs::create_dir_all(&directory)?;
        let digest = hex(&Sha256::digest(format!("{id}:{info}").as_bytes()));
        let path = directory.join(format!("{digest}.pop"));
        if !path.exists() {
            self.with_owner(id, |owner| {
                crate::local::request(owner, |owner: &Owner, options, callback| {
                    owner.copy(path.clone(), None, None, options, callback)
                })
                .map_err(failed)?;
                Ok(())
            })?;
        }
        self.current_operation(generation)?;
        self.index_recovery(&path, title)
    }
}
