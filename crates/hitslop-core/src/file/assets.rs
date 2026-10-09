//! The app's assets: their paths, how `pack` stores them and how a page reads them back.

use crate::error::{Result, failed, invalid, sqlite};
use crate::wire::ASSET_PATH_BYTES;
use rusqlite::{Connection, MAIN_DB, OptionalExtension};
#[cfg(not(target_arch = "wasm32"))]
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
    if extension == "html" { "text/html; charset=utf-8" } else { crate::media::asset_type(&extension).media_type }
}
/// What every app resource is, at pack and open: a program or stylesheet with content, or
/// media named by its SHA-256. `hashed` verifies the address; opening checks it only for
/// integrity, since the app row seals what `pack` verified.
pub(super) fn check_resource(key: &str, bytes: &[u8], hashed: bool) -> Result<()> {
    use sha2::{Digest, Sha256};
    match key.strip_prefix("media/") {
        Some(name) if hashed && !name.starts_with(&data_encoding::HEXLOWER.encode(&Sha256::digest(bytes))) => {
            Err(invalid(format!("Resource {key} does not match its SHA-256")))
        }
        None if bytes.is_empty() => Err(invalid(format!("Resource {key} is empty"))),
        _ => Ok(()),
    }
}
/// How an asset's `bytes` hold it. Every open refuses another encoding (`stored_assets`).
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
        .prepare_cached("SELECT encoding,size,bytes FROM assets WHERE key=?")
        .and_then(|mut s| s.query_row([key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional())
        .map_err(sqlite("read asset"))?;
    row.map(|(encoding, size, bytes)| decode(encoding, size as usize, bytes)).transpose()
}
/// How `pack` stores an asset: text and WebAssembly Brotli-compressed when that is smaller,
/// anything else as it is, so media ranges read straight from the file. Quality 10 stores
/// within a page or so of 11 in half the time; past 4 MiB it takes seconds, so larger
/// assets use 9.
#[cfg(not(target_arch = "wasm32"))]
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

/// URL namespaces are explicit: the page cannot fetch command programs or artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceRoute {
    App,
    Attachment,
}
#[derive(Clone, Debug)]
pub struct ResourceInfo {
    pub size: u64,
    pub media_type: String,
}
struct ResourceRow {
    id: i64,
    encoding: Encoding,
    info: ResourceInfo,
}

