//! Media names, packing policy and passive attachment signatures. Filenames select
//! app formats; attachment types depend only on their bytes, never a caller's claim.
use crate::{Code, Result, err};

#[derive(Clone, Copy, Debug)]
pub struct AssetType {
    pub extensions: &'static [&'static str],
    pub media_type: &'static str,
    pub canonical_extension: &'static str,
    pub compress: bool,
}

macro_rules! formats {
    ($( [$($extension:literal),+] => ($media:literal, $compress:literal) ),+ $(,)?) => {
        pub const ASSET_TYPES: &[AssetType] = &[
            $(AssetType { extensions: &[$($extension),+], media_type: $media,
                canonical_extension: formats!(@first $($extension),+), compress: $compress }),+
        ];
    };
    (@first $first:literal $(,$rest:literal)*) => { $first };
}
formats! {
    ["js", "mjs"] => ("text/javascript", true),
    ["css"] => ("text/css", true),
    ["json"] => ("application/json", true),
    ["txt"] => ("text/plain", true),
    ["svg"] => ("image/svg+xml", true),
    ["wasm"] => ("application/wasm", true),
    ["png"] => ("image/png", false),
    ["jpg", "jpeg"] => ("image/jpeg", false),
    ["webp"] => ("image/webp", false),
    ["gif"] => ("image/gif", false),
    ["avif"] => ("image/avif", false),
    ["heic", "heif"] => ("image/heic", false),
    ["woff"] => ("font/woff", false),
    ["woff2"] => ("font/woff2", false),
    ["ttf"] => ("font/ttf", false),
    ["otf"] => ("font/otf", false),
    ["mp3"] => ("audio/mpeg", false),
    ["m4a"] => ("audio/mp4", false),
    ["wav"] => ("audio/wav", false),
    ["ogg"] => ("audio/ogg", false),
    ["flac"] => ("audio/flac", false),
    ["mp4"] => ("video/mp4", false),
    ["mov"] => ("video/quicktime", false),
    ["webm"] => ("video/webm", false),
    ["pdf"] => ("application/pdf", false),
    ["bin"] => ("application/octet-stream", false),
}

pub fn asset_type(extension: &str) -> &'static AssetType {
    ASSET_TYPES
        .iter()
        .find(|kind| kind.extensions.iter().any(|ext| extension.eq_ignore_ascii_case(ext)))
        .unwrap_or_else(|| ASSET_TYPES.last().expect("binary fallback"))
}

/// Fixed entry names or a content-addressed resource with a canonical extension.
pub fn asset_key(key: &str) -> Option<&'static AssetType> {
    match key {
        "ui.js" | "commands.js" => Some(asset_type("js")),
        "ui.css" => Some(asset_type("css")),
        _ => {
            let (hash, extension) = key.strip_prefix("media/")?.split_once('.')?;
            if !crate::wire::valid_attachment_id(hash) {
                return None;
            }
            ASSET_TYPES.iter().find(|kind| kind.canonical_extension == extension)
        }
    }
}

/// Recognizes passive signatures, not complete valid files. Decoders still validate
/// the media they consume; skins/artwork additionally require a complete PNG decode.
pub fn attachment_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "image/png";
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return "image/jpeg";
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "image/gif";
    }
    if bytes.starts_with(b"RIFF") {
        match bytes.get(8..12) {
            Some(b"WEBP") => return "image/webp",
            Some(b"WAVE") => return "audio/wav",
            _ => {}
        }
    }
    if let Some(kind) = iso_media(bytes) {
        return kind;
    }
    if bytes.starts_with(b"ID3")
        || bytes.get(..3).is_some_and(|head| {
            head[0] == 0xff
                && head[1] & 0xe0 == 0xe0
                && head[1] & 0x18 != 0x08
                && head[1] & 0x06 != 0
                && head[2] & 0xf0 != 0xf0
                && head[2] & 0x0c != 0x0c
        })
    {
        return "audio/mpeg";
    }
    if bytes.starts_with(b"OggS") {
        return "audio/ogg";
    }
    if bytes.starts_with(b"fLaC") {
        return "audio/flac";
    }
    if bytes.starts_with(b"%PDF-") {
        return "application/pdf";
    }
    if webm(bytes) {
        return "video/webm";
    }
    "application/octet-stream"
}

