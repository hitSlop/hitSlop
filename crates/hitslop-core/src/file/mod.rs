//! A document is one SQLite file: the app its author built, the document's saved state, its
//! attachments and its artwork. This module owns that format: the tables and every statement
//! on them (every write, and the store's reads, in `rows`), the checks every open runs, packing a build into a template (`pack`),
//! creating and copying documents (`copy`), where documents may live (`places`), which
//! templates hosts list (`catalog`), and serving the app's assets (`assets`). `store` saves a document; `registry` holds its writer lock.

mod artwork;
mod assets;
pub mod build;
mod catalog;
mod copy;
mod pack;
mod places;
pub(crate) mod rows;

pub use artwork::Artwork;
pub(crate) use artwork::{check_artwork, optimize_png};
pub use assets::{ResourceInfo, ResourceReader, ResourceRoute, content_type, valid_asset_path};
pub use catalog::{Catalog, Folder, Template, find_template, list_templates, open_template, template_source};
pub(crate) use copy::copy;
pub use copy::create_document;
pub use pack::{APP_INPUT_BYTES, pack, validate_app};
pub use places::{TemplateSource, template_roots};
pub(crate) use places::{document_destination, document_location};

use crate::app::{AppDefinition, AppMetadata, Author, WindowDefinition};
use crate::error::{Error, Result, failed, invalid, requires_update, sqlite};
use crate::wire::{ASSET_PATH_BYTES, MANIFEST_BYTES};
use assets::{assets_within, read_asset};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, config::DbConfig, limits::Limit};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub(crate) const APPLICATION_ID: i64 = 0x4853_4C50; // HSLP
/// These tables. A compatibility requirement, not a release number: a build refuses a
/// newer one with `requires_update`.
pub(crate) const STORAGE_VERSION: i64 = 1;
/// `app` is what the author built, written once by `pack` and identical in a template and
/// its documents, so saved state always belongs to its descriptor. A template's
/// `checkpoint` holds its initial state; a document starts as a copy and adds its
/// `document` row, then `updates` and `attachments`. An asset's `size` is its length;
/// `encoding` is how `bytes` holds it (`encode`). The tables are STRICT: SQLite refuses a
/// mistyped write and the quick check finds a mistyped row, but a file is untrusted bytes,
/// so every open still checks what it reads.
pub(crate) const SCHEMA: &str = include_str!("storage-1.sql");
const _: () = assert!(STORAGE_VERSION == 1, "add the old reader and a transactional migration before raising storage");

