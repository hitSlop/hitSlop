//! A document is one SQLite file: the app its author built, the document's saved state, its
//! attachments and its artwork. This module owns that format: the tables, the checks every
//! open runs, packing a build into a template, creating and copying documents, and serving
//! the app's assets. `store` saves a document; `registry` holds its writer lock.

use crate::store::{failed, invalid, rejected, requires_update, sqlite, Error, Result};
use crate::{shape, Code};
use rusqlite::{config::DbConfig, limits::Limit, params, Connection, OpenFlags, OptionalExtension, MAIN_DB};
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub(crate) const APPLICATION_ID: i64 = 0x4853_4C50; // HSLP
/// These tables. A compatibility requirement, not a release number: a later version
/// migrates this one forward under the writer lock; a build refuses a newer one.
pub(crate) const STORAGE_VERSION: i64 = 1;
/// `app` is what the author built, written once by `pack` and identical in a template and
/// its documents. `document`, `checkpoint`, `updates` and `attachments` are the document;
/// a template has no rows in them. `doc_id` names the logical document: minted when a
/// document is created and renewed by a copy, so it never authorizes synchronization.
/// `checkpoint.schema_key` records the descriptor the saved state was written under.
pub(crate) const SCHEMA: &str = "\
CREATE TABLE app(id INTEGER PRIMARY KEY CHECK(id=1), package_format INTEGER NOT NULL, runtime_abi INTEGER NOT NULL, manifest TEXT NOT NULL, descriptor TEXT NOT NULL, initial TEXT NOT NULL, theme TEXT NOT NULL);
CREATE TABLE assets(path TEXT PRIMARY KEY, bytes BLOB NOT NULL);
CREATE TABLE artwork(name TEXT PRIMARY KEY CHECK(name IN ('preview','icon')), png BLOB NOT NULL);
CREATE TABLE document(id INTEGER PRIMARY KEY CHECK(id=1), doc_id TEXT NOT NULL, theme TEXT NOT NULL DEFAULT '{}');
CREATE TABLE checkpoint(id INTEGER PRIMARY KEY CHECK(id=1), schema_key TEXT NOT NULL, bytes BLOB NOT NULL);
CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);
CREATE TABLE attachments(id TEXT PRIMARY KEY, bytes BLOB NOT NULL);";

/// The longest manifest, descriptor or initial value, and the largest theme defaults.
const APP_TEXT_BYTES: i64 = 4 * 1024 * 1024;
const MANIFEST_BYTES: i64 = 64 * 1024;
const ASSET_PATH_BYTES: usize = 240;


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

