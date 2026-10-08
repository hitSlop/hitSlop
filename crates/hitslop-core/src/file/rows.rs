//! The rows a template or document holds beside its app, and every write to them: the saved
//! state (one checkpoint and the updates saved after it), attachments, artwork, and what
//! `pack` writes. The store saves and loads through these; the checks every open runs,
//! `inspect` and the asset reader read the tables directly.

use super::Artwork;
use super::assets::Encoding;
use crate::app::AppDefinition;
use crate::error::{Result, sqlite};
use rusqlite::{Connection, OptionalExtension, params};

/// The saved checkpoint, and the updates saved since, in order: streamed straight from
/// SQLite's buffers by the store's load.
pub(crate) const CHECKPOINT: &str = "SELECT bytes FROM checkpoint WHERE id=1";
pub(crate) const UPDATES: &str = "SELECT bytes FROM updates ORDER BY seq";

/// The saved state's sizes: update rows, update bytes and checkpoint bytes.
pub(crate) fn state_sizes(conn: &Connection) -> Result<(i64, i64, i64)> {
    conn.prepare_cached(
        "SELECT (SELECT count(*) FROM updates),(SELECT coalesce(sum(length(bytes)),0) FROM updates),(SELECT coalesce(sum(length(bytes)),0) FROM checkpoint)",
    )
    .and_then(|mut s| s.query_row([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))
    .map_err(sqlite("read state sizes"))
}
/// Writes the one checkpoint, in place of any before it.
pub(crate) fn put_checkpoint(conn: &Connection, bytes: &[u8]) -> Result<()> {
    conn.prepare_cached(
        "INSERT INTO checkpoint(id, bytes) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET bytes=excluded.bytes",
    )
    .and_then(|mut s| s.execute([bytes]))
    .map(|_| ())
    .map_err(sqlite("write checkpoint"))
}
/// Appends one saved update after the others.
pub(crate) fn append_update(conn: &Connection, bytes: &[u8]) -> Result<()> {
    conn.prepare_cached("INSERT INTO updates(bytes) VALUES(?)")
        .and_then(|mut s| s.execute([bytes]))
        .map(|_| ())
        .map_err(sqlite("append update"))
}
/// Deletes every saved update: a new checkpoint covers them.
pub(crate) fn clear_updates(conn: &Connection) -> Result<()> {
    conn.execute_batch("DELETE FROM updates").map_err(sqlite("clear updates"))
}
/// The document row that makes a copy of a template a document.
pub(crate) fn add_document(conn: &Connection) -> Result<()> {
    conn.execute("INSERT INTO document(id) VALUES(1)", []).map(|_| ()).map_err(sqlite("create the document"))
}

/// The stored attachments' count, largest size and total size.
pub(crate) fn attachment_sizes(conn: &Connection) -> Result<(i64, i64, i64)> {
    conn.prepare_cached(
        "SELECT count(*), coalesce(max(length(bytes)),0), coalesce(sum(length(bytes)),0) FROM attachments",
    )
    .and_then(|mut s| s.query_row([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))
    .map_err(sqlite("read attachment sizes"))
}
/// Every stored attachment's identity and size, by identity.
pub(crate) fn attachment_list(conn: &Connection) -> Result<Vec<(String, u64, String)>> {
    let mut statement = conn
        .prepare("SELECT id, length(bytes), media_type FROM attachments ORDER BY id")
        .map_err(sqlite("list attachments"))?;
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64, r.get(2)?)))
        .map_err(sqlite("list attachments"))?;
    rows.collect::<rusqlite::Result<_>>().map_err(sqlite("list attachments"))
}
/// One attachment's stored bytes, when it is stored.
pub(crate) fn read_attachment(conn: &Connection, id: &str) -> Result<Option<Vec<u8>>> {
    conn.prepare_cached("SELECT bytes FROM attachments WHERE id=?")
        .and_then(|mut s| s.query_row([id], |r| r.get(0)).optional())
        .map_err(sqlite("read attachment"))
}
pub(crate) fn put_attachment(conn: &Connection, id: &str, bytes: &[u8]) -> Result<()> {
    conn.execute(
        "INSERT INTO attachments(id, media_type, bytes) VALUES(?,?,?)",
        params![id, crate::media::attachment_type(bytes), bytes],
    )
    .map(|_| ())
    .map_err(sqlite("store attachment"))
}
/// Deletes one attachment; returns how many rows went (0 or 1).
pub(crate) fn delete_attachment(conn: &Connection, id: &str) -> Result<usize> {
    conn.execute("DELETE FROM attachments WHERE id=?", [id]).map_err(sqlite("delete attachment"))
}

/// One artwork image hosts can show, when the file holds one. Artwork is replaceable and
/// cosmetic, and the next capture rewrites it: an oversized image or one that is not a PNG
/// reads as absent, never as a damaged document. Writes check it fully.
pub(crate) fn read_artwork(conn: &Connection, name: Artwork) -> Result<Option<Vec<u8>>> {
    let length: Option<i64> = conn
        .prepare_cached("SELECT length(png) FROM artwork WHERE name=?")
        .and_then(|mut s| s.query_row([name], |r| r.get(0)).optional())
        .map_err(sqlite("read artwork"))?;
    if length.is_none_or(|n| n as usize > crate::ASSET_FILE_BYTES) {
        return Ok(None);
    }
    let png: Option<Vec<u8>> = conn
        .prepare_cached("SELECT png FROM artwork WHERE name=?")
        .and_then(|mut s| s.query_row([name], |r| r.get(0)).optional())
        .map_err(sqlite("read artwork"))?;
    Ok(png.filter(|png| crate::images::header(png).is_ok()))
}
/// Writes one artwork image, in place of any by that name.
pub(crate) fn put_artwork(conn: &Connection, name: Artwork, png: &[u8]) -> Result<()> {
    conn.execute(
        "INSERT INTO artwork(name, png) VALUES(?,?) ON CONFLICT(name) DO UPDATE SET png=excluded.png",
        params![name, png],
    )
    .map(|_| ())
    .map_err(sqlite("write artwork"))
}
/// Deletes every artwork image.
pub(crate) fn clear_artwork(conn: &Connection) -> Result<()> {
    conn.execute_batch("DELETE FROM artwork").map_err(sqlite("clear artwork"))
}

/// The `app` row `pack` writes once.
pub(super) fn put_app(conn: &Connection, app: &AppDefinition, package_format: u64, runtime_abi: u64) -> Result<()> {
    use crate::app::package_format_1::category_column as column;
    let m = app.metadata();
    conn.execute(
        "INSERT INTO app(id,package_format,runtime_abi,slug,title,description,author_name,author_url,category_primary,category_secondary,definition_json) VALUES(1,?,?,?,?,?,?,?,?,?,?)",
        params![package_format as i64, runtime_abi as i64, m.slug, m.title, m.description, m.author.name, m.author.url,
            column(m.categories[0]), m.categories.get(1).map(|c| column(*c)), app.definition_json()],
    ).map(|_| ()).map_err(sqlite("write app"))
}
/// One asset, before the app row seals the inventory.
pub(super) fn put_asset(
    conn: &Connection,
    key: &str,
    media_type: &str,
    encoding: Encoding,
    size: usize,
    stored: &[u8],
) -> Result<()> {
    conn.execute(
        "INSERT INTO assets(key,media_type,encoding,size,bytes) VALUES(?,?,?,?,?)",
        params![key, media_type, encoding, size as i64, stored],
    )
    .map(|_| ())
    .map_err(sqlite("write asset"))
}
