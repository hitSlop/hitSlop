//! The app's assets: their paths, how `pack` stores them and how a page reads them back.

use crate::error::{Result, failed, invalid, sqlite};
use crate::wire::ASSET_PATH_BYTES;
use rusqlite::{Connection, MAIN_DB, OptionalExtension};
use std::borrow::Cow;
use std::io::Read;

/// The asset budget, as packing reads a stage and as every open finds it stored.
pub(super) fn assets_within(count: usize, largest: usize, total: usize) -> Result<()> {
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
/// How an asset's `bytes` hold it, as the `assets` table's CHECK names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Encoding {
    Identity,
    Brotli,
}
impl rusqlite::ToSql for Encoding {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(match self {
            Self::Identity => "identity",
            Self::Brotli => "br",
        }
        .into())
    }
}
impl rusqlite::types::FromSql for Encoding {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_str()? {
            "identity" => Ok(Self::Identity),
            "br" => Ok(Self::Brotli),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}
/// An asset's bytes, decoded.
pub(super) fn read_asset(conn: &Connection, key: &str) -> Result<Option<Vec<u8>>> {
    let row: Option<(Encoding, i64, Vec<u8>)> = conn
        .prepare_cached("SELECT encoding,size,bytes FROM assets WHERE path=?")
        .and_then(|mut s| s.query_row([key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional())
        .map_err(sqlite("read asset"))?;
    row.map(|(encoding, size, bytes)| decode(encoding, size as usize, bytes)).transpose()
}
/// How `pack` stores an asset: text and WebAssembly Brotli-compressed when that is smaller,
/// anything else as it is, so media ranges read straight from the file. Quality 10 stores
/// within a page or so of 11 in half the time; past 4 MiB it takes seconds, so larger
/// assets use 9.
pub(super) fn encode<'a>(key: &str, bytes: &'a [u8]) -> Result<(Encoding, Cow<'a, [u8]>)> {
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
            return Ok((Encoding::Brotli, Cow::Owned(compressed)));
        }
    }
    Ok((Encoding::Identity, Cow::Borrowed(bytes)))
}
/// An asset's stored bytes, decoded to the `size` `check` bounded.
fn decode(encoding: Encoding, size: usize, stored: Vec<u8>) -> Result<Vec<u8>> {
    match encoding {
        Encoding::Identity => Ok(stored),
        Encoding::Brotli => {
            let mut bytes = Vec::with_capacity(size);
            let read = brotli::Decompressor::new(stored.as_slice(), 4096).take(size as u64 + 1).read_to_end(&mut bytes);
            if read.is_ok() && bytes.len() == size {
                Ok(bytes)
            } else {
                Err(failed("An app asset is damaged; keep the file for recovery"))
            }
        }
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
    fn row(&self, key: &str) -> Result<Option<(i64, Encoding, u64)>> {
        self.conn
            .prepare_cached("SELECT rowid, encoding, size FROM assets WHERE path=?")
            .and_then(|mut s| s.query_row([key], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64))).optional())
            .map_err(sqlite("read asset"))
    }
    /// The asset's size in bytes, when it exists.
    pub fn size(&self, key: &str) -> Result<Option<u64>> {
        Ok(self.row(key)?.map(|(_, _, size)| size))
    }
    /// `length` bytes from `offset`, clamped to the asset.
    pub fn read_range(&self, key: &str, offset: u64, length: u64) -> Result<Option<Vec<u8>>> {
        let Some((row, encoding, size)) = self.row(key)? else { return Ok(None) };
        let length = length.min(size.saturating_sub(offset)) as usize;
        if encoding != Encoding::Identity {
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
