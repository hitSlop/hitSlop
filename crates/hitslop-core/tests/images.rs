//! Full PNG acceptance, including inputs image libraries commonly tolerate.
#![cfg(feature = "storage")]
use hitslop_core::images::{Purpose, check};
use png::{BitDepth, ColorType};

fn image(width: u32, height: u32, color: ColorType, depth: BitDepth) -> Vec<u8> {
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(depth);
    let length = width as usize * height as usize * color.samples() * depth as usize / 8;
    encoder.write_header().unwrap().write_image_data(&vec![127; length]).unwrap();
    bytes
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = u32::MAX;
    for byte in &out[4..] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
    out
}
fn with_chunk(bytes: &[u8], at: usize, kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    [&bytes[..at], &chunk(kind, data), &bytes[at..]].concat()
}

#[test]
fn rgb_rgba_and_adam7_are_decoded_at_eight_and_sixteen_bits() {
    for color in [ColorType::Rgb, ColorType::Rgba] {
        for depth in [BitDepth::Eight, BitDepth::Sixteen] {
            let bytes = image(37, 23, color, depth);
            let checked = check(&bytes, Purpose::Preview).unwrap();
            assert_eq!((checked.width, checked.height), (37, 23));
            let interlaced = oxipng::optimize_from_memory(
                &bytes,
                &oxipng::Options {
                    interlace: Some(true),
                    force: true,
                    color_type_reduction: false,
                    bit_depth_reduction: false,
                    grayscale_reduction: false,
                    ..oxipng::Options::from_preset(0)
                },
            )
            .unwrap();
            assert_eq!(interlaced[28], 1, "fixture uses Adam7");
            assert_eq!(check(&interlaced, Purpose::Preview).unwrap(), checked);
        }
    }
    for color in [ColorType::Grayscale, ColorType::GrayscaleAlpha] {
        assert!(check(&image(2, 2, color, BitDepth::Eight), Purpose::Preview).is_err());
    }
}

#[test]
fn skins_use_one_or_two_pixels_per_point_and_icons_are_512_square() {
    for scale in [1, 2] {
        let bytes = image(240 * scale, 180 * scale, ColorType::Rgba, BitDepth::Eight);
        assert_eq!(check(&bytes, Purpose::Skin { width: 240, height: 180 }).unwrap().scale, scale as u8);
    }
    for (w, h, color) in [
        (241, 180, ColorType::Rgba),
        (480, 180, ColorType::Rgba),
        (720, 540, ColorType::Rgba),
        (240, 180, ColorType::Rgb),
    ] {
        assert!(check(&image(w, h, color, BitDepth::Eight), Purpose::Skin { width: 240, height: 180 }).is_err());
    }
    assert!(check(&image(512, 512, ColorType::Rgba, BitDepth::Eight), Purpose::Icon).is_ok());
    assert!(check(&image(511, 512, ColorType::Rgba, BitDepth::Eight), Purpose::Icon).is_err());
}

#[test]
fn chunks_pixels_checksums_and_the_file_ending_must_be_complete() {
    let valid = image(2, 2, ColorType::Rgba, BitDepth::Eight);
    // Every truncation, including a complete header or complete pixels without IEND.
    for length in 0..valid.len() {
        assert!(check(&valid[..length], Purpose::Preview).is_err(), "length {length}");
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(check(&trailing, Purpose::Preview).is_err());
    let mut header_crc = valid.clone();
    header_crc[29] ^= 1;
    assert!(check(&header_crc, Purpose::Preview).is_err());
    let ancillary = with_chunk(&valid, 33, b"tEXt", b"Comment\0Hello");
    assert!(check(&ancillary, Purpose::Preview).is_ok());
    let mut ancillary_crc = ancillary;
    ancillary_crc[33 + 8] ^= 1;
    assert!(check(&ancillary_crc, Purpose::Preview).is_err());
    // Corrupt zlib's Adler32 but recompute the PNG CRC: both layers must be checked.
    let at = valid.windows(4).position(|w| w == b"IDAT").unwrap() - 4;
    let length = u32::from_be_bytes(valid[at..at + 4].try_into().unwrap()) as usize;
    let mut compressed = valid[at + 8..at + 8 + length].to_vec();
    *compressed.last_mut().unwrap() ^= 1;
    let bad_adler = [&valid[..at], &chunk(b"IDAT", &compressed), &valid[at + length + 12..]].concat();
    assert!(check(&bad_adler, Purpose::Preview).is_err());
    // Valid CRC and compressed bytes, but the PNG claims another full row.
    let mut header = valid[16..29].to_vec();
    header[4..8].copy_from_slice(&3u32.to_be_bytes());
    let missing_row = [&valid[..8], &chunk(b"IHDR", &header), &valid[33..]].concat();
    assert!(check(&missing_row, Purpose::Preview).is_err());
}

#[test]
fn animation_is_refused_even_when_its_control_is_malformed_or_late() {
    let valid = image(2, 2, ColorType::Rgba, BitDepth::Eight);
    for at in [33, valid.len() - 12] {
        for kind in [b"acTL", b"fcTL", b"fdAT"] {
            for payload in [&[][..], &[0; 8][..], &[0, 0, 0, 1, 0, 0, 0, 0][..]] {
                assert!(check(&with_chunk(&valid, at, kind, payload), Purpose::Preview).is_err());
            }
        }
    }
}

#[test]
fn dimension_and_metadata_budgets_apply_before_pixel_allocation() {
    let valid = image(2, 2, ColorType::Rgba, BitDepth::Eight);
    for (width, height) in [(0, 1), (16_385, 1), (1, 16_385), (6000, 6000), (u32::MAX, u32::MAX)] {
        let mut header = valid[16..29].to_vec();
        header[..4].copy_from_slice(&u32::to_be_bytes(width));
        header[4..8].copy_from_slice(&u32::to_be_bytes(height));
        let hostile = [&valid[..8], &chunk(b"IHDR", &header), &valid[33..]].concat();
        assert!(check(&hostile, Purpose::Preview).is_err(), "{width} × {height}");
    }
    let mut text = b"Comment\0".to_vec();
    text.resize(9 << 20, b'x');
    assert!(check(&with_chunk(&valid, 33, b"tEXt", &text), Purpose::Preview).is_err());
}