/// The file's path with its folder resolved: NOFOLLOW refuses a symbolic link anywhere in
/// a path, while the file itself must not be one.
pub(crate) fn resolve(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| failed("Invalid document path"))?;
    let folder = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Ok(fs::canonicalize(folder).map_err(|e| failed(format!("Cannot resolve the document's folder: {e}")))?.join(name))
}
/// Every connection: no symbolic links, defensive mode, no trusted schema, cell checks, no
/// memory mapping, and values no longer than the largest stored one.
pub(crate) fn connect(path: &Path, flags: OpenFlags, busy: Duration) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        resolve(path)?,
        flags | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(sqlite("open"))?;
    conn.busy_timeout(busy).map_err(sqlite("open"))?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true).map_err(sqlite("open"))?;
    // Closing never checkpoints: a file in WAL mode, which only a newer build writes, is
    // refused, never rewritten.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true).map_err(sqlite("open"))?;
    conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA cell_size_check=ON; PRAGMA mmap_size=0;")
        .map_err(sqlite("open"))?;
    let version = one(&conn, "PRAGMA user_version")?;
    if version <= STORAGE_VERSION {
        conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, crate::STORAGE_BYTES as i32).map_err(sqlite("open"))?;
    }
    Ok(conn)
}
/// The writer's durability: a rollback journal that exists only while a save commits, and
/// a full sync of every commit. Deleted content is zeroed where that costs no extra write
/// (FAST): Apple's SQLite does so by default, the bundled SQLite of the Linux engines and
/// the browser does not, and a shared file should not depend on which one wrote it.
pub(crate) fn configure_writer(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON; PRAGMA secure_delete=FAST;",
    )
    .map_err(sqlite("configure"))
}
/// A writer's connection: read-write, creating a new file when `create`. An existing file
/// is checked before `configure_writer`, so a file this build refuses is never written.
pub(crate) fn writer(path: &Path, create: bool) -> Result<Connection> {
    let create = if create { OpenFlags::SQLITE_OPEN_CREATE } else { OpenFlags::empty() };
    connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE | create, Duration::from_secs(2))
}
/// A reader's connection: read-write with `query_only`. A read-only connection beside this
/// process's writer makes the platform SQLite fail the writer's locks, and sometimes its
/// own, with EBADF (SQLITE_IOERR_LOCK). Like any opener, it completes the rollback of a
/// crashed write, which restores the saved state it reads. A file this process can't write
/// opens read-only; no writer can be in this process then.
pub(crate) fn reader(path: &Path) -> Result<Connection> {
    let busy = Duration::from_secs(5);
    match connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE, busy) {
        Ok(conn) => {
            conn.execute_batch("PRAGMA query_only=ON").map_err(sqlite("open"))?;
            Ok(conn)
        }
        Err(_) => connect(path, OpenFlags::SQLITE_OPEN_READ_ONLY, busy),
    }
}
/// All durable writes pass through this transaction. Future forward migrations run
/// here, never on reads, and validate before the same transaction commits.
pub(crate) struct WriteTransaction<'a> {
    tx: Transaction<'a>,
    validate: bool,
}
impl std::ops::Deref for WriteTransaction<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.tx
    }
}
impl WriteTransaction<'_> {
    pub(crate) fn commit(self) -> Result<()> {
        if self.validate {
            check(&self.tx, true)?;
        }
        self.tx.commit().map_err(sqlite("commit"))
    }
}
pub(crate) fn begin_write<'a>(conn: &'a Connection, action: &str) -> Result<WriteTransaction<'a>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite(action))?;
    let version = markers(&tx)?;
    let validate = migrate(&tx, version)?;
    Ok(WriteTransaction { tx, validate })
}
/// Brings a file from `from` to `STORAGE_VERSION` inside the write's transaction; the commit
/// then checks the result in full. Version 1 is the first. A later version adds its arm
/// here, admits `from` in `markers`, and creates new files by replaying the same chain
/// (`storage-1.sql`, then each migration), so a migrated file and a new one share one
/// exact layout. Display reads never call this.
fn migrate(_tx: &Connection, from: i64) -> Result<bool> {
    match from {
        STORAGE_VERSION => Ok(false),
        _ => Err(invalid("Unsupported document storage")),
    }
}
pub(crate) fn initialize(conn: &Connection) -> Result<WriteTransaction<'_>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("create"))?;
    Ok(WriteTransaction { tx, validate: true })
}

/// A template holds only the app; a document also holds its saved state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Template,
    Document,
}

type SchemaRow = (String, String, String, Option<String>);
fn tables(conn: &Connection) -> Result<Vec<SchemaRow>> {
    let mut statement = conn
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name")
        .map_err(sqlite("read tables"))?;
    let rows =
        statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(sqlite("read tables"))?;
    rows.collect::<rusqlite::Result<_>>().map_err(sqlite("read tables"))
}
/// The tables and indexes `SCHEMA` makes, including SQLite's automatic primary-key
/// indexes; built once.
fn expected_tables(version: i64) -> &'static [SchemaRow] {
    assert_eq!(version, 1, "reader selected after marker validation");
    static LAYOUT: OnceLock<Vec<SchemaRow>> = OnceLock::new();
    LAYOUT.get_or_init(|| {
        let memory = Connection::open_in_memory().expect("in-memory database");
        memory.execute_batch(SCHEMA).expect("valid schema");
        tables(&memory).expect("readable tables")
    })
}

