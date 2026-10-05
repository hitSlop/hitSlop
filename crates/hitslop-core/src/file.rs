//! A document is one SQLite file: the app its author built, the document's saved state, its
//! attachments and its artwork. This module owns that format: the tables, the checks every
//! open runs, packing a build into a template, creating and copying documents, and serving
//! the app's assets. `store` saves a document; `registry` holds its writer lock.

use crate::error::{failed, invalid, rejected, requires_update, sqlite, Error, Result};
use crate::{shape, Code};
use rusqlite::{config::DbConfig, limits::Limit, params, Connection, OpenFlags, OptionalExtension, MAIN_DB};
use std::borrow::Cow;
use std::ffi::CString;
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub(crate) const APPLICATION_ID: i64 = 0x4853_4C50; // HSLP
/// These tables. A compatibility requirement, not a release number: a build refuses a
/// newer one with `requires_update`.
pub(crate) const STORAGE_VERSION: i64 = 1;
/// `app` is what the author built, written once by `pack` and identical in a template and
/// its documents, so saved state always belongs to its descriptor. `document`,
/// `checkpoint`, `updates` and `attachments` are the document; a template has no rows in
/// them. An asset's `size` is its length; `encoding` is how `bytes` holds it (`encode`).
pub(crate) const SCHEMA: &str = "\
CREATE TABLE app(id INTEGER PRIMARY KEY CHECK(id=1), package_format INTEGER NOT NULL, runtime_abi INTEGER NOT NULL, manifest TEXT NOT NULL, descriptor TEXT NOT NULL, initial TEXT NOT NULL, theme TEXT NOT NULL);
CREATE TABLE assets(path TEXT PRIMARY KEY, encoding TEXT NOT NULL CHECK(encoding IN ('identity','br')), size INTEGER NOT NULL, bytes BLOB NOT NULL);
CREATE TABLE artwork(name TEXT PRIMARY KEY CHECK(name IN ('preview','icon')), png BLOB NOT NULL);
CREATE TABLE document(id INTEGER PRIMARY KEY CHECK(id=1));
CREATE TABLE checkpoint(id INTEGER PRIMARY KEY CHECK(id=1), bytes BLOB NOT NULL);
CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);
CREATE TABLE attachments(id TEXT PRIMARY KEY, bytes BLOB NOT NULL);";

use crate::wire::{APP_TEXT_BYTES, ASSET_PATH_BYTES, MANIFEST_BYTES};
/// The artwork a file may hold, as the `artwork` table's CHECK names it.
pub(crate) const ARTWORK: [&str; 2] = ["preview", "icon"];

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
    let conn = Connection::open_with_flags(resolve(path)?, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW)
        .map_err(sqlite("open"))?;
    conn.busy_timeout(busy).map_err(sqlite("open"))?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true).map_err(sqlite("open"))?;
    // Closing never checkpoints: a file in WAL mode, which only a newer build writes, is
    // refused, never rewritten.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true).map_err(sqlite("open"))?;
    conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA cell_size_check=ON; PRAGMA mmap_size=0;").map_err(sqlite("open"))?;
    conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, crate::STORAGE_BYTES as i32).map_err(sqlite("open"))?;
    Ok(conn)
}
/// The writer's durability: a rollback journal that exists only while a save commits, and
/// a full sync of every commit.
pub(crate) fn configure_writer(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON;").map_err(sqlite("configure"))
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

/// A template holds only the app; a document also holds its saved state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Template,
    Document,
}

type SchemaRow = (String, String, String, Option<String>);
fn tables(conn: &Connection) -> Result<Vec<SchemaRow>> {
    let mut statement =
        conn.prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").map_err(sqlite("read tables"))?;
    let rows = statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(sqlite("read tables"))?;
    rows.collect::<rusqlite::Result<_>>().map_err(sqlite("read tables"))
}
/// The tables and indexes `SCHEMA` makes, including SQLite's automatic primary-key
/// indexes; built once.
fn expected_tables() -> &'static [SchemaRow] {
    static LAYOUT: OnceLock<Vec<SchemaRow>> = OnceLock::new();
    LAYOUT.get_or_init(|| {
        let memory = Connection::open_in_memory().expect("in-memory database");
        memory.execute_batch(SCHEMA).expect("valid schema");
        tables(&memory).expect("readable tables")
    })
}