/// Same passive sniffing as `attachment_type`, without buffering a large attachment.
/// ISO brands may occupy a large ftyp box; examine those in bounded blocks too.
#[cfg(feature = "storage")]
pub(crate) fn attachment_type_stream<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    size: usize,
) -> std::io::Result<&'static str> {
    use std::io::SeekFrom;
    reader.seek(SeekFrom::Start(0))?;
    let mut prefix = vec![0; size.min(8192)];
    reader.read_exact(&mut prefix)?;
    let initial = attachment_type(&prefix);
    if prefix.get(4..8) != Some(b"ftyp") || prefix.len() < 16 {
        return Ok(initial);
    }
    let length = u32::from_be_bytes(prefix[..4].try_into().expect("header")) as usize;
    if length <= prefix.len() || length > size || length < 16 || !(length - 16).is_multiple_of(4) {
        return Ok(initial);
    }
    // Preserve the signature precedence before ISO media.
    if prefix.starts_with(b"\x89PNG\r\n\x1a\n")
        || prefix.starts_with(b"\xff\xd8\xff")
        || prefix.starts_with(b"GIF87a")
        || prefix.starts_with(b"GIF89a")
        || prefix.starts_with(b"RIFF")
    {
        return Ok(initial);
    }
    let rank = |kind| match kind {
        "image/avif" => 0,
        "image/heic" => 1,
        "audio/mp4" => 2,
        "video/quicktime" => 3,
        "video/mp4" => 4,
        _ => 5,
    };
    let mut probe = [0u8; 20];
    probe[..16].copy_from_slice(&prefix[..16]);
    probe[..4].copy_from_slice(&20u32.to_be_bytes());
    let mut result = initial;
    reader.seek(SeekFrom::Start(16))?;
    let mut chunk = [0u8; 8192];
    let mut remaining = length - 16;
    while remaining > 0 {
        let n = remaining.min(chunk.len());
        reader.read_exact(&mut chunk[..n])?;
        for brand in chunk[..n].chunks_exact(4) {
            probe[16..].copy_from_slice(brand);
            if let Some(kind) = iso_media(&probe)
                && rank(kind) < rank(result)
            {
                result = kind;
            }
        }
        remaining -= n;
    }
    Ok(result)
}

fn iso_media(bytes: &[u8]) -> Option<&'static str> {
    if bytes.get(4..8)? != b"ftyp" {
        return None;
    }
    let length = u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    if length < 16 || length > bytes.len() || !(length - 16).is_multiple_of(4) {
        return None;
    }
    let major = bytes.get(8..12)?;
    let has = |names: &[&[u8]]| {
        names.contains(&major) || bytes[16..length].chunks_exact(4).any(|brand| names.contains(&brand))
    };
    Some(if has(&[b"avif", b"avis"]) {
        "image/avif"
    } else if has(&[b"heic", b"heix", b"mif1", b"msf1"]) {
        "image/heic"
    } else if major == b"M4A " {
        "audio/mp4"
    } else if major == b"qt  " {
        "video/quicktime"
    } else if has(&[b"isom", b"iso2", b"iso3", b"iso4", b"iso5", b"iso6", b"mp41", b"mp42", b"avc1", b"dash", b"M4V "])
    {
        "video/mp4"
    } else {
        return None;
    })
}

/// An EBML header with a WebM DocType. Parse the header's top-level elements so an
/// arbitrary payload containing the word `webm` cannot select a content type.
fn webm(bytes: &[u8]) -> bool {
    fn vint(bytes: &[u8], keep_marker: bool) -> Option<(u64, usize)> {
        let first = *bytes.first()?;
        let count = first.leading_zeros() as usize + 1;
        if count > 8 || bytes.len() < count {
            return None;
        }
        let mut value = if keep_marker { first as u64 } else { (first & (0xff_u16 >> count) as u8) as u64 };
        for byte in &bytes[1..count] {
            value = (value << 8) | *byte as u64;
        }
        Some((value, count))
    }
    if !bytes.starts_with(b"\x1a\x45\xdf\xa3") {
        return false;
    }
    let Some((length, size)) = vint(&bytes[4..], false) else {
        return false;
    };
    let start = 4 + size;
    // Real headers are small; refuse oversized/unknown-length headers for sniffing.
    if length > 4096 || length as usize > bytes.len().saturating_sub(start) {
        return false;
    }
    let mut header = &bytes[start..start + length as usize];
    while !header.is_empty() {
        let Some((id, id_size)) = vint(header, true) else {
            return false;
        };
        let Some((length, size)) = vint(&header[id_size..], false) else {
            return false;
        };
        let start = id_size + size;
        if length > header.len().saturating_sub(start) as u64 {
            return false;
        }
        let end = start + length as usize;
        if id == 0x4282 {
            return &header[start..end] == b"webm";
        }
        header = &header[end..];
    }
    false
}

/// App assets may include executable/text formats. Binary formats must match their
/// declared signatures; passive attachments never use this author-selected type.
pub fn check_asset(media_type: &str, bytes: &[u8]) -> Result<()> {
    let kind = ASSET_TYPES
        .iter()
        .find(|kind| kind.media_type == media_type)
        .ok_or_else(|| err(Code::InvalidRequest, "Unknown app media type"))?;
    let valid = match kind.canonical_extension {
        "bin" => true,
        "js" | "css" | "json" | "txt" | "svg" => std::str::from_utf8(bytes).is_ok(),
        "wasm" => bytes.starts_with(b"\0asm\x01\0\0\0"),
        "woff" => bytes.starts_with(b"wOFF"),
        "woff2" => bytes.starts_with(b"wOF2"),
        "ttf" => bytes.starts_with(b"\0\x01\0\0") || bytes.starts_with(b"true"),
        "otf" => bytes.starts_with(b"OTTO"),
        _ => attachment_type(bytes) == media_type,
    };
    if valid { Ok(()) } else { Err(err(Code::InvalidRequest, "App asset bytes do not match their media type")) }
}

/// The TypeScript bundler uses this projection; it owns no second MIME registry.
pub fn build_registry() -> serde_json::Value {
    ASSET_TYPES
        .iter()
        .flat_map(|kind| {
            kind.extensions.iter().map(move |extension| {
                (extension.to_string(), crate::json!([kind.media_type, kind.canonical_extension]))
            })
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}