/// The checks every open runs, in order, reading sizes (`length()`, an asset's recorded
/// size) and never a value. The application ID, the storage version and the app's
/// requirements come first, so a newer file is refused with `requires_update` even where a
/// newer build changed the tables they describe; then the exact tables, the rows a template
/// or a document may hold, and every size budget. `integrity` adds `PRAGMA quick_check`
/// (42 ms for the largest allowed document). The checks run in one read transaction, the
/// caller's if it holds one (`opened`), so they see one state while another process saves.
pub(crate) fn check(conn: &Connection, integrity: bool) -> Result<Kind> {
    let _read = if conn.is_autocommit() { Some(conn.unchecked_transaction().map_err(sqlite("read"))?) } else { None };
    let version = markers(conn)?;
    layout(conn, version)?;
    contents(conn, integrity)
}
/// The checks after the markers and exact layout: sizes, assets, saved-state rows and,
/// with `integrity`, SQLite's quick check.
fn contents(conn: &Connection, integrity: bool) -> Result<Kind> {
    app_sizes(conn)?;
    stored_assets(conn)?;
    let kind = state(conn)?;
    if integrity {
        let result: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(sqlite("check"))?;
        if result != "ok" {
            return Err(failed("The document file is damaged; keep it for recovery"));
        }
    }
    Ok(kind)
}
fn one(conn: &Connection, sql: &str) -> Result<i64> {
    conn.query_row(sql, [], |r| r.get(0)).map_err(sqlite("read"))
}
/// The application ID, the storage version and the app's requirements: what tells a file
/// this build reads from one it is too old for.
fn markers(conn: &Connection) -> Result<i64> {
    if one(conn, "PRAGMA application_id")? != APPLICATION_ID {
        return Err(invalid("This is not a hitSlop document"));
    }
    let version = one(conn, "PRAGMA user_version")?;
    if version > STORAGE_VERSION {
        return Err(requires_update(format!(
            "This document uses storage version {version}; this hitSlop reads version {STORAGE_VERSION}"
        )));
    }
    if version != STORAGE_VERSION {
        return Err(invalid("Unsupported document storage"));
    }
    // Two named columns, which any newer app format keeps; the tables are compared next.
    let found =
        conn.query_row("SELECT package_format, runtime_abi FROM app WHERE id=1", [], |r| Ok((r.get(0)?, r.get(1)?)));
    let (package_format, runtime_abi) = match found {
        Ok(markers) => markers,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(invalid("Unexpected rows in app")),
        // No such table or column, or a marker that is not an integer.
        Err(rusqlite::Error::InvalidColumnType(..)) => return Err(invalid("Unexpected document tables")),
        Err(e) if e.sqlite_error_code() == Some(rusqlite::ffi::ErrorCode::Unknown) => {
            return Err(invalid("Unexpected document tables"));
        }
        Err(e) => return Err(sqlite("read")(e)),
    };
    requirements(package_format, runtime_abi)?;
    Ok(version)
}
/// The app's requirements, as `pack` reads them from a build and every open from a file:
/// none newer than this build supports, and each at least 1.
fn requirements(package_format: i64, runtime_abi: i64) -> Result<()> {
    crate::app::requirements(package_format.into(), runtime_abi.into()).map_err(Error::Rejected)
}
/// The exact tables, and at most one row, row 1, in each one-row table.
fn layout(conn: &Connection, version: i64) -> Result<()> {
    if tables(conn)? != expected_tables(version) {
        return Err(invalid("Unexpected document tables"));
    }
    // A file written without its CHECK constraints may hold other rows, which no read
    // would see.
    for table in ["app", "document", "checkpoint"] {
        let (rows, first): (i64, i64) = conn
            .query_row(&format!("SELECT count(*), coalesce(sum(id=1),0) FROM {table}"), [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(sqlite("read"))?;
        if rows > 1 || rows != first || (table == "app" && rows != 1) {
            return Err(invalid(format!("Unexpected rows in {table}")));
        }
    }
    Ok(())
}
/// Bound recursive definition bytes before decoding. Display reads never call this.
fn app_sizes(conn: &Connection) -> Result<()> {
    if one(conn, "SELECT length(CAST(definition_json AS BLOB)) FROM app")?
        > crate::app::package_format_1::DEFINITION_BYTES as i64
    {
        return Err(invalid("The app definition is too large"));
    }
    Ok(())
}
/// The assets' encodings, budgets and paths.
fn stored_assets(conn: &Connection) -> Result<()> {
    // Stored as it is, an asset is its size; compressed, it stores less, and decoding stops
    // at its size. The budgets then bound what decoding produces.
    if one(
        conn,
        "SELECT count(*) FROM assets WHERE typeof(size)!='integer' OR CASE encoding WHEN 'identity' THEN length(bytes)!=size WHEN 'br' THEN length(bytes)>=size ELSE 1 END",
    )? > 0
    {
        return Err(invalid("Invalid asset encoding"));
    }
    let (assets, largest, total, longest_path): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT count(*), coalesce(max(size),0), coalesce(sum(size),0), coalesce(max(length(CAST(key AS BLOB))),0) FROM assets",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(sqlite("read"))?;
    assets_within(assets as usize, largest as usize, total as usize)?;
    if longest_path > ASSET_PATH_BYTES as i64 {
        return Err(invalid("An asset path is too long"));
    }
    let mut paths = conn.prepare("SELECT key,media_type FROM assets").map_err(sqlite("read"))?;
    for row in paths.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(sqlite("read"))? {
        let (key, media_type) = row.map_err(sqlite("read"))?;
        let kind = crate::media::asset_key(&key).ok_or_else(|| invalid("Invalid app asset key"))?;
        if kind.media_type != media_type {
            return Err(invalid("Invalid app asset media type"));
        }
    }
    Ok(())
}
/// Whether the file is a template or a document. Both hold exactly one checkpoint: a
/// template's is its initial state, which only a document adds updates and attachments
/// to. A document's attachments are within their limits and named by their hashes; the
/// saved state's own budget is the store's (`checked_bounds`).
fn state(conn: &Connection) -> Result<Kind> {
    let documents = one(conn, "SELECT count(*) FROM document")?;
    let checkpoints = one(conn, "SELECT count(*) FROM checkpoint")?;
    let updates = one(conn, "SELECT count(*) FROM updates")?;
    let (attachments, attachment_largest, attachment_bytes) = rows::attachment_sizes(conn)?;
    if checkpoints != 1 {
        return Err(invalid("The file has no saved state; keep it for recovery"));
    }
    let kind = if documents == 0 {
        if updates + attachments > 0 {
            return Err(invalid("A template holds no document edits"));
        }
        Kind::Template
    } else {
        Kind::Document
    };
    if !attachments_fit(attachments, attachment_largest, attachment_bytes) {
        return Err(invalid("Attachments exceed their limits"));
    }
    // Sizes first, as everywhere: then at most `ATTACHMENT_COUNT` IDs of 64 bytes are read.
    if one(conn, "SELECT count(*) FROM attachments WHERE length(CAST(id AS BLOB)) != 64")? > 0 {
        return Err(invalid("Invalid attachment identity"));
    }
    let mut ids = conn.prepare("SELECT CAST(id AS BLOB) FROM attachments").map_err(sqlite("read"))?;
    for id in ids.query_map([], |r| r.get::<_, Vec<u8>>(0)).map_err(sqlite("read"))? {
        let id = id.map_err(sqlite("read"))?;
        if !std::str::from_utf8(&id).is_ok_and(crate::wire::valid_attachment_id) {
            return Err(invalid("Invalid attachment identity"));
        }
    }
    Ok(kind)
}

/// Whether a document's attachments fit their limits, as stored or with one more.
pub(crate) fn attachments_fit(count: i64, largest: i64, total: i64) -> bool {
    count <= crate::ATTACHMENT_COUNT as i64
        && largest <= crate::ATTACHMENT_FILE_BYTES as i64
        && total <= crate::ATTACHMENT_BYTES as i64
}

/// Display metadata is deliberately not a certificate of full acceptance.
#[derive(Clone, Debug)]
pub struct Summary {
    pub kind: Kind,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub metadata: AppMetadata,
    pub bytes: u64,
}
/// Display metadata, bounded; the definition's decoder owns every other rule.
fn scalar_metadata(conn: &Connection) -> Result<AppMetadata> {
    let size = one(
        conn,
        "SELECT length(CAST(slug AS BLOB))+length(CAST(title AS BLOB))+length(CAST(description AS BLOB))+length(CAST(author_name AS BLOB))+coalesce(length(CAST(author_url AS BLOB)),0)+length(CAST(category_primary AS BLOB))+coalesce(length(CAST(category_secondary AS BLOB)),0) FROM app WHERE id=1",
    )?;
    if size > MANIFEST_BYTES as i64 {
        return Err(invalid("App metadata is too large"));
    }
    let (slug,title,description,name,url,primary,secondary) = conn.query_row(
        "SELECT slug,title,description,author_name,author_url,category_primary,category_secondary FROM app WHERE id=1", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?))
    ).map_err(sqlite("read metadata"))?;
    use crate::app::package_format_1::column_category;
    let mut categories = vec![column_category(&primary).map_err(Error::Rejected)?];
    if let Some(secondary) = secondary {
        categories.push(column_category(&secondary).map_err(Error::Rejected)?);
    }
    Ok(AppMetadata { slug, title, description, author: Author { name, url }, categories })
}
/// Common display preamble: marker dispatch, exact layout and bounded catalog fields.
/// No definition, app blob or attachment blob (even its length) is read here.
fn summary_on(conn: &Connection, path: &Path) -> Result<Summary> {
    let version = markers(conn)?;
    layout(conn, version)?;
    let (package_format, runtime_abi) = conn
        .query_row("SELECT package_format,runtime_abi FROM app WHERE id=1", [], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64))
        })
        .map_err(sqlite("read markers"))?;
    let metadata = scalar_metadata(conn)?;
    if one(conn, "SELECT count(*) FROM checkpoint")? != 1 {
        return Err(invalid("The file has no saved state; keep it for recovery"));
    }
    let kind = if one(conn, "SELECT count(*) FROM document")? == 0 { Kind::Template } else { Kind::Document };
    Ok(Summary {
        kind,
        package_format,
        runtime_abi,
        metadata,
        bytes: fs::metadata(resolve(path)?).map(|m| m.len()).unwrap_or(0),
    })
}
pub fn summary(path: &Path) -> Result<Summary> {
    let conn = reader(path)?;
    let read = conn.unchecked_transaction().map_err(sqlite("read"))?;
    summary_on(&read, path)
}

