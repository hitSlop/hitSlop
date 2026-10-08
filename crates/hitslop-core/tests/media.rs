#![cfg(feature = "storage")]
use hitslop_core::media::{ASSET_TYPES, asset_key, asset_type, attachment_type, check_asset};

#[test]
fn attachments_use_passive_signatures_and_never_executable_text() {
    for (bytes, mime) in [
        (b"\x89PNG\r\n\x1a\n".as_slice(), "image/png"),
        (b"\xff\xd8\xff", "image/jpeg"),
        (b"GIF89a", "image/gif"),
        (b"RIFFxxxxWEBP", "image/webp"),
        (b"RIFFxxxxWAVE", "audio/wav"),
        (b"ID3", "audio/mpeg"),
        (b"\xff\xfb\x90", "audio/mpeg"),
        (b"OggS", "audio/ogg"),
        (b"fLaC", "audio/flac"),
        (b"%PDF-1.7", "application/pdf"),
    ] {
        assert_eq!(attachment_type(bytes), mime);
        check_asset(mime, bytes).unwrap();
    }
    for bytes in [
        b"<svg onload='alert(1)'/>".as_slice(),
        b"<!doctype html>",
        b"<html/>",
        b"<?xml version='1.0'?>",
        b"alert(1)",
        b"@import 'x';",
        b"hello",
        b"wOFF",
        b"wOF2",
        b"\0asm\x01\0\0\0",
        b"",
        b"\xff\xf1\x50",
    ] {
        assert_eq!(attachment_type(bytes), "application/octet-stream", "{bytes:?}");
    }
}

#[test]
fn iso_media_distinguishes_images_audio_video_and_unknown_brands() {
    let ftyp = |major: &[u8; 4], compatible: &[u8]| {
        let mut bytes = ((16 + compatible.len()) as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(b"ftyp");
        bytes.extend_from_slice(major);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(compatible);
        bytes
    };
    for (major, compatible, mime) in [
        (b"avif", b"".as_slice(), "image/avif"),
        (b"mif1", b"avif", "image/avif"),
        (b"heic", b"mif1", "image/heic"),
        (b"M4A ", b"isom", "audio/mp4"),
        (b"qt  ", b"", "video/quicktime"),
        (b"isom", b"mp42", "video/mp4"),
        (b"????", b"", "application/octet-stream"),
    ] {
        let bytes = ftyp(major, compatible);
        assert_eq!(attachment_type(&bytes), mime);
        if mime != "application/octet-stream" {
            check_asset(mime, &bytes).unwrap();
        }
        for length in 0..bytes.len() {
            assert_eq!(attachment_type(&bytes[..length]), "application/octet-stream");
        }
    }
}

#[test]
fn webm_requires_a_doctype_element_in_a_bounded_ebml_header() {
    let webm = b"\x1a\x45\xdf\xa3\x87\x42\x82\x84webm";
    assert_eq!(attachment_type(webm), "video/webm");
    for length in 0..webm.len() {
        assert_eq!(attachment_type(&webm[..length]), "application/octet-stream");
    }
    for bytes in [
        b"\x1a\x45\xdf\xa3\x8b\x42\x82\x88matroska".as_slice(),
        b"\x1a\x45\xdf\xa3\x87\x42\x83\x84webm", // wrong element ID
        b"\x1a\x45\xdf\xa3\xff\x42\x82\x84webm", // unknown/truncated length
        b"\x1a\x45\xdf\xa3\x01\0\0\0\0\0\0\0",   // eight-byte zero length
        b"\x1a\x45\xdf\xa3\0webm",               // invalid length
    ] {
        assert_eq!(attachment_type(bytes), "application/octet-stream", "{bytes:?}");
    }
}

#[test]
fn app_formats_match_signatures_and_fonts_remain_uncompressed() {
    for (extension, bytes) in [
        ("woff", b"wOFF".as_slice()),
        ("woff2", b"wOF2"),
        ("ttf", b"\0\x01\0\0"),
        ("otf", b"OTTO"),
        ("wasm", b"\0asm\x01\0\0\0"),
    ] {
        let kind = asset_type(extension);
        check_asset(kind.media_type, bytes).unwrap();
        assert!(check_asset(kind.media_type, b"wrong format").is_err());
        assert_eq!(kind.compress, extension == "wasm");
    }
    assert!(check_asset("image/jpeg", b"\x89PNG\r\n\x1a\n").is_err());
    assert!(check_asset("text/javascript", b"\xff").is_err());
    assert!(check_asset("text/html", b"<html/>").is_err());
    check_asset("image/svg+xml", b"<svg/>").unwrap();
}

#[test]
fn resource_keys_use_one_registry_and_refuse_traversal_aliases_and_noncanonical_hashes() {
    let hash = "a".repeat(64);
    for kind in ASSET_TYPES {
        for extension in kind.extensions {
            assert_eq!(asset_type(&extension.to_uppercase()).media_type, kind.media_type);
        }
        let key = format!("media/{hash}.{}", kind.canonical_extension);
        assert_eq!(asset_key(&key).unwrap().media_type, kind.media_type);
    }
    assert_eq!(asset_type("jpeg").canonical_extension, "jpg");
    assert_eq!(asset_type("unrecognized").canonical_extension, "bin");
    assert_eq!(asset_key("commands.js").unwrap().media_type, "text/javascript");
    assert_eq!(asset_key("ui.css").unwrap().media_type, "text/css");
    for key in [
        "/ui.js".into(),
        "../ui.js".into(),
        "media/../ui.js".into(),
        format!("media/{hash}.jpeg"),
        format!("media/{hash}.PNG"),
        format!("media/{}.png", hash.to_uppercase()),
        format!("media/{hash}.png/extra"),
        format!("media/{hash}.png?x"),
        "media/short.png".into(),
        "ui.js\0".into(),
    ] {
        assert!(asset_key(&key).is_none(), "{key:?}");
    }
}
