//! Bounded PNG acceptance. Pixel validation borrows one decoder row at a time;
//! neither artwork nor skins allocate a full decoded frame here.
use crate::{Code, Result, err};
use png::{BitDepth, ColorType, DecodeOptions, Decoder, Limits};
use std::io::Cursor;

const DECODER_BYTES: usize = 8 << 20;

#[derive(Clone, Copy, Debug)]
pub enum Purpose {
    Preview,
    Icon,
    Skin { width: u32, height: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedPng {
    pub width: u32,
    pub height: u32,
    /// Pixels per declared point for a skin; 1 for standalone artwork.
    pub scale: u8,
}

fn invalid(message: impl ToString) -> crate::Error {
    err(Code::InvalidRequest, message)
}
fn decoder<R: std::io::BufRead + std::io::Seek>(source: R) -> Decoder<R> {
    let mut options = DecodeOptions::default();
    options.set_ignore_adler32(false);
    options.set_skip_ancillary_crc_failures(false);
    let mut decoder = Decoder::new_with_options(source, options);
    decoder.set_limits(Limits { bytes: DECODER_BYTES });
    decoder
}

fn dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0
        || height == 0
        || width as usize > crate::IMAGE_SIDE
        || height as usize > crate::IMAGE_SIDE
        || width.checked_mul(height).is_none_or(|pixels| pixels as usize > crate::IMAGE_PIXELS)
    {
        return Err(invalid("PNG exceeds the image dimension limit"));
    }
    Ok(())
}

/// Cheap stored-artwork metadata check. A header is not a successful pixel decode;
/// full validation happens at every write, and every skin open.
pub(crate) fn header(bytes: &[u8]) -> Result<()> {
    let mut decoder = decoder(Cursor::new(bytes));
    let info = decoder.read_header_info().map_err(|e| invalid(format!("Invalid PNG header: {e}")))?;
    dimensions(info.width, info.height)
}

pub fn check(bytes: &[u8], purpose: Purpose) -> Result<CheckedPng> {
    if bytes.len() > crate::ASSET_FILE_BYTES {
        return Err(invalid("PNG exceeds the asset byte limit"));
    }
    refuse_animation(bytes)?;
    let mut cursor = Cursor::new(bytes);
    let result;
    {
        let mut reader = decoder(&mut cursor).read_info().map_err(|e| invalid(format!("Invalid PNG: {e}")))?;
        let info = reader.info();
        dimensions(info.width, info.height)?;
        if !matches!(info.bit_depth, BitDepth::Eight | BitDepth::Sixteen)
            || !matches!(info.color_type, ColorType::Rgb | ColorType::Rgba)
        {
            return Err(invalid("Use an RGB or RGBA PNG with 8 or 16 bits per channel"));
        }
        if info.animation_control.is_some() || info.frame_control.is_some() {
            return Err(invalid("Skins and artwork must be still PNG images"));
        }
        let scale = match purpose {
            Purpose::Preview => 1,
            Purpose::Icon => {
                if (info.width, info.height) != (512, 512) {
                    return Err(invalid("Icon artwork must be 512 × 512 pixels"));
                }
                1
            }
            Purpose::Skin { width, height } => {
                if info.color_type != ColorType::Rgba {
                    return Err(invalid("A window skin must be an RGBA PNG with alpha"));
                }
                let pixels = (info.width, info.height);
                if pixels == (width, height) {
                    1
                } else if width.checked_mul(2).zip(height.checked_mul(2)) == Some(pixels) {
                    2
                } else {
                    return Err(invalid(format!(
                        "Skin must be {width} × {height} or {} × {} pixels",
                        u64::from(width) * 2,
                        u64::from(height) * 2
                    )));
                }
            }
        };
        // next_row's own output buffer is outside png::Limits. Bound it explicitly
        // before the first allocation, including 16-bit RGBA's eight bytes per pixel.
        let channels = if info.color_type == ColorType::Rgba { 4 } else { 3 };
        let sample = if info.bit_depth == BitDepth::Sixteen { 2 } else { 1 };
        let row = (info.width as usize).checked_mul(channels * sample);
        if row.is_none_or(|length| length > crate::IMAGE_SIDE * 8) {
            return Err(invalid("PNG row exceeds its byte limit"));
        }
        result = CheckedPng { width: info.width, height: info.height, scale };
        // Adam7 yields rows per pass; counting these against height would reject it.
        while reader.next_row().map_err(|e| invalid(format!("Invalid PNG pixels: {e}")))?.is_some() {}
        reader.finish().map_err(|e| invalid(format!("Invalid PNG ending: {e}")))?;
    }
    if cursor.position() != bytes.len() as u64 {
        return Err(invalid("PNG contains bytes after IEND"));
    }
    Ok(result)
}

/// png deliberately ignores some malformed acTL chunks (including late or zero-frame
/// controls). Inspect only the bounded chunk framing to forbid every animation chunk;
/// the library remains responsible for CRCs, compression, filtering and pixels.
fn refuse_animation(bytes: &[u8]) -> Result<()> {
    let mut at = 8usize;
    while at < bytes.len() {
        let length = bytes.get(at..at + 4).ok_or_else(|| invalid("Truncated PNG chunk"))?;
        let length = u32::from_be_bytes(length.try_into().expect("four bytes")) as usize;
        let end = at
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| invalid("Truncated PNG chunk"))?;
        if matches!(&bytes[at + 4..at + 8], b"acTL" | b"fcTL" | b"fdAT") {
            return Err(invalid("Skins and artwork must be still PNG images"));
        }
        at = end;
    }
    Ok(())
}