/// Fully accepted app and resources, kept by the owner for its entire lifetime.
pub struct OpenedApp {
    pub kind: Kind,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub app: AppDefinition,
    pub skin: Option<Vec<u8>>,
    pub style: bool,
    pub commands: Option<std::sync::Arc<String>>,
    pub bytes: u64,
}
pub(crate) fn opened(conn: &Connection, path: &Path, integrity: bool) -> Result<OpenedApp> {
    let _read = if conn.is_autocommit() { Some(conn.unchecked_transaction().map_err(sqlite("read"))?) } else { None };
    let summary = summary_on(conn, path)?;
    contents(conn, integrity)?;
    let definition: String = conn
        .query_row("SELECT definition_json FROM app WHERE id=1", [], |r| r.get(0))
        .map_err(sqlite("read definition"))?;
    let app = AppDefinition::decode(
        summary.package_format,
        summary.runtime_abi,
        &crate::encode(&summary.metadata),
        &definition,
    )
    .map_err(Error::Rejected)?;
    let mut keys = conn.prepare("SELECT key FROM assets ORDER BY key").map_err(sqlite("read assets"))?;
    let mut ui = false;
    let mut style = false;
    let mut skin = None;
    let mut commands = None;
    for key in keys.query_map([], |r| r.get::<_, String>(0)).map_err(sqlite("read assets"))? {
        let key = key.map_err(sqlite("read assets"))?;
        let kind = crate::media::asset_key(&key).ok_or_else(|| invalid("Invalid app asset key"))?;
        let bytes = read_asset(conn, &key)?.ok_or_else(|| invalid("Missing app asset"))?;
        // `pack` checked each asset's signature and the app row seals them; a later
        // build's signature rules never judge a saved app. Its content address is a
        // damage check that cannot change meaning.
        assets::check_resource(&key, &bytes, integrity)?;
        if key == "ui.css" {
            style = true;
        }
        if key == "ui.js" {
            ui = true;
        }
        if let WindowDefinition::Skin { width, height, skin: skin_key } = app.window()
            && &key == skin_key
        {
            if kind.media_type != "image/png" {
                return Err(invalid("Window skins must be PNG"));
            }
            crate::images::check(&bytes, crate::images::Purpose::Skin { width: *width, height: *height })
                .map_err(Error::Rejected)?;
            skin = Some(bytes.clone());
        }
        if key == "commands.js" {
            commands = Some(std::sync::Arc::new(String::from_utf8(bytes).map_err(invalid)?));
        }
    }
    if !ui {
        return Err(invalid("Missing ui.js"));
    }
    if matches!(app.window(), WindowDefinition::Skin { .. }) && skin.is_none() {
        return Err(invalid("Missing window skin"));
    }
    if commands.is_some() == app.commands().is_empty() {
        return Err(invalid("Commands and their private program must be present together"));
    }
    Ok(OpenedApp {
        kind: summary.kind,
        package_format: summary.package_format,
        runtime_abi: summary.runtime_abi,
        app,
        skin,
        style,
        commands,
        bytes: summary.bytes,
    })
}
pub fn open(path: &Path, integrity: bool) -> Result<OpenedApp> {
    opened(&reader(path)?, path, integrity)
}
pub(crate) fn checked(path: &Path) -> Result<(Connection, Kind)> {
    let conn = reader(path)?;
    let kind = summary_on(&conn, path)?.kind;
    Ok((conn, kind))
}
pub fn artwork(path: &Path, preferred: &[Artwork]) -> Result<Option<(Artwork, Vec<u8>)>> {
    let conn = reader(path)?;
    let read = conn.unchecked_transaction().map_err(sqlite("read"))?;
    summary_on(&read, path)?;
    for &name in preferred {
        if let Some(png) = rows::read_artwork(&read, name)? {
            return Ok(Some((name, png)));
        }
    }
    Ok(None)
}
pub fn kind(path: &Path) -> Result<Kind> {
    Ok(summary(path)?.kind)
}
/// Copies existing cosmetic artwork without running the app or changing its file.
/// A caller provides a fresh output path; existing files are never overwritten.
pub fn export_artwork(path: &Path, name: Artwork, output: &Path) -> Result<bool> {
    use std::io::Write;
    let Some((_, png)) = artwork(path, &[name])? else {
        return Ok(false);
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| failed(format!("Create artwork output: {e}")))?;
    file.write_all(&png).map_err(|e| failed(format!("Write artwork output: {e}")))?;
    Ok(true)
}