/// The saved state's sizes: update rows, update bytes and checkpoint bytes.
pub(crate) const STATE_SIZES: &str =
    "SELECT (SELECT count(*) FROM updates),(SELECT coalesce(sum(length(bytes)),0) FROM updates),(SELECT coalesce(sum(length(bytes)),0) FROM checkpoint)";

/// The checks every open runs, in order, reading sizes (`length()`, an asset's recorded
/// size) and never a value. The application ID, the storage version and the app's
/// requirements come first, so a newer file is refused with `requires_update` even where a
/// newer build changed the tables they describe; then the exact tables, the rows a template
/// or a document may hold, and every size budget. `integrity` adds `PRAGMA quick_check`
/// (42 ms for the largest allowed document). The checks run in one read transaction, the
/// caller's if it holds one (`opened`), so they see one state while another process saves.
pub(crate) fn check(conn: &Connection, integrity: bool) -> Result<Kind> {
    let _read = if conn.is_autocommit() { Some(conn.unchecked_transaction().map_err(sqlite("read"))?) } else { None };
    markers(conn)?;
    layout(conn)?;
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
fn markers(conn: &Connection) -> Result<()> {
    if one(conn, "PRAGMA application_id")? != APPLICATION_ID {
        return Err(invalid("This is not a hitSlop document"));
    }
    let version = one(conn, "PRAGMA user_version")?;
    if version > STORAGE_VERSION {
        return Err(requires_update(format!("This document uses storage version {version}; this hitSlop reads version {STORAGE_VERSION}")));
    }
    if version != STORAGE_VERSION {
        return Err(invalid("Unsupported document storage"));
    }
    // Two named columns, which any newer app format keeps; the tables are compared next.
    let found = conn.query_row("SELECT package_format, runtime_abi FROM app WHERE id=1", [], |r| Ok((r.get(0)?, r.get(1)?)));
    let (package_format, runtime_abi) = match found {
        Ok(markers) => markers,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(invalid("Unexpected rows in app")),
        // No such table or column, or a marker that is not an integer.
        Err(rusqlite::Error::InvalidColumnType(..)) => return Err(invalid("Unexpected document tables")),
        Err(e) if e.sqlite_error_code() == Some(rusqlite::ffi::ErrorCode::Unknown) => return Err(invalid("Unexpected document tables")),
        Err(e) => return Err(sqlite("read")(e)),
    };
    requirements(package_format, runtime_abi)
}
/// The app's requirements, as `pack` reads them from a build and every open from a file:
/// none newer than this build supports, and each at least 1.
fn requirements(package_format: i64, runtime_abi: i64) -> Result<()> {
    let markers = [("package format", package_format, crate::PACKAGE_FORMAT), ("runtime ABI", runtime_abi, crate::RUNTIME_ABI)];
    if let Some((name, level, supported)) = markers.iter().find(|(_, level, supported)| *level > *supported as i64) {
        return Err(requires_update(format!("This slop needs {name} {level}; this hitSlop supports {supported}")));
    }
    if let Some((name, ..)) = markers.iter().find(|(_, level, _)| *level < 1) {
        return Err(invalid(format!("Invalid {name}")));
    }
    Ok(())
}
/// The exact tables, and at most one row, row 1, in each one-row table.
fn layout(conn: &Connection) -> Result<()> {
    if tables(conn)? != expected_tables() {
        return Err(invalid("Unexpected document tables"));
    }
    // A file written without its CHECK constraints may hold other rows, which no read
    // would see.
    for table in ["app", "document", "checkpoint"] {
        let (rows, first): (i64, i64) = conn
            .query_row(&format!("SELECT count(*), coalesce(sum(id=1),0) FROM {table}"), [], |r| Ok((r.get(0)?, r.get(1)?)))
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
        one(conn, "SELECT max(length(CAST(descriptor AS BLOB)), length(CAST(initial AS BLOB))) FROM app")?,
        one(conn, "SELECT length(CAST(theme AS BLOB)) FROM app")?,
    );
    if manifest > MANIFEST_BYTES as i64 || longest > APP_TEXT_BYTES as i64 || theme > crate::wire::THEME_LIMIT as i64 {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    Ok(())
}
/// The assets' encodings, budgets and paths.
fn stored_assets(conn: &Connection) -> Result<()> {
    // Stored as it is, an asset is its size; compressed, it stores less, and decoding stops
    // at its size. The budgets then bound what decoding produces.
    if one(conn, "SELECT count(*) FROM assets WHERE typeof(size)!='integer' OR CASE encoding WHEN 'identity' THEN length(bytes)!=size WHEN 'br' THEN length(bytes)>=size ELSE 1 END")? > 0 {
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
    if one(conn, &format!("SELECT count(*) FROM artwork WHERE name NOT IN ('{}')", ARTWORK.join("','")))? > 0 {
        return Err(invalid("Unexpected artwork"));
    }
    // Artwork reaches Finder and Quick Look's image decoders: its PNG header is checked as
    // pack and every artwork write check it, reading only the header.
    let mut artwork = conn.prepare("SELECT name, substr(png,1,33) FROM artwork").map_err(sqlite("read"))?;
    for row in artwork.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))).map_err(sqlite("read"))? {
        let (name, header) = row.map_err(sqlite("read"))?;
        png(&header, &format!("The {name} artwork"))?;
    }
    Ok(())
}
/// Whether the file is a template or a document: a template holds no document state. A
/// document's attachments are within their limits and named by their hashes; the saved
/// state's own budget is the store's (`checked_bounds`).
fn state(conn: &Connection) -> Result<Kind> {
    let documents = one(conn, "SELECT count(*) FROM document")?;
    let checkpoints = one(conn, "SELECT count(*) FROM checkpoint")?;
    let updates = one(conn, "SELECT count(*) FROM updates")?;
    let (attachments, attachment_largest, attachment_bytes): (i64, i64, i64) = conn
        .query_row("SELECT count(*), coalesce(max(length(bytes)),0), coalesce(sum(length(bytes)),0) FROM attachments", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(sqlite("read"))?;
    let kind = if documents == 0 {
        if checkpoints + updates + attachments > 0 {
            return Err(invalid("A template holds no document state"));
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

/// The asset budget, as packing reads a stage and as every open finds it stored.
fn assets_within(count: usize, largest: usize, total: usize) -> Result<()> {
    if count > crate::ASSET_COUNT || largest > crate::ASSET_FILE_BYTES || total > crate::ASSET_BYTES {
        return Err(invalid(format!(
            "The app exceeds {} assets, {} MiB per asset or {} MiB",
            crate::ASSET_COUNT,
            crate::ASSET_FILE_BYTES >> 20,
            crate::ASSET_BYTES >> 20
        )));
    }
    Ok(())
}
/// Whether a document's attachments fit their limits, as stored or with one more.
pub(crate) fn attachments_fit(count: i64, largest: i64, total: i64) -> bool {
    count <= crate::ATTACHMENT_COUNT as i64 && largest <= crate::ATTACHMENT_FILE_BYTES as i64 && total <= crate::ATTACHMENT_BYTES as i64
}
/// A relative asset path: no empty or dot segments, no backslash or NUL.
pub fn valid_asset_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= ASSET_PATH_BYTES
        && !path.starts_with('/')
        && !path.contains(['\\', '\0'])
        && path.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}
/// The content type a served asset carries, by its extension.
pub fn content_type(path: &str) -> &'static str {
    let extension = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match extension.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "css" => "text/css",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
/// Artwork as every open accepts it: a PNG within the image limits, at most one asset's size.
pub(crate) fn check_artwork(label: &str, bytes: &[u8]) -> Result<()> {
    if bytes.len() > crate::ASSET_FILE_BYTES {
        return Err(invalid(format!("{label} is too large")));
    }
    png(bytes, label).map(|_| ())
}
/// A PNG's width and height, and whether it carries alpha (colour type 6), from its header.
fn png(bytes: &[u8], label: &str) -> Result<(u32, u32, bool)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 33 || bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return Err(invalid(format!("{label} must be a valid PNG")));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if width == 0 || height == 0 || width as usize > crate::IMAGE_SIDE || height as usize > crate::IMAGE_SIDE
        || (width as usize) * (height as usize) > crate::IMAGE_PIXELS
    {
        return Err(invalid(format!("{label} exceeds the PNG dimension limit")));
    }
    Ok((width, height, bytes[25] == 6))
}

/// What the author built, as `pack` writes it.
pub struct App {
    pub package_format: u64,
    pub runtime_abi: u64,
    /// The authored manifest, without the markers.
    pub manifest: String,
    pub descriptor: String,
    pub initial: String,
    /// The declared colors and their defaults.
    pub theme: String,
}
/// What checking an app found: its template's slug, its parsed descriptor, its window shape, its declared colors in the order the author wrote them, and the window
/// skin's PNG when the manifest names one.
struct CheckedApp {
    slug: String,
    schema: crate::Node,
    silhouette: shape::Silhouette,
    theme_tokens: Vec<(String, String)>,
    skin: Option<Vec<u8>>,
}
/// The content rules `pack` applies and every open relies on, each run once.
fn check_app_values(app: &App) -> Result<(crate::manifest::Window, crate::Node, Vec<(String, String)>)> {
    let window = crate::manifest::validate(&app.manifest, app.package_format).map_err(Error::Rejected)?;
    let schema = crate::descriptor::checked(&app.descriptor, &app.initial).map_err(Error::Rejected)?;
    let theme_tokens = crate::theme::validate_defaults(&app.theme).map_err(Error::Rejected)?;
    Ok((window, schema, theme_tokens))
}
fn check_app(app: &App, asset: &dyn Fn(&str) -> Result<Option<Vec<u8>>>) -> Result<CheckedApp> {
    let (window, schema, theme_tokens) = check_app_values(app)?;
    let entry = asset("app.js")?.ok_or_else(|| invalid("Missing assets/app.js"))?;
    std::str::from_utf8(&entry).map_err(|_| invalid("assets/app.js must be UTF-8"))?;
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
            Some(bytes)
        }
    };
    Ok(CheckedApp { slug: window.slug, schema, silhouette: window.silhouette, theme_tokens, skin })
}

