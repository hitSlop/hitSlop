//! Documents this device creates, adopts and copies.
use super::{projection::file_stamp, *};

impl Library {
    pub fn create(&self, template: &Path, title: &str) -> Result<Entry> {
        let _change = self.changing();
        self.check()?;
        let temporary = self.root.join("documents").join(format!("new-{}.pop", random_hex(16)));
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
        let inner = self.inner.lock().unwrap();
        let existing = index::indexed(&inner.db, &self.root, &info.id)?;
        if !existing.is_empty() {
            if existing.iter().any(|(_, path)| path != &source) {
                return Err(Error {
                    issue: Issue::AppMismatch,
                    message: "A different local file has this document identity".into(),
                });
            }
            index::mark_opened_anywhere(&inner.db, &self.account, &info.id)?;
            return self.row(&inner, &info.id);
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
        let inner = self.inner.lock().unwrap();
        self.row(&inner, &info.id)
    }
    pub fn delete_local(&self, id: &str) -> Result<()> {
        let _change = self.changing();
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        if !entry.local_only {
            return Err(failed("Use cloud deletion for synchronized documents"));
        }
        self.unregister(id)?;
        file::delete_document(Path::new(&entry.path))?;
        index::delete_local(&self.inner.lock().unwrap().db, if entry.account_bound { &self.account } else { "" }, id)?;
        Ok(())
    }
    pub(super) fn cache_file(&self, id: &str, artwork_source: Option<&str>) -> Result<()> {
        let entry = {
            let inner = self.inner.lock().unwrap();
            self.row(&inner, id)?
        };
        let info = crate::local::inspect(Path::new(&entry.path)).map_err(failed)?;
        let stamp = file_stamp(Path::new(&entry.path))?;
        let inner = self.inner.lock().unwrap();
        index::update_cached(
            &inner.db,
            if entry.account_bound { &self.account } else { "" },
            id,
            &info.version,
            &info.artwork_digest,
            &stamp,
            artwork_source,
        )
    }
    pub fn local_copy(&self, id: &str, title: &str) -> Result<Entry> {
        let _change = self.changing();
        let path = self.root.join("documents").join(format!("copy-{}.pop", random_hex(16)));
        self.with_owner(id, |owner| {
            crate::local::request(owner, Request::Copy { destination: path.clone(), preview: None, icon: None })
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
        {
            let inner = self.inner.lock().unwrap();
            if self.row(&inner, id).is_err() {
                return Ok(false);
            }
        }
        crate::local::request(owner, Request::SetArtwork { source_version: version.into(), preview, icon })
            .map_err(failed)?;
        self.cache_file(id, Some(version))?;
        Ok(true)
    }
}
pub(super) fn random_hex(bytes: usize) -> String {
    let mut value = vec![0; bytes];
    getrandom::getrandom(&mut value).expect("OS random source");
    hex(&value)
}