/// A summary for `slop inspect`: kind, markers, assets, artwork and the document's sizes,
/// of a file every open would accept.
pub fn inspect(path: &Path) -> Result<crate::engine::InspectInfo> {
    use crate::engine::{AttachmentTotals, FileKind, InspectInfo, NamedSize, StateSizes};
    let conn = reader(path)?;
    // The summary reads the state the checks passed.
    let conn = conn.unchecked_transaction().map_err(sqlite("read"))?;
    let opened = opened(&conn, path, true)?;
    let (updates, update_bytes, checkpoint_bytes) = rows::state_sizes(&conn)?;
    let (attachments, _, attachment_bytes) = rows::attachment_sizes(&conn)?;
    let size = |value: i64| u64::try_from(value).map_err(|_| invalid("Negative stored size"));
    let list = |sql: &str| -> Result<Vec<NamedSize>> {
        let mut statement = conn.prepare(sql).map_err(sqlite("inspect"))?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(sqlite("inspect"))?;
        rows.map(|row| {
            let (name, bytes) = row.map_err(sqlite("inspect"))?;
            Ok(NamedSize { name, bytes: size(bytes)? })
        })
        .collect()
    };
    Ok(InspectInfo {
        kind: match opened.kind {
            Kind::Template => FileKind::Template,
            Kind::Document => FileKind::Document,
        },
        package_format: opened.package_format,
        runtime_abi: opened.runtime_abi,
        metadata: opened.app.metadata().clone(),
        window: opened.app.page_window(),
        views: opened.app.views(),
        assets: list("SELECT key, size FROM assets ORDER BY key")?,
        artwork: list("SELECT name, length(png) FROM artwork ORDER BY name")?,
        defaults: opened.app.spec().theme_tokens().iter().cloned().collect(),
        attachments: AttachmentTotals { count: size(attachments)?, bytes: size(attachment_bytes)? },
        state: StateSizes {
            checkpoint_bytes: size(checkpoint_bytes)?,
            updates: size(updates)?,
            update_bytes: size(update_bytes)?,
        },
        bytes: opened.bytes,
        stored_asset_bytes: size(
            conn.query_row("SELECT coalesce(sum(length(bytes)),0) FROM assets", [], |r| r.get(0))
                .map_err(sqlite("inspect"))?,
        )?,
        live: crate::registry::discovery(path)?.is_some(),
    })
}

