//! A document is one SQLite file: the app its author built, the document's saved state, its
//! attachments and its artwork. This module owns that format: the tables and every statement
//! on them (every write, and the store's reads, in `rows`), the checks every open runs, packing a build into a template (`pack`),
//! creating and copying documents (`copy`), where documents may live (`places`), which
//! templates hosts list (`catalog`), and serving the app's assets (`assets`). `store` saves a document; `registry` holds its writer lock.

mod artwork;
mod assets;
mod catalog;
mod commands;
mod copy;
mod pack;
mod places;
pub(crate) mod rows;

pub use artwork::Artwork;
pub(crate) use artwork::{check_artwork, optimize_png};
pub use assets::{AssetReader, content_type, valid_asset_path};
pub use catalog::{Catalog, Folder, Template, find_template, list_templates, open_template, template_source};
pub use commands::{COMMAND_BUNDLE, COMMAND_METADATA, CommandAssets, commands, valid_call};
pub(crate) use copy::copy;
pub use copy::create_document;
pub use pack::{APP_INPUT_BYTES, pack, validate_app};
pub use places::{TemplateSource, template_roots};
pub(crate) use places::{document_destination, document_location};

use crate::error::{Error, Result, failed, invalid, requires_update, sqlite};
use crate::shape;
use crate::wire::{APP_TEXT_BYTES, ASSET_PATH_BYTES, MANIFEST_BYTES};
use artwork::png;
use assets::{assets_within, read_asset};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, config::DbConfig, limits::Limit};
use std::borrow::Cow;
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
    // Version 1 is the first public storage. The assertion above forces a real reader
    // and migration step to accompany a bump; no prelaunch legacy format is supported.
    let validate = version != STORAGE_VERSION;
    if validate {
        return Err(invalid("Missing storage migration"));
    }
    Ok(WriteTransaction { tx, validate })
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
    app_sizes(conn)?;
    stored_assets(conn)?;
    stored_artwork(conn)?;
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
    let markers =
        [("package format", package_format, crate::PACKAGE_FORMAT), ("runtime ABI", runtime_abi, crate::RUNTIME_ABI)];
    if let Some((name, level, supported)) = markers.iter().find(|(_, level, supported)| *level > *supported as i64) {
        return Err(requires_update(format!("This slop needs {name} {level}; this hitSlop supports {supported}")));
    }
    if let Some((name, ..)) = markers.iter().find(|(_, level, _)| *level < 1) {
        return Err(invalid(format!("Invalid {name}")));
    }
    Ok(())
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
/// The `app` row's text and every theme, within their budgets.
fn app_sizes(conn: &Connection) -> Result<()> {
    let (manifest, longest, theme) = (
        one(conn, "SELECT length(CAST(manifest AS BLOB)) FROM app")?,
        one(conn, "SELECT length(CAST(descriptor AS BLOB)) FROM app")?,
        one(conn, "SELECT length(CAST(theme AS BLOB)) FROM app")?,
    );
    if manifest > MANIFEST_BYTES as i64 || longest > APP_TEXT_BYTES as i64 || theme > crate::wire::THEME_LIMIT as i64 {
        return Err(invalid("The app's manifest, schema or theme is too large"));
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
            "SELECT count(*), coalesce(max(size),0), coalesce(sum(size),0), coalesce(max(length(CAST(path AS BLOB))),0) FROM assets",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(sqlite("read"))?;
    assets_within(assets as usize, largest as usize, total as usize)?;
    if longest_path > ASSET_PATH_BYTES as i64 {
        return Err(invalid("An asset path is too long"));
    }
    let mut paths = conn.prepare("SELECT path FROM assets").map_err(sqlite("read"))?;
    for path in paths.query_map([], |r| r.get::<_, String>(0)).map_err(sqlite("read"))? {
        if !valid_asset_path(&path.map_err(sqlite("read"))?) {
            return Err(invalid("Unsafe asset path"));
        }
    }
    Ok(())
}
/// The artwork's size, names and PNG headers.
fn stored_artwork(conn: &Connection) -> Result<()> {
    if one(conn, "SELECT coalesce(max(length(png)),0) FROM artwork")? > crate::ASSET_FILE_BYTES as i64 {
        return Err(invalid("Artwork is too large"));
    }
    if one(
        conn,
        &format!(
            "SELECT count(*) FROM artwork WHERE name NOT IN ('{}')",
            Artwork::ALL.map(Artwork::as_str).join("','")
        ),
    )? > 0
    {
        return Err(invalid("Unexpected artwork"));
    }
    // Artwork reaches Finder and Quick Look's image decoders: its PNG header is checked as
    // pack and every artwork write check it, reading only the header.
    let mut artwork = conn.prepare("SELECT name, substr(png,1,33) FROM artwork").map_err(sqlite("read"))?;
    for row in
        artwork.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))).map_err(sqlite("read"))?
    {
        let (name, header) = row.map_err(sqlite("read"))?;
        png(&header, &format!("The {name} artwork"))?;
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

/// What the author built, as `pack` writes it. Its initial values become the template's
/// checkpoint.
pub struct App {
    pub package_format: u64,
    pub runtime_abi: u64,
    /// The authored manifest, without the markers.
    pub manifest: String,
    pub descriptor: String,
    /// The declared colors and their defaults.
    pub theme: String,
}
/// What checking an app found: what its documents are instances of, its window shape, and
/// the window skin's PNG when the manifest names one.
struct CheckedApp {
    spec: crate::AppSpec,
    manifest: String,
    silhouette: shape::Silhouette,
    skin: Option<Vec<u8>>,
}
/// The content rules `pack` applies and every open relies on, each run once.
fn check_app_values(app: &App) -> Result<(crate::manifest::Window, crate::AppSpec, String)> {
    let (window, manifest) = crate::manifest::stored(&app.manifest, app.package_format).map_err(Error::Rejected)?;
    let schema = crate::descriptor::descriptor(&app.descriptor).map_err(Error::Rejected)?;
    let theme_tokens = crate::theme::validate_defaults(&app.theme).map_err(Error::Rejected)?;
    let spec = crate::AppSpec::of(schema, &window.slug, theme_tokens);
    Ok((window, spec, manifest))
}
/// An asset's bytes by key: borrowed from a stage being packed, read from a file.
type Assets<'a, 'b> = &'b dyn Fn(&str) -> Result<Option<Cow<'a, [u8]>>>;
fn check_app(app: &App, asset: Assets) -> Result<CheckedApp> {
    let (window, spec, manifest) = check_app_values(app)?;
    let entry = asset("app.js")?.ok_or_else(|| invalid("Missing assets/app.js"))?;
    std::str::from_utf8(&entry).map_err(|_| invalid("assets/app.js must be UTF-8"))?;
    commands::validate(asset(COMMAND_METADATA)?.as_deref(), asset(COMMAND_BUNDLE)?.as_deref())?;
    let skin = match &window.skin {
        None => None,
        Some(skin) => {
            let key = skin.strip_prefix("assets/").ok_or_else(|| invalid("The window skin must be an asset"))?;
            let bytes = asset(key)?.ok_or_else(|| invalid(format!("Missing window skin {skin}")))?;
            let (width, height, alpha) = png(&bytes, "window skin")?;
            if (width as u64, height as u64) != (window.width, window.height) {
                return Err(invalid("The window skin must match the window's width and height in pixels"));
            }
            if !alpha {
                return Err(invalid("The window skin must be an RGBA PNG with alpha"));
            }
            Some(bytes.into_owned())
        }
    };
    Ok(CheckedApp { spec, manifest, silhouette: window.silhouette, skin })
}

/// A template or document a host opened: its kind and markers, its app as stored, and what
/// checking the app found. Each open checks the app once and keeps this.
pub struct OpenedApp {
    pub kind: Kind,
    pub app: App,
    /// Normalized host-facing manifest. The stored manifest is never rewritten on read.
    pub manifest: String,
    /// What its documents are instances of: the descriptor and the declared palette.
    pub spec: crate::AppSpec,
    pub silhouette: shape::Silhouette,
    /// The window skin's PNG, when the manifest names one.
    pub skin: Option<Vec<u8>>,
    /// The file's size in bytes when it was opened.
    pub bytes: u64,
}
/// Checks the file at `path` on `conn`, then reads and checks its app, in one read
/// transaction (the caller's, if it holds one): the app read is the one the checks passed
/// while another process saves.
pub(crate) fn opened(conn: &Connection, path: &Path, integrity: bool) -> Result<OpenedApp> {
    let _read = if conn.is_autocommit() { Some(conn.unchecked_transaction().map_err(sqlite("read"))?) } else { None };
    let kind = check(conn, integrity)?;
    let app = read_app(conn)?;
    let found = check_app(&app, &|key| Ok(read_asset(conn, key)?.map(Cow::Owned)))?;
    Ok(OpenedApp {
        kind,
        spec: found.spec,
        manifest: found.manifest,
        silhouette: found.silhouette,
        skin: found.skin,
        bytes: fs::metadata(resolve(path)?).map(|m| m.len()).unwrap_or(0),
        app,
    })
}
/// Opens and checks a template or a document without its writer lock. `integrity` adds
/// SQLite's quick check; display-only opens (the catalog, Quick Look) leave it out.
pub fn open(path: &Path, integrity: bool) -> Result<OpenedApp> {
    opened(&reader(path)?, path, integrity)
}
/// A reader on the file at `path`, and its kind, once the checks every open runs pass.
pub(crate) fn checked(path: &Path) -> Result<(Connection, Kind)> {
    let conn = reader(path)?;
    let kind = check(&conn, false)?;
    Ok((conn, kind))
}
/// The first of `preferred` artwork (`preview`, `icon`) the file holds, by name: one read,
/// for a host displaying the file (Quick Look, the catalog, a window's icon). A file that
/// cannot be read now (busy, or mid-recovery) is an error, never "no artwork".
pub fn artwork(path: &Path, preferred: &[Artwork]) -> Result<Option<(Artwork, Vec<u8>)>> {
    let (conn, _) = checked(path)?;
    for &name in preferred {
        if let Some(png) = rows::read_artwork(&conn, name)? {
            return Ok(Some((name, png)));
        }
    }
    Ok(None)
}
/// A file's kind from its checks alone, for a host deciding how to open it.
pub fn kind(path: &Path) -> Result<Kind> {
    checked(path).map(|(_, kind)| kind)
}
fn read_app(conn: &Connection) -> Result<App> {
    conn.query_row("SELECT package_format,runtime_abi,manifest,descriptor,theme FROM app WHERE id=1", [], |r| {
        Ok(App {
            package_format: r.get::<_, i64>(0)? as u64,
            runtime_abi: r.get::<_, i64>(1)? as u64,
            manifest: r.get(2)?,
            descriptor: r.get(3)?,
            theme: r.get(4)?,
        })
    })
    .map_err(sqlite("read app"))
}
/// A summary for `slop inspect`: kind, markers, assets, artwork and the document's sizes,
/// of a file every open would accept.
pub fn inspect(path: &Path) -> Result<serde_json::Value> {
    let conn = reader(path)?;
    // The summary reads the state the checks passed.
    let conn = conn.unchecked_transaction().map_err(sqlite("read"))?;
    let opened = opened(&conn, path, true)?;
    let (updates, update_bytes, checkpoint_bytes) = rows::state_sizes(&conn)?;
    let (attachments, _, attachment_bytes) = rows::attachment_sizes(&conn)?;
    let list = |sql: &str| -> Result<Vec<serde_json::Value>> {
        let mut statement = conn.prepare(sql).map_err(sqlite("inspect"))?;
        let rows = statement
            .query_map([], |r| Ok(serde_json::json!({ "name": r.get::<_, String>(0)?, "bytes": r.get::<_, i64>(1)? })))
            .map_err(sqlite("inspect"))?;
        rows.collect::<rusqlite::Result<_>>().map_err(sqlite("inspect"))
    };
    let one = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(sqlite("inspect"));
    Ok(serde_json::json!({
        "kind": match opened.kind { Kind::Template => "template", Kind::Document => "document" },
        "packageFormat": opened.app.package_format,
        "runtimeABI": opened.app.runtime_abi,
        "manifest": serde_json::from_str::<serde_json::Value>(&opened.manifest).map_err(failed)?,
        "assets": list("SELECT path, size FROM assets ORDER BY path")?,
        "artwork": list("SELECT name, length(png) FROM artwork ORDER BY name")?,
        "attachments": { "count": attachments, "bytes": attachment_bytes },
        "state": { "checkpointBytes": checkpoint_bytes, "updates": updates, "updateBytes": update_bytes },
        "bytes": opened.bytes,
        "storedAssetBytes": one("SELECT coalesce(sum(length(bytes)),0) FROM assets")?,
    }))
}

/// The app's descriptor, for `slop schema`, from a file every open would accept.
pub fn descriptor(path: &Path) -> Result<String> {
    Ok(open(path, false)?.app.descriptor)
}
