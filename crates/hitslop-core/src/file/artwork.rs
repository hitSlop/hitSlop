//! The artwork a file may hold: its names, and the PNG rules every write and open apply.

use crate::error::{Result, invalid};

/// The artwork a file may hold, as the `artwork` table's CHECK names it. Its name is the
/// row's in the file and the image's in a build's stage (`artwork/<name>.png`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artwork {
    Preview,
    Icon,
}
impl Artwork {
    pub const ALL: [Artwork; 2] = [Artwork::Preview, Artwork::Icon];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Icon => "icon",
        }
    }
}
impl std::fmt::Display for Artwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl rusqlite::ToSql for Artwork {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}
/// Artwork as every open accepts it: a PNG within the image limits, at most one asset's size.
pub(crate) fn check_artwork(label: &str, bytes: &[u8]) -> Result<()> {
    if bytes.len() > crate::ASSET_FILE_BYTES {
        return Err(invalid(format!("{label} is too large")));
    }
    png(bytes, label).map(|_| ())
}
/// The decoded bytes optimizing may hold: artwork larger than this is stored as it is.
const OPTIMIZE_DECODED_BYTES: usize = 64 << 20;
/// Artwork that `check_artwork` accepted, losslessly smaller when oxipng finds a smaller
/// encoding at `level`: every pixel decodes the same, fully transparent ones too. Anything
/// oxipng refuses, panics on or cannot shrink is kept as it is, so optimizing never fails
/// a write.
pub(crate) fn optimize_png(bytes: Vec<u8>, level: u8) -> Vec<u8> {
    let options = oxipng::Options {
        // oxipng's display chunks (`StripChunks::Safe`) plus gAMA and cHRM, which macOS may
        // apply to a PNG without an sRGB or ICC chunk; other metadata goes.
        strip: oxipng::StripChunks::Keep(oxipng::indexset! {
            *b"cICP", *b"iCCP", *b"sRGB", *b"gAMA", *b"cHRM", *b"pHYs", *b"acTL", *b"fcTL", *b"fdAT"
        }),
        max_decompressed_size: Some(OPTIMIZE_DECODED_BYTES),
        ..oxipng::Options::from_preset(level)
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| oxipng::optimize_from_memory(&bytes, &options))) {
        Ok(Ok(smaller)) if smaller.len() < bytes.len() => smaller,
        _ => bytes,
    }
}
/// A PNG's width and height, and whether it carries alpha (colour type 6), from its header.
pub(super) fn png(bytes: &[u8], label: &str) -> Result<(u32, u32, bool)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 33 || bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return Err(invalid(format!("{label} must be a valid PNG")));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("4 bytes"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("4 bytes"));
    if width == 0
        || height == 0
        || width as usize > crate::IMAGE_SIDE
        || height as usize > crate::IMAGE_SIDE
        || (width as usize) * (height as usize) > crate::IMAGE_PIXELS
    {
        return Err(invalid(format!("{label} exceeds the PNG dimension limit")));
    }
    Ok((width, height, bytes[25] == 6))
}