/// A template or document a host opened: its kind and markers, its app as stored, and what
/// checking the app found. Each open checks the app once and keeps this.
pub struct OpenedApp {
    pub kind: Kind,
    pub app: App,
    /// The template's slug, which names it in theme files.
    pub slug: String,
    pub(crate) schema: crate::Node,
    pub silhouette: shape::Silhouette,
    pub theme_tokens: Vec<(String, String)>,
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
    let found = check_app(&app, &|key| read_asset(conn, key))?;
    Ok(OpenedApp {
        kind,
        slug: found.slug,
        schema: found.schema,
        silhouette: found.silhouette,
        theme_tokens: found.theme_tokens,
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
pub fn artwork(path: &Path, preferred: &[&str]) -> Result<Option<(String, Vec<u8>)>> {
    let (conn, _) = checked(path)?;
    for name in preferred {
        if let Some(png) = read_artwork(&conn, name)? {
            return Ok(Some((name.to_string(), png)));
        }
    }
    Ok(None)
}
/// One artwork image by name, when the file holds it.
pub(crate) fn read_artwork(conn: &Connection, name: &str) -> Result<Option<Vec<u8>>> {
    conn.prepare_cached("SELECT png FROM artwork WHERE name=?")
        .and_then(|mut s| s.query_row([name], |r| r.get(0)).optional())
        .map_err(sqlite("read artwork"))
}
/// A file's kind from its checks alone, for a host deciding how to open it.
pub fn kind(path: &Path) -> Result<Kind> {
    checked(path).map(|(_, kind)| kind)
}
fn read_app(conn: &Connection) -> Result<App> {
    conn.query_row("SELECT package_format,runtime_abi,manifest,descriptor,initial,theme FROM app WHERE id=1", [], |r| {
        Ok(App {
            package_format: r.get::<_, i64>(0)? as u64,
            runtime_abi: r.get::<_, i64>(1)? as u64,
            manifest: r.get(2)?,
            descriptor: r.get(3)?,
            initial: r.get(4)?,
            theme: r.get(5)?,
        })
    })
    .map_err(sqlite("read app"))
}
/// An asset's bytes, decoded.
fn read_asset(conn: &Connection, key: &str) -> Result<Option<Vec<u8>>> {
    let row: Option<(String, i64, Vec<u8>)> = conn
        .prepare_cached("SELECT encoding,size,bytes FROM assets WHERE path=?")
        .and_then(|mut s| s.query_row([key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional())
        .map_err(sqlite("read asset"))?;
    row.map(|(encoding, size, bytes)| decode(&encoding, size as usize, bytes)).transpose()
}
/// How `pack` stores an asset: text and WebAssembly Brotli-compressed when that is smaller,
/// anything else as it is, so media ranges read straight from the file. Quality 10 stores
/// within a page or so of 11 in half the time; past 4 MiB it takes seconds, so larger
/// assets use 9.
fn encode<'a>(key: &str, bytes: &'a [u8]) -> Result<(&'static str, Cow<'a, [u8]>)> {
    let kind = content_type(key);
    if kind.starts_with("text/") || matches!(kind, "application/json" | "image/svg+xml" | "application/wasm") {
        let params = brotli::enc::BrotliEncoderParams {
            quality: if bytes.len() <= 4 << 20 { 10 } else { 9 },
            lgwin: 22,
            size_hint: bytes.len(),
            ..Default::default()
        };
        let mut compressed = Vec::new();
        brotli::BrotliCompress(&mut &bytes[..], &mut compressed, &params).map_err(failed)?;
        if compressed.len() < bytes.len() {
            return Ok(("br", Cow::Owned(compressed)));
        }
    }
    Ok(("identity", Cow::Borrowed(bytes)))
}
/// An asset's stored bytes, decoded to the `size` `check` bounded.
fn decode(encoding: &str, size: usize, stored: Vec<u8>) -> Result<Vec<u8>> {
    match encoding {
        "identity" => Ok(stored),
        "br" => {
            let mut bytes = Vec::with_capacity(size);
            let read = brotli::Decompressor::new(stored.as_slice(), 4096).take(size as u64 + 1).read_to_end(&mut bytes);
            if read.is_ok() && bytes.len() == size {
                Ok(bytes)
            } else {
                Err(failed("An app asset is damaged; keep the file for recovery"))
            }
        }
        _ => Err(invalid("Invalid asset encoding")),
    }
}

/// Serves the app's assets from one long-lived connection: whole, or a byte range. An asset
/// stored as it is is read without loading the rest; compressed text is decoded whole.
pub struct AssetReader {
    conn: Connection,
}
impl AssetReader {
    /// A reader on `conn`, a connection to a file its store checked (`Store::asset_reader`).
    pub(crate) fn new(conn: Connection) -> Self {
        Self { conn }
    }
    fn row(&self, key: &str) -> Result<Option<(i64, bool, u64)>> {
        self.conn
            .prepare_cached("SELECT rowid, encoding='identity', size FROM assets WHERE path=?")
            .and_then(|mut s| s.query_row([key], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64))).optional())
            .map_err(sqlite("read asset"))
    }
    /// The asset's size in bytes, when it exists.
    pub fn size(&self, key: &str) -> Result<Option<u64>> {
        Ok(self.row(key)?.map(|(_, _, size)| size))
    }
    /// `length` bytes from `offset`, clamped to the asset.
    pub fn read_range(&self, key: &str, offset: u64, length: u64) -> Result<Option<Vec<u8>>> {
        let Some((row, identity, size)) = self.row(key)? else { return Ok(None) };
        let length = length.min(size.saturating_sub(offset)) as usize;
        if !identity {
            let Some(mut bytes) = read_asset(&self.conn, key)? else { return Ok(None) };
            let start = offset.min(size) as usize;
            bytes.truncate(start + length);
            bytes.drain(..start);
            return Ok(Some(bytes));
        }
        let blob = self.conn.blob_open(MAIN_DB, "assets", "bytes", row, true).map_err(sqlite("read asset"))?;
        let mut buffer = vec![0u8; length];
        blob.read_at_exact(&mut buffer, offset as usize).map_err(sqlite("read asset"))?;
        Ok(Some(buffer))
    }
}

/// A private temporary file beside `dest`, removed if it is never published.
pub(crate) struct Staged {
    path: PathBuf,
    published: bool,
}
impl Staged {
    pub(crate) fn beside(dest: &Path) -> Result<Self> {
        let dest = resolve(dest)?;
        let name = dest.file_name().unwrap().to_string_lossy();
        Ok(Self { path: dest.with_file_name(format!(".{name}.{}.tmp", crate::random_hex(8))), published: false })
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    /// Publishes the finished file without replacing anything at `dest`.
    pub(crate) fn publish_new(mut self, dest: &Path) -> Result<()> {
        let (from, to) = (cstring(&self.path)?, cstring(&resolve(dest)?)?);
        #[cfg(target_os = "macos")]
        // SAFETY: valid C strings.
        let status = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(target_os = "linux")]
        // SAFETY: valid C strings.
        let status = unsafe { libc::renameat2(libc::AT_FDCWD, from.as_ptr(), libc::AT_FDCWD, to.as_ptr(), libc::RENAME_NOREPLACE) };
        if status != 0 {
            let error = std::io::Error::last_os_error();
            return Err(if error.raw_os_error() == Some(libc::EEXIST) {
                rejected(Code::Exists, "A file already exists there")
            } else {
                failed(format!("Cannot save the document: {error}"))
            });
        }
        self.published = true;
        sync_folder(dest);
        Ok(())
    }
    /// Publishes over a template, never over a document.
    fn publish_template(mut self, dest: &Path) -> Result<()> {
        if let Ok(meta) = fs::symlink_metadata(dest) {
            if !meta.is_file() {
                return Err(rejected(Code::Exists, format!("Refusing to replace {}: it is not a template", dest.display())));
            }
            if checked(dest)?.1 != Kind::Template {
                return Err(rejected(Code::Exists, "Refusing to replace a document with a template"));
            }
            fs::rename(&self.path, resolve(dest)?).map_err(|e| failed(format!("Cannot save the template: {e}")))?;
            self.published = true;
            sync_folder(dest);
            return Ok(());
        }
        self.publish_new(dest)
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(self.path.with_extension("tmp-journal"));
        }
    }
}
fn cstring(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| failed("Invalid path"))
}
fn sync_folder(path: &Path) {
    if let Some(folder) = resolve(path).ok().and_then(|p| p.parent().map(Path::to_owned)) {
        if let Ok(file) = fs::File::open(folder) {
            let _ = file.sync_all();
        }
    }
}