/// One query-only connection, with first-touch attachment verification and decoded text
/// caching. Identity media ranges use SQLite BLOB reads without loading the whole asset.
#[derive(Default)]
pub(crate) struct ResourceCache {
    verified: std::collections::HashSet<String>,
    decoded: std::collections::VecDeque<(String, Vec<u8>)>,
    bytes: usize,
}
pub struct ResourceReader<C = Connection> {
    conn: C,
    cache: std::sync::Arc<std::sync::Mutex<ResourceCache>>,
}
impl<C: std::borrow::Borrow<Connection>> ResourceReader<C> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn new(conn: C) -> Self {
        Self::with_cache(conn, Default::default())
    }
    pub(crate) fn with_cache(conn: C, cache: std::sync::Arc<std::sync::Mutex<ResourceCache>>) -> Self {
        Self { conn, cache }
    }
    fn row(&self, route: ResourceRoute, key: &str) -> Result<Option<ResourceRow>> {
        let row = match route {
            ResourceRoute::App => {
                if key == "commands.js" || crate::media::asset_key(key).is_none() {
                    return Ok(None);
                }
                self.conn
                    .borrow()
                    .prepare_cached("SELECT rowid,encoding,size,media_type FROM assets WHERE key=?")
                    .and_then(|mut s| {
                        s.query_row([key], |r| {
                            Ok(ResourceRow {
                                id: r.get(0)?,
                                encoding: r.get(1)?,
                                info: ResourceInfo { size: r.get::<_, i64>(2)? as u64, media_type: r.get(3)? },
                            })
                        })
                        .optional()
                    })
                    .map_err(sqlite("read asset"))?
            }
            ResourceRoute::Attachment => {
                if !crate::wire::valid_attachment_id(key) {
                    return Ok(None);
                }
                self.conn
                    .borrow()
                    .prepare_cached("SELECT rowid,length(bytes),media_type FROM attachments WHERE id=?")
                    .and_then(|mut s| {
                        s.query_row([key], |r| {
                            Ok(ResourceRow {
                                id: r.get(0)?,
                                encoding: Encoding::Identity,
                                info: ResourceInfo { size: r.get::<_, i64>(1)? as u64, media_type: r.get(2)? },
                            })
                        })
                        .optional()
                    })
                    .map_err(sqlite("read attachment"))?
            }
        };
        let Some(row) = row else { return Ok(None) };
        if row.info.size
            > if route == ResourceRoute::App {
                crate::ASSET_FILE_BYTES as u64
            } else {
                crate::ATTACHMENT_FILE_BYTES as u64
            }
        {
            return Err(invalid("Resource exceeds its byte limit"));
        }
        if route == ResourceRoute::Attachment && !crate::lock(&self.cache).verified.contains(key) {
            use sha2::{Digest, Sha256};
            use std::io::{Read, Seek, SeekFrom};
            let mut blob = self
                .conn
                .borrow()
                .blob_open(MAIN_DB, "attachments", "bytes", row.id, true)
                .map_err(sqlite("verify attachment"))?;
            let media = crate::media::attachment_type_stream(&mut blob, row.info.size as usize).map_err(failed)?;
            blob.seek(SeekFrom::Start(0)).map_err(failed)?;
            let mut hash = Sha256::new();
            let mut chunk = vec![0; crate::wire::browser::TRANSFER_BYTES];
            loop {
                let count = blob.read(&mut chunk).map_err(failed)?;
                if count == 0 {
                    break;
                }
                hash.update(&chunk[..count]);
            }
            if data_encoding::HEXLOWER.encode(&hash.finalize()) != key || media != row.info.media_type {
                return Err(failed("An attachment is damaged; keep the file for recovery"));
            }
            crate::lock(&self.cache).verified.insert(key.into());
        }
        if route == ResourceRoute::App
            && crate::media::asset_key(key).is_none_or(|kind| kind.media_type != row.info.media_type)
        {
            return Err(invalid("Invalid resource media type"));
        }
        Ok(Some(row))
    }
    pub fn info(&self, route: ResourceRoute, key: &str) -> Result<Option<ResourceInfo>> {
        Ok(self.row(route, key)?.map(|row| row.info))
    }
    pub fn read_range(&self, route: ResourceRoute, key: &str, offset: u64, length: u64) -> Result<Option<Vec<u8>>> {
        let Some(row) = self.row(route, key)? else { return Ok(None) };
        let start = offset.min(row.info.size) as usize;
        let length = length.min(row.info.size.saturating_sub(offset)) as usize;
        if length == 0 {
            return Ok(Some(Vec::new()));
        }
        if row.encoding != Encoding::Identity {
            const CACHE_BYTES: usize = 32 << 20;
            let mut cache = crate::lock(&self.cache);
            let entry = if let Some(index) = cache.decoded.iter().position(|(name, _)| name == key) {
                cache.decoded.remove(index).expect("cached entry")
            } else {
                let bytes = read_asset(self.conn.borrow(), key)?.ok_or_else(|| invalid("Missing resource"))?;
                while cache.bytes + bytes.len() > CACHE_BYTES {
                    let Some((_, old)) = cache.decoded.pop_front() else { break };
                    cache.bytes -= old.len();
                }
                cache.bytes += bytes.len();
                (key.into(), bytes)
            };
            let result = entry.1[start..start + length].to_vec();
            cache.decoded.push_back(entry);
            return Ok(Some(result));
        }
        let table = match route {
            ResourceRoute::App => "assets",
            ResourceRoute::Attachment => "attachments",
        };
        let blob =
            self.conn.borrow().blob_open(MAIN_DB, table, "bytes", row.id, true).map_err(sqlite("read resource"))?;
        let mut bytes = vec![0; length];
        blob.read_at_exact(&mut bytes, start).map_err(sqlite("read resource"))?;
        Ok(Some(bytes))
    }
}
