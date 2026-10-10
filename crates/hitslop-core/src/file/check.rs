//! Marker-first acceptance of the stored SQLite layout and rows.
use super::assets::assets_within;
use super::{APPLICATION_ID, Kind, SCHEMA, STORAGE_VERSION, rows};
use crate::error::{Error, Result, failed, invalid, requires_update, sqlite};
use crate::wire::ASSET_PATH_BYTES;
use rusqlite::Connection;
use std::sync::OnceLock;

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
pub(super) fn contents(conn: &Connection, integrity: bool) -> Result<Kind> {
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
pub(super) fn one(conn: &Connection, sql: &str) -> Result<i64> {
    conn.query_row(sql, [], |r| r.get(0)).map_err(sqlite("read"))
}
/// The application ID, the storage version and the app's requirements: what tells a file
/// this build reads from one it is too old for.
pub(super) fn markers(conn: &Connection) -> Result<i64> {
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
pub(super) fn layout(conn: &Connection, version: i64) -> Result<()> {
    if tables(conn)? != expected_tables(version) {
        return Err(invalid("Unexpected document tables"));
    }
    // A file written without its CHECK constraints may hold other rows, which no read
    // would see.
    for table in ["app", "document", "share"] {
        let (rows, first): (i64, i64) = conn
            .query_row(&format!("SELECT count(*), coalesce(sum(id=1),0) FROM {table}"), [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(sqlite("read"))?;
        if rows > 1 || rows != first || (table == "app" && rows != 1) {
            return Err(invalid(format!("Unexpected rows in {table}")));
        }
    }
    // A lowercase hyphenated UUID: 36 ASCII bytes, hyphens at 8, 13, 18 and 23.
    if let Some(uuid) = rows::document_uuid(conn)?
        && (uuid.len() != 36
            || !uuid.bytes().enumerate().all(|(index, byte)| {
                if matches!(index, 8 | 13 | 18 | 23) { byte == b'-' } else { matches!(byte, b'0'..=b'9' | b'a'..=b'f') }
            }))
    {
        return Err(invalid("Invalid document identity"));
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
/// Whether the file is a template or a document. Both hold a history: a template's is one
/// snapshot, its initial state, which only a document adds updates and attachments to. A
/// document's attachments are within their limits and named by their hashes; the saved
/// state's own budget is the store's (`checked_bounds`).
fn state(conn: &Connection) -> Result<Kind> {
    let documents = one(conn, "SELECT count(*) FROM document")?;
    let history = one(conn, "SELECT count(*) FROM history")?;
    let shares = one(conn, "SELECT count(*) FROM share")?;
    let (attachments, attachment_largest, attachment_bytes) = rows::attachment_sizes(conn)?;
    if history == 0 {
        return Err(invalid("The file has no saved state; keep it for recovery"));
    }
    let kind = if documents == 0 {
        if history > 1 || attachments > 0 || shares > 0 {
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