/// The checks every open runs, in order, reading sizes with `length()` and never a value:
/// the application ID, the markers (a newer one is refused before anything else is read),
/// the exact tables, the rows a template or a document may hold, and every size budget.
/// `integrity` adds `PRAGMA quick_check` (42 ms for the largest allowed document).
pub(crate) fn check(conn: &Connection, integrity: bool) -> Result<Kind> {
    let one = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(sqlite("read"));
    if one("PRAGMA application_id")? != APPLICATION_ID {
        return Err(invalid("This is not a hitSlop document"));
    }
    let version = one("PRAGMA user_version")?;
    if version > STORAGE_VERSION {
        return Err(requires_update(format!("This document uses storage version {version}; this hitSlop reads version {STORAGE_VERSION}")));
    }
    if version != STORAGE_VERSION {
        return Err(invalid("Unsupported document storage"));
    }
    if tables(conn)? != expected_tables() {
        return Err(invalid("Unexpected document tables"));
    }
    if one("SELECT count(*) FROM app")? != 1 {
        return Err(invalid("A document needs its app"));
    }
    let (package_format, runtime_abi): (i64, i64) =
        conn.query_row("SELECT package_format, runtime_abi FROM app", [], |r| Ok((r.get(0)?, r.get(1)?))).map_err(sqlite("read"))?;
    for (name, level, supported) in [("runtime ABI", runtime_abi, crate::RUNTIME_ABI), ("package format", package_format, crate::PACKAGE_FORMAT)] {
        if level > supported as i64 {
            return Err(requires_update(format!("This slop needs {name} {level}; this hitSlop supports {supported}")));
        }
        if level < 1 {
            return Err(invalid(format!("Invalid {name}")));
        }
    }
    let (manifest, longest) = (
        one("SELECT length(CAST(manifest AS BLOB)) FROM app")?,
        one("SELECT max(length(CAST(descriptor AS BLOB)), length(CAST(initial AS BLOB)), length(CAST(theme AS BLOB))) FROM app")?,
    );
    if manifest > MANIFEST_BYTES || longest > APP_TEXT_BYTES {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    let (assets, largest, total, longest_path): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT count(*), coalesce(max(length(bytes)),0), coalesce(sum(length(bytes)),0), coalesce(max(length(CAST(path AS BLOB))),0) FROM assets",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(sqlite("read"))?;
    if assets > crate::ASSET_COUNT as i64 || largest > crate::ASSET_FILE_BYTES as i64 || total > crate::ASSET_BYTES as i64 {
        return Err(invalid("The app exceeds 256 assets, 25 MiB per asset or 50 MiB"));
    }
    if longest_path > ASSET_PATH_BYTES as i64 {
        return Err(invalid("An asset path is too long"));
    }
    let mut paths = conn.prepare("SELECT path FROM assets").map_err(sqlite("read"))?;
    for path in paths.query_map([], |r| r.get::<_, String>(0)).map_err(sqlite("read"))? {
        if !valid_asset_path(&path.map_err(sqlite("read"))?) {
            return Err(invalid("Unsafe asset path"));
        }
    }
    if one("SELECT coalesce(max(length(png)),0) FROM artwork")? > crate::ASSET_FILE_BYTES as i64 {
        return Err(invalid("Artwork is too large"));
    }
    let documents = one("SELECT count(*) FROM document")?;
    let checkpoints = one("SELECT count(*) FROM checkpoint")?;
    // The saved state's own budget is the store's (`checked_bounds`).
    let updates = one("SELECT count(*) FROM updates")?;
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
    if attachments > crate::ATTACHMENT_COUNT as i64
        || attachment_largest > crate::ATTACHMENT_FILE_BYTES as i64
        || attachment_bytes > crate::ATTACHMENT_BYTES as i64
    {
        return Err(invalid("Attachments exceed their limits"));
    }
    // Sizes first, as everywhere: then at most `ATTACHMENT_COUNT` IDs of 64 bytes are read.
    if one("SELECT count(*) FROM attachments WHERE length(CAST(id AS BLOB)) != 64")? > 0 {
        return Err(invalid("Invalid attachment identity"));
    }
    let mut ids = conn.prepare("SELECT CAST(id AS BLOB) FROM attachments").map_err(sqlite("read"))?;
    for id in ids.query_map([], |r| r.get::<_, Vec<u8>>(0)).map_err(sqlite("read"))? {
        let id = id.map_err(sqlite("read"))?;
        if !std::str::from_utf8(&id).is_ok_and(crate::store::valid_attachment_id) {
            return Err(invalid("Invalid attachment identity"));
        }
    }
    if integrity {
        let result: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(sqlite("check"))?;
        if result != "ok" {
            return Err(failed("The document file is damaged; keep it for recovery"));
        }
    }
    Ok(kind)
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
/// What checking an app found: its window shape, its declared colors in the order the
/// author wrote them, and the window skin's PNG when the manifest names one.
pub struct CheckedApp {
    pub silhouette: shape::Silhouette,
    pub theme_tokens: Vec<(String, String)>,
    pub skin: Option<Vec<u8>>,
}
/// The content rules `pack` applies and every open relies on, each run once.
fn check_app(app: &App, asset: &dyn Fn(&str) -> Result<Option<Vec<u8>>>) -> Result<CheckedApp> {
    let window = crate::manifest::validate(&app.manifest, app.package_format).map_err(Error::Rejected)?;
    crate::validate(&app.descriptor, &app.initial).map_err(Error::Rejected)?;
    let theme_tokens = crate::theme::validate_defaults(&app.theme).map_err(Error::Rejected)?;
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
    Ok(CheckedApp { silhouette: window.silhouette, theme_tokens, skin })
}

/// A template or document a host opened: its kind and markers, its app as stored, and what
/// checking the app found. Each open checks the app once and keeps this.
pub struct OpenedApp {
    pub kind: Kind,
    pub app: App,
    pub schema_key: String,
    pub silhouette: shape::Silhouette,
    pub theme_tokens: Vec<(String, String)>,
    /// The window skin's PNG, when the manifest names one.
    pub skin: Option<Vec<u8>>,
    /// The file's size in bytes when it was opened.
    pub bytes: u64,
}
/// Reads and checks the app of the file at `path`, which `check` accepted on `conn`.
pub(crate) fn open_app(conn: &Connection, kind: Kind, path: &Path) -> Result<OpenedApp> {
    let app = read_app(conn)?;
    let found = check_app(&app, &|key| read_asset(conn, key))?;
    Ok(OpenedApp {
        kind,
        schema_key: crate::schema_key(&app.descriptor).map_err(Error::Rejected)?,
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
    let conn = reader(path)?;
    let kind = check(&conn, integrity)?;
    open_app(&conn, kind, path)
}
/// A file's kind from its header checks alone, for a host deciding how to open it.
pub fn kind(path: &Path) -> Result<Kind> {
    check(&reader(path)?, false)
}
pub(crate) fn read_app(conn: &Connection) -> Result<App> {
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
fn read_asset(conn: &Connection, key: &str) -> Result<Option<Vec<u8>>> {
    conn.prepare_cached("SELECT bytes FROM assets WHERE path=?")
        .and_then(|mut s| s.query_row([key], |r| r.get(0)).optional())
        .map_err(sqlite("read asset"))
}

/// Serves the app's assets from one long-lived connection: whole, or a byte range read
/// without loading the rest.
pub struct AssetReader {
    conn: Connection,
}
impl AssetReader {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = reader(path)?;
        check(&conn, false)?;
        Ok(Self { conn })
    }
    fn row(&self, key: &str) -> Result<Option<(i64, u64)>> {
        self.conn
            .prepare_cached("SELECT rowid, length(bytes) FROM assets WHERE path=?")
            .and_then(|mut s| s.query_row([key], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64))).optional())
            .map_err(sqlite("read asset"))
    }
    /// The asset's size in bytes, when it exists.
    pub fn size(&self, key: &str) -> Result<Option<u64>> {
        Ok(self.row(key)?.map(|(_, size)| size))
    }
    /// `length` bytes from `offset`, clamped to the asset.
    pub fn read_range(&self, key: &str, offset: u64, length: u64) -> Result<Option<Vec<u8>>> {
        let Some((row, size)) = self.row(key)? else { return Ok(None) };
        let length = length.min(size.saturating_sub(offset)) as usize;
        let blob = self.conn.blob_open(MAIN_DB, "assets", "bytes", row, true).map_err(sqlite("read asset"))?;
        let mut buffer = vec![0u8; length];
        blob.read_at_exact(&mut buffer, offset as usize).map_err(sqlite("read asset"))?;
        Ok(Some(buffer))
    }
}

fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    getrandom::getrandom(&mut buffer).expect("random bytes");
    crate::hex(&buffer)
}
/// A new logical document's identity: 16 random bytes in lowercase hex.
pub(crate) fn new_doc_id() -> String {
    random_hex(16)
}
/// A private temporary file beside `dest`, removed if it is never published.
pub(crate) struct Staged(PathBuf);
impl Staged {
    pub(crate) fn beside(dest: &Path) -> Result<Self> {
        let dest = resolve(dest)?;
        let name = dest.file_name().unwrap().to_string_lossy();
        Ok(Self(dest.with_file_name(format!(".{name}.{}.tmp", random_hex(8)))))
    }
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
    /// Publishes the finished file without replacing anything at `dest`.
    pub(crate) fn publish_new(self, dest: &Path) -> Result<()> {
        let (from, to) = (cstring(&self.0)?, cstring(&resolve(dest)?)?);
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
        sync_folder(dest);
        std::mem::forget(self);
        Ok(())
    }
    /// Publishes over a template, never over a document.
    fn publish_template(self, dest: &Path) -> Result<()> {
        if let Ok(meta) = fs::symlink_metadata(dest) {
            if !meta.is_file() {
                return Err(rejected(Code::Exists, format!("Refusing to replace {}: it is not a template", dest.display())));
            }
            if check(&reader(dest)?, false)? != Kind::Template {
                return Err(rejected(Code::Exists, "Refusing to replace a document with a template"));
            }
            fs::rename(&self.0, resolve(dest)?).map_err(|e| failed(format!("Cannot save the template: {e}")))?;
            sync_folder(dest);
            std::mem::forget(self);
            return Ok(());
        }
        self.publish_new(dest)
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let _ = fs::remove_file(self.0.with_extension("tmp-journal"));
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
pub fn write_template(path: &Path, app: &App, assets: &[(String, Vec<u8>)], artwork: &[(String, Vec<u8>)]) -> Result<()> {
    let conn = writer(path, true)?;
    conn.execute_batch("PRAGMA auto_vacuum=INCREMENTAL;").map_err(sqlite("create"))?;
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
        tx.execute("INSERT INTO assets VALUES(?,?)", params![key, bytes]).map_err(sqlite("create"))?;
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
    if size > (MANIFEST_BYTES + 3 * APP_TEXT_BYTES) as u64 {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    let text = fs::read_to_string(&file).map_err(|e| invalid(format!("app.json: {e}")))?;
    let row: crate::wire::AppRow = serde_json::from_str(&text).map_err(|e| invalid(format!("app.json: {e}")))?;
    if row.packageFormat == 0 || row.runtimeABI == 0 {
        return Err(invalid("app.json needs packageFormat and runtimeABI of at least 1"));
    }
    if row.packageFormat > crate::PACKAGE_FORMAT || row.runtimeABI > crate::RUNTIME_ABI {
        return Err(requires_update("This build is newer than this hitSlop engine"));
    }
    let app = App {
        package_format: row.packageFormat,
        runtime_abi: row.runtimeABI,
        manifest: row.manifest.get().to_owned(),
        descriptor: row.descriptor.get().to_owned(),
        initial: row.initial.get().to_owned(),
        theme: row.theme.get().to_owned(),
    };
    let assets = stage_assets(&stage.join("assets"))?;
    let mut artwork = vec![];
    for name in ["preview", "icon"] {
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
/// The stage's assets: regular files only, within the budgets.
fn stage_assets(root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut assets = vec![];
    let (mut count, mut total) = (0usize, 0usize);
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
            count += 1;
            total += meta.len() as usize;
            if !valid_asset_path(&key) {
                return Err(invalid(format!("Unsafe asset path {key}")));
            }
            if meta.len() as usize > crate::ASSET_FILE_BYTES || count > crate::ASSET_COUNT || total > crate::ASSET_BYTES {
                return Err(invalid("The app exceeds 256 assets, 25 MiB per asset or 50 MiB"));
            }
            assets.push((key, fs::read(&path).map_err(failed)?));
        }
    }
    assets.sort();
    Ok(assets)
}

/// Copies `source` through SQLite's online backup into a temporary file beside `dest`,
/// runs `finish` there, checks the result and publishes it without replacing anything.
/// Copies `source` to `dest` as a new logical document: `create` adds the document row a
/// template lacks; otherwise the copy's row gets a new identity.
pub(crate) fn copy(source: &Connection, dest: &Path, create: bool) -> Result<()> {
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
    let identity = if create { "INSERT INTO document(id,doc_id) VALUES(1,?1)" } else { "UPDATE document SET doc_id=?1 WHERE id=1" };
    output.execute(identity, [new_doc_id()]).map_err(sqlite("Cannot give the copy its identity"))?;
    check(&output, false)?;
    output.close().map_err(|(_, e)| sqlite("close")(e))?;
    staged.publish_new(dest)
}
/// A new document from a template: the same app, a fresh identity, no saved state yet.
pub fn create_document(template: &Path, dest: &Path) -> Result<()> {
    let source = reader(template)?;
    let read = source.unchecked_transaction().map_err(sqlite("read"))?;
    let kind = check(&read, true)?;
    if kind != Kind::Template {
        return Err(invalid("Documents are created from a template"));
    }
    // The app is checked in the same read as the copy: a template the app would refuse
    // to open publishes nothing.
    open_app(&read, kind, template)?;
    copy(&read, dest, true)
}
/// A closed document's copy as a new logical document: the same app, history,
/// attachments and theme, a new identity. An open document is copied by its owner
/// (`Store::copy_to`), so saves wait behind the copy instead of timing out.
pub fn duplicate(source: &Path, dest: &Path) -> Result<()> {
    let source = reader(source)?;
    // One read transaction: the backup copies exactly what was checked.
    let read = source.unchecked_transaction().map_err(sqlite("read"))?;
    if check(&read, true)? != Kind::Document {
        return Err(invalid("Only a document can be duplicated; create one from a template"));
    }
    crate::store::checked_bounds(&read, 0, 0)?;
    copy(&read, dest, false)
}

/// A summary for `slop inspect`: kind, markers, assets, artwork and the document's sizes.
pub fn inspect(path: &Path) -> Result<serde_json::Value> {
    let conn = reader(path)?;
    let kind = check(&conn, true)?;
    let app = read_app(&conn)?;
    let list = |sql: &str| -> Result<Vec<serde_json::Value>> {
        let mut statement = conn.prepare(sql).map_err(sqlite("inspect"))?;
        let rows = statement
            .query_map([], |r| Ok(serde_json::json!({ "name": r.get::<_, String>(0)?, "bytes": r.get::<_, i64>(1)? })))
            .map_err(sqlite("inspect"))?;
        rows.collect::<rusqlite::Result<_>>().map_err(sqlite("inspect"))
    };
    let one = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(sqlite("inspect"));
    Ok(serde_json::json!({
        "kind": match kind { Kind::Template => "template", Kind::Document => "document" },
        "packageFormat": app.package_format,
        "runtimeABI": app.runtime_abi,
        "manifest": serde_json::from_str::<serde_json::Value>(&app.manifest).map_err(failed)?,
        "assets": list("SELECT path, length(bytes) FROM assets ORDER BY path")?,
        "artwork": list("SELECT name, length(png) FROM artwork ORDER BY name")?,
        "attachments": { "count": one("SELECT count(*) FROM attachments")?, "bytes": one("SELECT coalesce(sum(length(bytes)),0) FROM attachments")? },
        "state": {
            "checkpointBytes": one("SELECT coalesce(sum(length(bytes)),0) FROM checkpoint")?,
            "updates": one("SELECT count(*) FROM updates")?,
            "updateBytes": one("SELECT coalesce(sum(length(bytes)),0) FROM updates")?,
        },
        "bytes": fs::metadata(resolve(path)?).map(|m| m.len()).unwrap_or(0),
    }))
}

/// The app's descriptor, for `slop schema`.
pub fn descriptor(path: &Path) -> Result<String> {
    let conn = reader(path)?;
    check(&conn, false)?;
    Ok(read_app(&conn)?.descriptor)
}
