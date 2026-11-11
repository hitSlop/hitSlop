//! Documents this device creates, adopts and copies.
use super::{projection::file_stamp, *};

impl Library {
    pub fn create(&self, template: &Path, title: &str) -> Result<Entry> {
        let _change = self.changing();
        self.check()?;
        let temporary = self
            .root
            .join("documents")
            .join(format!("new-{}.pop", pop_core::random_hex(16).expect("OS random source")));
        file::create_document(template, &temporary)?;
        let opened = file::open(&temporary, true)?;
        let id = opened.document_uuid.ok_or_else(|| failed("Missing document identity"))?;
        let path = self.root.join("documents").join(format!("{id}.pop"));
        std::fs::rename(&temporary, &path)?;
        self.adopt(&path, title)
    }
    pub fn adopt(&self, source: &Path, title: &str) -> Result<Entry> {
        let _change = self.changing();
        self.check()?;
        let source = file::resolve(source)?;
        let info = crate::local::inspect(&source).map_err(failed)?;
        let store = Store::open(&source, Mode::Snapshot)?;
        let template = store.app().app.metadata().slug.clone();
        store.close()?;
        let inner = pop_core::lock(&self.inner);
        let existing = index::indexed(&inner.db, &self.root, &info.id)?;
        if !existing.is_empty() {
            if existing.iter().any(|(_, path)| path != &source) {
                return Err(Error::Library {
                    issue: Issue::AppMismatch,
                    message: "A different local file has this document identity".into(),
                });
            }
            index::mark_opened_anywhere(&inner.db, &self.account(), &info.id)?;
            drop(inner);
            return self.row(&info.id);
        }
        index::insert_local(
            &inner.db,
            &info.id,
            title.trim(),
            &template,
            &paths::store(&self.root, &source)?,
            &info.version,
        )?;
        drop(inner);
        self.cache_file(&info.id, Some(&info.version))?;
        self.row(&info.id)
    }
    pub fn delete_local(&self, id: &str) -> Result<()> {
        let _change = self.changing();
        let entry = self.row(id)?;
        if !entry.local_only {
            return Err(failed("Use cloud deletion for synchronized documents"));
        }
        self.unregister(id)?;
        file::delete_document(Path::new(&entry.path))?;
        index::delete_local(
            &pop_core::lock(&self.inner).db,
            &if entry.account_bound { self.account() } else { String::new() },
            id,
        )?;
        Ok(())
    }
    pub(super) fn cache_file(&self, id: &str, artwork_source: Option<&str>) -> Result<()> {
        let generation = self.generation.load(Ordering::Acquire);
        let entry = self.row(id)?;
        let info = crate::local::inspect(Path::new(&entry.path)).map_err(failed)?;
        let stamp = file_stamp(Path::new(&entry.path))?;
        let inner = pop_core::lock(&self.inner);
        self.current_operation(generation)?;
        let current = IndexedDocument::one(&inner.db, &self.account(), id)?;
        if paths::resolve(&self.root, &current.path)? != Path::new(&entry.path) {
            return Err(failed("The document location changed"));
        }
        index::update_cached(
            &inner.db,
            &if entry.account_bound { self.account() } else { String::new() },
            id,
            &info.version,
            &info.artwork_digest,
            &stamp,
            artwork_source,
        )
    }
    pub fn local_copy(&self, id: &str, title: &str) -> Result<Entry> {
        let _change = self.changing();
        let path = self
            .root
            .join("documents")
            .join(format!("copy-{}.pop", pop_core::random_hex(16).expect("OS random source")));
        self.with_owner(id, |owner| {
            crate::local::request(owner, |owner: &Owner, options, callback| {
                owner.copy(path.clone(), None, None, options, callback)
            })
            .map_err(failed)?;
            Ok(())
        })?;
        self.adopt(&path, title)
    }
    pub fn publish_artwork(
        &self,
        owner: &Arc<Owner>,
        version: &str,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    ) -> Result<bool> {
        let _change = self.changing();
        let Some(id) = owner.app().document_uuid.as_deref() else {
            return Ok(false);
        };
        if self.row(id).is_err() {
            return Ok(false);
        }
        crate::local::request(owner, |owner: &Owner, options, callback| {
            owner.set_artwork(version.into(), preview, icon, options, callback)
        })
        .map_err(failed)?;
        self.cache_file(id, Some(version))?;
        Ok(true)
    }
}
