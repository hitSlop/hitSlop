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

/// The TypeScript bundler's projection of the Rust-owned media registry.
pub fn build_registry() -> serde_json::Value {
    ASSET_TYPES
        .iter()
        .flat_map(|kind| {
            kind.extensions.iter().map(move |extension| {
                (extension.to_string(), serde_json::json!([kind.media_type, kind.canonical_extension]))
            })
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// Fixed entry names or a content-addressed resource with a canonical extension.
pub fn asset_key(key: &str) -> Option<&'static AssetType> {
    match key {
        "ui.js" | "commands.js" | "widget.js" | "icon.js" | "export.js" | "preview.js" => Some(asset_type("js")),
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