/// Writes a template: the app's row, its assets and its artwork, in one transaction.
fn write_template(path: &Path, app: &App, assets: &[(String, Vec<u8>)], artwork: &[(String, Vec<u8>)]) -> Result<()> {
    let conn = writer(path, true)?;
    conn.execute_batch("PRAGMA auto_vacuum=FULL;").map_err(sqlite("create"))?;
    configure_writer(&conn)?;
    let tx = conn.unchecked_transaction().map_err(sqlite("create"))?;
    tx.execute_batch(&format!("PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={STORAGE_VERSION}; {SCHEMA}"))
        .map_err(sqlite("create"))?;
    tx.execute(
        "INSERT INTO app VALUES(1,?,?,?,?,?,?)",
        params![app.package_format as i64, app.runtime_abi as i64, app.manifest, app.descriptor, app.initial, app.theme],
    )
    .map_err(sqlite("create"))?;
    for (key, bytes) in assets {
        let (encoding, stored) = encode(key, bytes)?;
        tx.execute("INSERT INTO assets VALUES(?,?,?,?)", params![key, encoding, bytes.len() as i64, stored.as_ref()]).map_err(sqlite("create"))?;
    }
    for (name, png) in artwork {
        tx.execute("INSERT INTO artwork VALUES(?,?)", params![name, png]).map_err(sqlite("create"))?;
    }
    tx.commit().map_err(sqlite("create"))?;
    conn.close().map_err(|(_, e)| sqlite("close")(e))
}

