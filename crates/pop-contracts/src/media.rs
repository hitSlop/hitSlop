#[derive(Clone, Copy, Debug)]
pub struct AssetType {
    pub extensions: &'static [&'static str],
    pub media_type: &'static str,
    pub canonical_extension: &'static str,
    pub compress: bool,
}

macro_rules! formats {
    ($( [$($extension:literal),+] => ($media:expr, $compress:literal) ),+ $(,)?) => {
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
    ["png"] => (AttachmentMedia::Png.as_str(), false),
    ["jpg", "jpeg"] => (AttachmentMedia::Jpeg.as_str(), false),
    ["webp"] => (AttachmentMedia::Webp.as_str(), false),
    ["gif"] => (AttachmentMedia::Gif.as_str(), false),
    ["avif"] => (AttachmentMedia::Avif.as_str(), false),
    ["heic", "heif"] => (AttachmentMedia::Heic.as_str(), false),
    ["woff"] => ("font/woff", false),
    ["woff2"] => ("font/woff2", false),
    ["ttf"] => ("font/ttf", false),
    ["otf"] => ("font/otf", false),
    ["mp3"] => (AttachmentMedia::Mpeg.as_str(), false),
    ["m4a"] => (AttachmentMedia::M4a.as_str(), false),
    ["wav"] => (AttachmentMedia::Wav.as_str(), false),
    ["ogg"] => (AttachmentMedia::Ogg.as_str(), false),
    ["flac"] => (AttachmentMedia::Flac.as_str(), false),
    ["mp4"] => (AttachmentMedia::Mp4.as_str(), false),
    ["mov"] => (AttachmentMedia::Quicktime.as_str(), false),
    ["webm"] => (AttachmentMedia::Webm.as_str(), false),
    ["pdf"] => (AttachmentMedia::Pdf.as_str(), false),
    ["bin"] => (AttachmentMedia::Binary.as_str(), false),
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
    if program(key).is_some() {
        return Some(asset_type("js"));
    }
    match key {
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

macro_rules! attachment_types {
    ($($name:ident => $media:literal),+ $(,)?) => {
        /// Passive attachment types emitted by signature sniffing.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum AttachmentMedia { $($name),+ }
        impl AttachmentMedia {
            pub const ALL: &[Self] = &[$(Self::$name),+];
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$name => $media),+ }
            }
        }
    };
}
attachment_types! {
    Binary => "application/octet-stream",
    Pdf => "application/pdf",
    Png => "image/png",
    Jpeg => "image/jpeg",
    Webp => "image/webp",
    Gif => "image/gif",
    Avif => "image/avif",
    Heic => "image/heic",
    Mpeg => "audio/mpeg",
    M4a => "audio/mp4",
    Wav => "audio/wav",
    Ogg => "audio/ogg",
    Flac => "audio/flac",
    Mp4 => "video/mp4",
    Quicktime => "video/quicktime",
    Webm => "video/webm",
}
pub fn attachment_media_type(kind: &str) -> bool {
    AttachmentMedia::ALL.iter().any(|value| value.as_str() == kind)
}

/// Fixed program roles, including whether the page may fetch their source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramRole {
    Ui,
    Commands,
    Widget,
    Icon,
    Export,
    Preview,
}
/// A fixed entry point and its host access policy.
#[derive(Debug)]
pub struct Program {
    pub key: &'static str,
    pub role: ProgramRole,
    pub public: bool,
}
pub const PROGRAMS: &[Program] = &[
    Program { key: "ui.js", role: ProgramRole::Ui, public: true },
    Program { key: "commands.js", role: ProgramRole::Commands, public: false },
    Program { key: "widget.js", role: ProgramRole::Widget, public: false },
    Program { key: "icon.js", role: ProgramRole::Icon, public: false },
    Program { key: "export.js", role: ProgramRole::Export, public: false },
    Program { key: "preview.js", role: ProgramRole::Preview, public: false },
];
pub fn program(key: &str) -> Option<&'static Program> {
    PROGRAMS.iter().find(|program| program.key == key)
}
