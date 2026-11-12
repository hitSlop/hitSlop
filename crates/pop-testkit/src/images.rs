//! PNG fixtures and independent pixel decoding for storage and capture tests.
/// Encodes an opaque white RGB or RGBA fixture.
pub fn png(width: u32, height: u32, color_type: u8) -> Vec<u8> {
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(if color_type == 6 { png::ColorType::Rgba } else { png::ColorType::Rgb });
    encoder.set_depth(png::BitDepth::Eight);
    let channels = if color_type == 6 { 4 } else { 3 };
    encoder.write_header().unwrap().write_image_data(&vec![255; width as usize * height as usize * channels]).unwrap();
    bytes
}

/// Encodes supplied RGBA pixels without compression.
pub fn encoded(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::NoCompression);
    encoder.write_header().unwrap().write_image_data(pixels).unwrap();
    bytes
}

/// Decodes PNG pixels to normalized eight-bit RGBA.
pub fn decoded(png: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buffer).unwrap();
    let pixels = &buffer[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => unreachable!("normalizing expands a palette"),
    }
}

/// A renderer-sized uncompressed image and its independent expected pixels.
pub fn render_of(shade: u8) -> (Vec<u8>, Vec<u8>) {
    let pixels: Vec<u8> = (0..512 * 512usize).flat_map(|i| [shade, (i % 251) as u8, 0x77, 255]).collect();
    (encoded(512, 512, &pixels), pixels)
}
