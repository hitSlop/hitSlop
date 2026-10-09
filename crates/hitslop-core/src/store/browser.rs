//! Browser VFS copies, resources and streamed SQLite exports.
use super::*;

impl Store {
    /// Streams SQLite pages from a single read transaction. The browser writes each slice
    /// directly to a sync OPFS export handle, never materializing the whole file in memory.
    pub fn export_pages(&self, write: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        self.check(true)?;
        let backing = lock(&self.backing);
        let conn = backing.conn.as_ref().ok_or(Error::Closed)?;
        let tx = conn.unchecked_transaction().map_err(sqlite("export"))?;
        rows::export_pages(&tx, write)
    }

    /// The browser driver holds this copy's Web Lock before installing its VFS.
    /// No filesystem paths, secondary connections or native registry are involved.
    pub fn open_vfs(name: &str, vfs: &str, imported: bool) -> Result<Self> {
        let (conn, mut app) = open_browser(name, vfs, "open")?;
        if app.kind != Kind::Document {
            return Err(rejected(crate::Code::IsTemplate, "A template opens by creating a document from it"));
        }
        // Validate the Loro checkpoint too, before changing a connection pragma.
        load(&conn, app.app.spec())?;
        file::configure_writer(&conn)?;
        if imported {
            let tx = conn.unchecked_transaction().map_err(sqlite("import identity"))?;
            rows::renew_document(&tx)?;
            tx.commit().map_err(sqlite("import identity"))?;
            app.document_uuid = rows::document_uuid(&conn)?;
        }
        Ok(Self {
            resource_cache: Default::default(),
            browser_file: (name.into(), vfs.into()),
            app,
            owned: AtomicBool::new(true),
            backing: Mutex::new(Backing { conn: Some(conn) }),
            account: Mutex::new(Account::new(Budget::DEFAULT)),
        })
    }

    pub fn browser_needs_recovery(&self) -> bool {
        self.owned.load(Ordering::Acquire) && lock(&self.backing).conn.is_none()
    }

    pub fn resource_info(&self, route: file::ResourceRoute, key: &str) -> Result<Option<file::ResourceInfo>> {
        self.read(|conn| file::ResourceReader::with_cache(conn, self.resource_cache.clone()).info(route, key))
    }

    pub fn resource_range(
        &self,
        route: file::ResourceRoute,
        key: &str,
        offset: u64,
        length: u64,
    ) -> Result<Option<Vec<u8>>> {
        self.read(|conn| {
            file::ResourceReader::with_cache(conn, self.resource_cache.clone()).read_range(route, key, offset, length)
        })
    }
}

/// Opens and checks the stored app without changing the file or its identity.
pub(super) fn open_browser(name: &str, vfs: &str, action: &'static str) -> Result<(Connection, OpenedApp)> {
    let conn = Connection::open_with_flags_and_vfs(
        name,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        vfs,
    )
    .map_err(sqlite(action))?;
    file::configure_connection(&conn)?;
    let app = file::opened(&conn, Path::new(name), true)?;
    Ok((conn, app))
}