/// The app's descriptor, for `slop schema`, from a file every open would accept.
pub fn descriptor(path: &Path) -> Result<String> {
    Ok(open(path, false)?.app.document_json().into())
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    #[test]
    fn summary_cannot_read_the_definition_or_app_and_document_payloads() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(&format!("PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version=1;
            INSERT INTO checkpoint VALUES(1,x'00');
            INSERT INTO assets VALUES('ui.js','text/javascript','identity',1,x'00');
            INSERT INTO app VALUES(1,1,1,'fixture','Fixture','A fixture','Author',NULL,'utilities',NULL,'invalid JSON');")).unwrap();
        conn.authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Read { table_name, column_name }
                if table_name == "assets"
                    || table_name == "attachments"
                    || table_name == "updates"
                    || (table_name == "app" && column_name == "definition_json")
                    || (table_name == "checkpoint" && column_name == "bytes") =>
            {
                Authorization::Deny
            }
            _ => Authorization::Allow,
        }))
        .unwrap();
        for query in [
            "SELECT definition_json FROM app",
            "SELECT bytes FROM assets",
            "SELECT bytes FROM attachments",
            "SELECT bytes FROM checkpoint",
        ] {
            assert!(conn.prepare(query).is_err(), "authorizer must refuse {query}");
        }
        let summary = summary_on(&conn, Path::new("unused.slop")).unwrap();
        assert_eq!(summary.kind, Kind::Template);
        assert_eq!(summary.metadata.title, "Fixture");
        // An unreadable payload does not make a summary a validity certificate.
        assert!(opened(&conn, Path::new("unused.slop"), false).is_err());
    }
}
