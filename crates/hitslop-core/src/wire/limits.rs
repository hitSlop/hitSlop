//! Limits and requirements owned by the Rust contract.
#[cfg(feature = "storage")]
pub(crate) const WINDOW_MIN_WIDTH: u32 = 240;
#[cfg(feature = "storage")]
pub(crate) const WINDOW_MIN_HEIGHT: u32 = 180;
#[cfg(feature = "storage")]
pub(crate) const WINDOW_MAX: u32 = 4096;
pub(crate) const ROW_ID_MAX: usize = 64;
pub(crate) const ROW_ID_CHARACTERS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_-";
#[cfg(feature = "storage")]
pub(crate) const ATTACHMENT_ID_LENGTH: usize = 64;
#[cfg(feature = "storage")]
pub(crate) const ATTACHMENT_ID_CHARACTERS: &str = "0123456789abcdef";
/// The platform level this build runs; a template or document above it needs a newer app.
pub const PACKAGE_FORMAT: u64 = 1;
pub const RUNTIME_ABI: u64 = 1;
#[cfg(feature = "storage")]
pub const HELPER_PROTOCOL: u64 = 1;
/// The CSS `border-radius` of a window whose manifest names no shape.
pub(crate) const DEFAULT_WINDOW_RADIUS: &str = "22px";
/// Effective theme JSON, and a theme file, in UTF-8 bytes.
pub(crate) const THEME_LIMIT: usize = 65536;
pub(crate) const THEME_FILE_LIMIT: usize = 66560;
/// The longest theme token name, the most tokens a palette declares, and the token
/// prefix the host reserves.
pub(crate) const THEME_NAME_LIMIT: usize = 64;
pub(crate) const THEME_TOKENS: usize = 256;
pub(crate) const THEME_RESERVED_PREFIX: &str = "window-";
/// The lowercase Crockford alphabet of minted and derived row IDs.
pub(crate) const ID_ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
/// A document's saved checkpoint plus updates, in bytes, and its update rows.
pub const STORAGE_BYTES: usize = 33554432;
pub const STORAGE_ROWS: usize = 4096;
/// An app: its manifest and its descriptor or initial values, in bytes, and its longest
/// asset path; one asset's bytes, the asset count and their total bytes; and the largest
/// image it may carry, per side and in pixels.
#[cfg(feature = "storage")]
pub(crate) const MANIFEST_BYTES: usize = 65536;
pub(crate) const APP_TEXT_BYTES: usize = 4194304;
#[cfg(feature = "storage")]
pub(crate) const ASSET_PATH_BYTES: usize = 240;
pub const ASSET_FILE_BYTES: usize = 26214400;
pub const ASSET_COUNT: usize = 256;
pub const ASSET_BYTES: usize = 52428800;
pub const IMAGE_SIDE: usize = 16384;
pub const IMAGE_PIXELS: usize = 24000000;
/// A document's attachments: one file's bytes, their total bytes and their count.
pub const ATTACHMENT_FILE_BYTES: usize = 10485760;
pub const ATTACHMENT_BYTES: usize = 104857600;
pub const ATTACHMENT_COUNT: usize = 256;
/// A window shape's longest path data and radius list, and its largest view box side.
pub(crate) const SHAPE_PATH: usize = 4096;
pub(crate) const SHAPE_RADIUS: usize = 256;
pub(crate) const SHAPE_VIEW_BOX: f64 = 16384.0;
/// A batch's most intents and an intent's longest path; a page request's largest payload.
pub(crate) const BATCH_INTENTS: usize = 1000;
pub(crate) const PATH_SEGMENTS: usize = 64;
pub(crate) const PAGE_PAYLOAD: usize = 4194304;
/// The largest socket request, an attachment upload; no envelope the core checks is larger.
#[cfg(feature = "storage")]
pub const SOCKET_ATTACHMENT: usize = 16777216;
#[cfg(feature = "storage")]
pub const SOCKET_REQUEST: usize = 1048576;
/// An attachment's identity: the SHA-256 of its bytes, in lowercase hex.
#[cfg(feature = "storage")]
pub(crate) fn valid_attachment_id(id: &str) -> bool {
    id.len() == ATTACHMENT_ID_LENGTH && id.bytes().all(|b| ATTACHMENT_ID_CHARACTERS.as_bytes().contains(&b))
}
pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= ROW_ID_MAX && value.bytes().all(|b| ROW_ID_CHARACTERS.as_bytes().contains(&b))
}

/// Diagnostic text, in the page's UTF-16 units.
#[cfg(feature = "storage")]
pub(crate) const ERROR_TEXT: usize = 4096;

/// Native host values from the same constants used by decoding and persistence.
#[cfg(feature = "storage")]
pub struct HostLimits {
    pub error_text: u64,
    pub storage_bytes: u64,
    pub theme_file: u64,
    pub push_items: u64,
    pub push_bytes: u64,
    pub socket_request: u64,
    pub socket_attachment: u64,
    pub image_side: u64,
    pub image_pixels: u64,
    pub min_width: u64,
    pub min_height: u64,
    pub max_window: u64,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub helper_protocol: u64,
}
#[cfg(feature = "storage")]
pub fn host_limits() -> HostLimits {
    HostLimits {
        error_text: ERROR_TEXT as u64,
        storage_bytes: STORAGE_BYTES as u64,
        theme_file: THEME_FILE_LIMIT as u64,
        push_items: 256,
        push_bytes: PAGE_PAYLOAD as u64,
        socket_request: SOCKET_REQUEST as u64,
        socket_attachment: SOCKET_ATTACHMENT as u64,
        image_side: IMAGE_SIDE as u64,
        image_pixels: IMAGE_PIXELS as u64,
        min_width: WINDOW_MIN_WIDTH as u64,
        min_height: WINDOW_MIN_HEIGHT as u64,
        max_window: WINDOW_MAX as u64,
        package_format: PACKAGE_FORMAT,
        runtime_abi: RUNTIME_ABI,
        helper_protocol: HELPER_PROTOCOL,
    }
}
/// Attachments share the origin for canvas use, but are never script sources.
#[cfg(feature = "storage")]
pub const NATIVE_RESOURCE_POLICY: &str = "default-src 'none'; script-src slop://app/__shell__/ slop://app/assets/ 'wasm-unsafe-eval'; connect-src slop: https: blob:; media-src slop: https: blob:; frame-src https:; style-src slop://app/__shell__/ slop://app/assets/ 'unsafe-inline'; img-src slop: data: https: blob:; font-src slop://app/assets/ data:";