/// Packs a build's stage into a template at `dest`. The stage is the build's folder:
/// `app.json` (the `app` row: requirements, manifest, descriptor, initial values and
/// theme), `assets/` and optional `artwork/preview.png` and `artwork/icon.png`. Everything
/// is checked before the template is published; a rebuild replaces a template, never a
/// document.
pub fn pack(stage: &Path, dest: &Path) -> Result<()> {
    let file = stage.join("app.json");
    let size = fs::metadata(&file).map_err(|e| invalid(format!("app.json: {e}")))?.len();
    if size > APP_INPUT_BYTES as u64 {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    let text = fs::read_to_string(&file).map_err(|e| invalid(format!("app.json: {e}")))?;
    let app = parse_app(&text)?;
    let assets = stage_assets(&stage.join("assets"))?;
    let mut artwork = vec![];
    for name in ARTWORK {
        let file = stage.join("artwork").join(format!("{name}.png"));
        match fs::symlink_metadata(&file) {
            Ok(meta) if meta.is_file() => {
                let bytes = fs::read(&file).map_err(failed)?;
                check_artwork(&format!("artwork/{name}.png"), &bytes)?;
                artwork.push((name.to_string(), bytes));
            }
            Ok(_) => return Err(invalid(format!("artwork/{name}.png must be a regular file"))),
            Err(_) => {}
        }
    }
    let lookup = |key: &str| Ok(assets.iter().find(|(k, _)| k == key).map(|(_, b)| b.clone()));
    check_app(&app, &lookup)?;
    let staged = Staged::beside(dest)?;
    write_template(staged.path(), &app, &assets, &artwork)?;
    if check(&reader(staged.path())?, true)? != Kind::Template {
        return Err(failed("Packing produced document state"));
    }
    staged.publish_template(dest)
}
/// Maximum evaluated app row accepted by packing and authoring validation.
pub const APP_INPUT_BYTES: usize = MANIFEST_BYTES + 2 * APP_TEXT_BYTES + crate::wire::THEME_LIMIT;

fn parse_app(input: &str) -> Result<App> {
    if input.len() > APP_INPUT_BYTES {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    let row: crate::wire::AppRow = serde_json::from_str(input).map_err(|e| invalid(format!("app.json: {e}")))?;
    requirements(row.packageFormat.try_into().unwrap_or(i64::MAX), row.runtimeABI.try_into().unwrap_or(i64::MAX))?;
    if row.manifest.get().len() > MANIFEST_BYTES || row.descriptor.get().len() > APP_TEXT_BYTES
        || row.initial.get().len() > APP_TEXT_BYTES || row.theme.get().len() > crate::wire::THEME_LIMIT {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    Ok(App {
        package_format: row.packageFormat,
        runtime_abi: row.runtimeABI,
        manifest: compact(row.manifest.get()),
        descriptor: compact(row.descriptor.get()),
        initial: compact(row.initial.get()),
        theme: compact(row.theme.get()),
    })
}
/// Checks evaluated app values with the same rules as packing and opening; assets are
/// checked later when they exist. Refuses unsupported markers before interpreting values.
pub fn validate_app(input: &str) -> Result<()> {
    check_app_values(&parse_app(input)?).map(|_| ())
}
/// Parsed JSON text without the whitespace between its tokens, in the order written: the
/// `app` row's text is one line, so hosts pass it on without re-encoding it.
fn compact(json: &str) -> String {
    let (mut out, mut quoted, mut escaped) = (String::with_capacity(json.len()), false, false);
    for c in json.chars() {
        if quoted {
            (escaped, quoted) = (!escaped && c == '\\', escaped || c != '"');
        } else if c.is_ascii_whitespace() {
            continue;
        } else {
            quoted = c == '"';
        }
        out.push(c);
    }
    out
}
/// The stage's assets: regular files only, within the budgets.
fn stage_assets(root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut assets = vec![];
    let (mut count, mut largest, mut total) = (0usize, 0usize, 0usize);
    let mut pending = vec![root.to_owned()];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).map_err(|e| invalid(format!("assets: {e}")))? {
            let path = entry.map_err(failed)?.path();
            let meta = fs::symlink_metadata(&path).map_err(failed)?;
            if meta.is_dir() {
                pending.push(path);
                continue;
            }
            if !meta.is_file() {
                return Err(invalid("Assets must be regular files, without symbolic links"));
            }
            let key = path.strip_prefix(root).map_err(|_| invalid("An asset escapes its folder"))?.to_string_lossy().into_owned();
            if !valid_asset_path(&key) {
                return Err(invalid(format!("Unsafe asset path {key}")));
            }
            // Checked before reading, so a stage of huge files is refused, never loaded.
            (count, largest, total) = (count + 1, largest.max(meta.len() as usize), total + meta.len() as usize);
            assets_within(count, largest, total)?;
            assets.push((key, fs::read(&path).map_err(failed)?));
        }
    }
    assets.sort();
    Ok(assets)
}

/// Copies `source` to `dest` through SQLite's online backup into a temporary file beside
/// `dest`, then checks it and publishes it without replacing anything; `create` adds the
/// document row a template lacks.
pub(crate) fn copy(source: &Connection, dest: &Path, initial: Option<&[u8]>) -> Result<()> {
    let staged = Staged::beside(dest)?;
    let mut output = writer(staged.path(), true)?;
    {
        let backup = rusqlite::backup::Backup::new(source, &mut output).map_err(sqlite("Cannot copy the document"))?;
        match backup.step(-1).map_err(sqlite("Cannot copy the document"))? {
            rusqlite::backup::StepResult::Done => {}
            rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => return Err(Error::Busy),
            _ => return Err(failed("The copy did not complete")),
        }
    }
    configure_writer(&output)?;
    if let Some(checkpoint) = initial {
        let tx = output.unchecked_transaction().map_err(sqlite("Cannot create the document"))?;
        tx.execute("INSERT INTO document(id) VALUES(1)", []).map_err(sqlite("Cannot create the document"))?;
        tx.execute("INSERT INTO checkpoint VALUES(1,?)", [checkpoint]).map_err(sqlite("Cannot create the document"))?;
        tx.commit().map_err(sqlite("Cannot create the document"))?;
    }
    check(&output, false)?;
    output.close().map_err(|(_, e)| sqlite("close")(e))?;
    staged.publish_new(dest)
}
/// A new document from a template: the same app, with its initial state already durable.
pub fn create_document(template: &Path, dest: &Path) -> Result<()> {
    let source = reader(template)?;
    // The app is checked in the same read as the copy: a template the app would refuse
    // to open publishes nothing.
    let read = source.unchecked_transaction().map_err(sqlite("read"))?;
    let app = opened(&read, template, true)?;
    if app.kind != Kind::Template {
        return Err(invalid("Documents are created from a template"));
    }
    let doc = crate::Document::create_with(app.schema, &app.app.initial).map_err(Error::Rejected)?;
    let checkpoint = doc.checkpoint().map_err(Error::Rejected)?;
    if checkpoint.len() + 512 > crate::STORAGE_BYTES { return Err(Error::Full); }
    copy(&read, dest, Some(&checkpoint))
}
/// A summary for `slop inspect`: kind, markers, assets, artwork and the document's sizes,
/// of a file every open would accept.
pub fn inspect(path: &Path) -> Result<serde_json::Value> {
    let conn = reader(path)?;
    // The summary reads the state the checks passed.
    let conn = conn.unchecked_transaction().map_err(sqlite("read"))?;
    let opened = opened(&conn, path, true)?;
    let (updates, update_bytes, checkpoint_bytes): (i64, i64, i64) =
        conn.query_row(STATE_SIZES, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(sqlite("inspect"))?;
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
        "manifest": serde_json::from_str::<serde_json::Value>(&opened.app.manifest).map_err(failed)?,
        "assets": list("SELECT path, size FROM assets ORDER BY path")?,
        "artwork": list("SELECT name, length(png) FROM artwork ORDER BY name")?,
        "attachments": { "count": one("SELECT count(*) FROM attachments")?, "bytes": one("SELECT coalesce(sum(length(bytes)),0) FROM attachments")? },
        "state": { "checkpointBytes": checkpoint_bytes, "updates": updates, "updateBytes": update_bytes },
        "bytes": opened.bytes,
        "storedAssetBytes": one("SELECT coalesce(sum(length(bytes)),0) FROM assets")?,
    }))
}

/// The app's descriptor, for `slop schema`, from a file every open would accept.
pub fn descriptor(path: &Path) -> Result<String> {
    Ok(open(path, false)?.app.descriptor)
}
