//! The artwork a file may hold: its names, and the PNG rules every write and open apply.

#[cfg(not(target_arch = "wasm32"))]
use crate::error::{Result, invalid};
#[cfg(not(target_arch = "wasm32"))]
use crate::images::{self, Purpose};

/// The artwork a file holds and hosts read; rows with other names are ignored. Its name is the
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
/// Every write fully decodes a bounded image before publishing it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn check_artwork(name: Artwork, bytes: &[u8]) -> Result<()> {
    let purpose = match name {
        Artwork::Preview => Purpose::Preview,
        Artwork::Icon => Purpose::Icon,
    };
    images::check(bytes, purpose).map(|_| ()).map_err(|e| invalid(format!("artwork/{name}.png: {}", e.message)))
}
/// The decoded bytes optimizing may hold: artwork larger than this is stored as it is.
#[cfg(not(target_arch = "wasm32"))]
const OPTIMIZE_DECODED_BYTES: usize = 64 << 20;
/// Artwork that `check_artwork` accepted, losslessly smaller when oxipng finds a smaller
/// encoding at `level`: every pixel decodes the same, fully transparent ones too. Anything
/// oxipng refuses, panics on or cannot shrink is kept as it is, so optimizing never fails
/// a write.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn optimize_png(bytes: Vec<u8>, level: u8) -> Vec<u8> {
    let options = oxipng::Options {
        // oxipng's display chunks (`StripChunks::Safe`) plus gAMA and cHRM, which macOS may
        // apply to a PNG without an sRGB or ICC chunk; other metadata goes.
        strip: oxipng::StripChunks::Keep(oxipng::indexset! {
            *b"cICP", *b"iCCP", *b"sRGB", *b"gAMA", *b"cHRM", *b"pHYs", *b"acTL", *b"fcTL", *b"fdAT"
        }),
        max_decompressed_size: Some(OPTIMIZE_DECODED_BYTES),
        // Acceptance requires RGB/RGBA at 8/16 bits; optimizing cannot change that.
        color_type_reduction: false,
        bit_depth_reduction: false,
        grayscale_reduction: false,
        ..oxipng::Options::from_preset(level)
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| oxipng::optimize_from_memory(&bytes, &options))) {
        Ok(Ok(smaller)) if smaller.len() < bytes.len() => smaller,
        _ => bytes,
    }
}
