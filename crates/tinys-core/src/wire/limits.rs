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
// The native evaluator and core must admit the same app ABI. The WASM core has no
// runner dependency; its command workers use the generated ABI prelude.
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
const _: () = assert!(RUNTIME_ABI == tinys_runner::RUNTIME_ABI, "core and runner runtime ABIs must agree");
#[cfg(feature = "storage")]
pub const ENGINE_PROTOCOL: u64 = 1;
/// The CSS `border-radius` of a window that names no shape. Package format 1 means this
/// by an absent shape, so it never changes.
pub(crate) const DEFAULT_WINDOW_RADIUS: &str = "22px";
/// Effective theme JSON, and a theme file, in UTF-8 bytes.
pub(crate) const THEME_LIMIT: usize = 65536;
pub(crate) const THEME_FILE_LIMIT: usize = 66560;
/// The longest theme token name, the most tokens a palette declares, and the token
/// prefix the host reserves.
pub(crate) const THEME_NAME_LIMIT: usize = 64;
pub(crate) const THEME_TOKENS: usize = 256;
pub(crate) const THEME_RESERVED_PREFIXES: &[&str] = &["window-", "host-", "platform-"];
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
/// The parser serves both stored descriptors/arguments and same-build page requests.
pub(crate) const JSON_BYTES: usize = if APP_TEXT_BYTES > PAGE_PAYLOAD { APP_TEXT_BYTES } else { PAGE_PAYLOAD };
#[cfg(feature = "storage")]
pub(crate) const PNG_DECODER_BYTES: usize = 8 << 20;
#[cfg(feature = "storage")]
pub(crate) const ICON_SIDE: u32 = 512;
#[cfg(feature = "storage")]
pub(crate) const ASSET_PATH_BYTES: usize = 240;
pub use tinys_sharing_protocol::limits::ASSET_BYTES;
pub use tinys_sharing_protocol::limits::ASSET_COUNT;
pub use tinys_sharing_protocol::limits::ASSET_FILE_BYTES;
pub const IMAGE_SIDE: usize = 16384;
pub const IMAGE_PIXELS: usize = 24000000;
pub use tinys_sharing_protocol::limits::ATTACHMENT_BYTES;
pub use tinys_sharing_protocol::limits::ATTACHMENT_COUNT;
/// A document's attachments: one file's bytes, their total bytes and their count.
pub use tinys_sharing_protocol::limits::ATTACHMENT_FILE_BYTES;
/// An attachment display name, in UTF-8 bytes.
pub const ATTACHMENT_NAME_BYTES: usize = 255;
/// A file downloaded by an authored page, independent of document attachment storage.
#[cfg(feature = "storage")]
pub(crate) const DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
/// A window shape's longest path data and radius list, and its largest view box side.
pub(crate) const SHAPE_PATH: usize = 4096;
pub(crate) const SHAPE_RADIUS: usize = 256;
pub(crate) const SHAPE_VIEW_BOX: f64 = 16384.0;
/// Acceptance bounds shared by stored descriptors and command arguments.
pub(crate) const DESCRIPTOR_DEPTH: usize = 16;
pub(crate) const DESCRIPTOR_FIELDS: usize = 1024;
pub(crate) const ENUM_VALUES: usize = 1024;
pub(crate) const DESCRIPTOR_DESCRIPTION: u64 = 500;
pub(crate) const MAX_SCALAR_LIST: usize = 100_000;
pub(crate) const RECORD_KEY_MAX: usize = 256;
pub(crate) const SHAPE_COMMANDS: usize = 512;
pub(crate) const SHAPE_SEGMENTS: usize = 2048;
pub(crate) const SHAPE_ARC_MIN_RADIUS: f64 = 1e-5;
pub(crate) const SHAPE_ARC_MIN_CHORD: f64 = 1e-100;
pub(crate) const SHAPE_ARC_TOLERANCE: f64 = 0.1;
pub(crate) const SHAPE_VIEW_BOX_MIN: f64 = 1.0;
#[cfg(feature = "storage")]
pub(crate) const COMMAND_NAME_BYTES: usize = 80;
#[cfg(feature = "storage")]
pub(crate) const COMMANDS: usize = 64;
/// Stored format-1 window bounds; authoring may use a narrower range.
#[cfg(feature = "storage")]
pub(crate) const OPEN_WINDOW_MIN_WIDTH: u32 = 240;
#[cfg(feature = "storage")]
pub(crate) const OPEN_WINDOW_MIN_HEIGHT: u32 = 180;
#[cfg(feature = "storage")]
pub(crate) const OPEN_WINDOW_MAX: u32 = 4096;
#[cfg(feature = "storage")]
pub(crate) const DEFINITION_BYTES: usize = APP_TEXT_BYTES + MANIFEST_BYTES + THEME_LIMIT + ASSET_FILE_BYTES;
#[cfg(feature = "storage")]
pub(crate) const CATEGORIES_MIN: usize = 1;
#[cfg(feature = "storage")]
pub(crate) const CATEGORIES_MAX: usize = 2;
/// Conservative SQLite row overhead reserved when saving a checkpoint.
#[cfg(feature = "storage")]
pub(crate) const STORAGE_ROW_OVERHEAD: usize = 512;
#[cfg(feature = "storage")]
pub(crate) const SHARE_ROOM_BYTES: usize = 128;
#[cfg(feature = "storage")]
pub(crate) const SHARE_ENDPOINT_BYTES: usize = 2048;
#[cfg(feature = "storage")]
pub(crate) const WEBM_HEADER_BYTES: u64 = 4096;
/// A batch's most intents and an intent's longest path; a page request's largest payload.
pub(crate) const BATCH_INTENTS: usize = 1000;
pub(crate) const PATH_SEGMENTS: usize = 64;
pub(crate) const PAGE_PAYLOAD: usize = 4194304;
#[cfg(feature = "storage")]
pub(crate) const PUSH_ITEMS: usize = 256;
#[cfg(feature = "storage")]
pub(crate) const PUSH_BYTES: usize = PAGE_PAYLOAD;
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
/// A speech locale identifier (`en-US`, `zh_Hant_TW`), in ASCII bytes, and a transcript,
/// in UTF-8 bytes. A recording is an attachment, so `ATTACHMENT_FILE_BYTES` bounds it.
#[cfg(feature = "storage")]
pub(crate) const SPEECH_LOCALE_BYTES: usize = 35;
#[cfg(feature = "storage")]
pub(crate) const SPEECH_TEXT_BYTES: usize = 1048576;
/// A locale identifier a speech request may name: a letter, then letters, digits, `-`
/// and `_`, as the page checks it before asking (`SPEECH_LOCALE_PATTERN`). The provider
/// decides whether it can recognize it.
#[cfg(feature = "ts")]
pub(crate) const SPEECH_LOCALE_PATTERN: &str = "^[A-Za-z][A-Za-z0-9_-]*$";
#[cfg(feature = "storage")]
pub(crate) fn valid_speech_locale(locale: &str) -> bool {
    locale.len() <= SPEECH_LOCALE_BYTES
        && locale.bytes().next().is_some_and(|b| b.is_ascii_alphabetic())
        && locale.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Native host values from the same constants used by decoding and persistence.
#[cfg(feature = "storage")]
pub struct HostLimits {
    pub error_text: u64,
    pub download_bytes: u64,
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
    pub engine_protocol: u64,
}
#[cfg(feature = "storage")]
pub fn host_limits() -> HostLimits {
    HostLimits {
        error_text: ERROR_TEXT as u64,
        download_bytes: DOWNLOAD_BYTES,
        storage_bytes: STORAGE_BYTES as u64,
        theme_file: THEME_FILE_LIMIT as u64,
        push_items: PUSH_ITEMS as u64,
        push_bytes: PUSH_BYTES as u64,
        socket_request: SOCKET_REQUEST as u64,
        socket_attachment: SOCKET_ATTACHMENT as u64,
        image_side: IMAGE_SIDE as u64,
        image_pixels: IMAGE_PIXELS as u64,
        min_width: WINDOW_MIN_WIDTH as u64,
        min_height: WINDOW_MIN_HEIGHT as u64,
        max_window: WINDOW_MAX as u64,
        package_format: PACKAGE_FORMAT,
        runtime_abi: RUNTIME_ABI,
        engine_protocol: ENGINE_PROTOCOL,
    }
}
/// Attachments share the origin for canvas use, but are never script sources.
#[cfg(feature = "storage")]
pub const NATIVE_RESOURCE_POLICY: &str = "default-src 'none'; script-src tiny://app/__shell__/ tiny://app/assets/ 'wasm-unsafe-eval'; connect-src tiny: https: blob:; media-src tiny: https: blob:; frame-src https:; style-src tiny://app/__shell__/ tiny://app/assets/ 'unsafe-inline'; img-src tiny: data: https: blob:; font-src tiny://app/assets/ data:";

/// A released bound's direction: maximums may rise, minimums may fall, exact values stay.
#[cfg(feature = "storage")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LimitKind {
    Max,
    Min,
    Exact,
}
#[cfg(feature = "storage")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Limit {
    pub name: String,
    #[serde(with = "decimal")]
    pub value: u64,
    pub kind: LimitKind,
}
/// JSON numbers cannot carry every u64 through TypeScript capture without rounding.
#[cfg(feature = "storage")]
mod decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "storage")]
impl Limit {
    pub fn accepts_recorded(&self, recorded: &Self) -> bool {
        self.name == recorded.name
            && self.kind == recorded.kind
            && match self.kind {
                LimitKind::Max => self.value >= recorded.value,
                LimitKind::Min => self.value <= recorded.value,
                LimitKind::Exact => self.value == recorded.value,
            }
    }
}

/// Retiring a limit requires an explicit review of its owning format. None are retired.
#[cfg(feature = "storage")]
pub const RETIRED_LIMITS: &[&str] = &[];

/// Same-build transport, authoring-only bounds, generated identities and contract metadata
/// are not stored-data acceptance limits. Format/ABI markers are recorded separately.
#[cfg(all(test, feature = "storage"))]
const NOT_PERSISTED: &[&str] = &[
    "WINDOW_MIN_WIDTH",
    "WINDOW_MIN_HEIGHT",
    "WINDOW_MAX",
    "PACKAGE_FORMAT",
    "RUNTIME_ABI",
    "ENGINE_PROTOCOL",
    "ID_ALPHABET",
    "DOWNLOAD_BYTES",
    "BATCH_INTENTS",
    "PATH_SEGMENTS",
    "PAGE_PAYLOAD",
    "PUSH_ITEMS",
    "PUSH_BYTES",
    "SOCKET_ATTACHMENT",
    "SOCKET_REQUEST",
    "ERROR_TEXT",
    "SPEECH_LOCALE_BYTES",
    "SPEECH_TEXT_BYTES",
    "SPEECH_LOCALE_PATTERN",
    "NATIVE_RESOURCE_POLICY",
    "NOT_PERSISTED",
    "RETIRED_LIMITS",
];

/// An exact fingerprint for nonnumeric acceptance rules (alphabets, reserved prefix,
/// default radius). Their behavior is also exercised by vectors and document replay.
#[cfg(feature = "storage")]
fn spelling(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}

/// All bounds that govern opening, reading or saving stored app/resource/state data.
/// A release records this table; every later build checks its bounds against that release.
#[cfg(feature = "storage")]
pub fn open_limits() -> Vec<Limit> {
    let mut limits = vec![];
    macro_rules! bound {
        ($name:ident, $kind:ident) => {
            limits.push(Limit { name: stringify!($name).into(), value: $name as u64, kind: LimitKind::$kind });
        };
    }
    bound!(ROW_ID_MAX, Max);
    bound!(THEME_LIMIT, Max);
    bound!(THEME_FILE_LIMIT, Max);
    bound!(THEME_NAME_LIMIT, Max);
    bound!(THEME_TOKENS, Max);
    bound!(STORAGE_BYTES, Max);
    bound!(STORAGE_ROWS, Max);
    bound!(MANIFEST_BYTES, Max);
    bound!(APP_TEXT_BYTES, Max);
    bound!(JSON_BYTES, Max);
    bound!(PNG_DECODER_BYTES, Max);
    bound!(ICON_SIDE, Exact);
    bound!(ASSET_PATH_BYTES, Max);
    bound!(ASSET_FILE_BYTES, Max);
    bound!(ASSET_COUNT, Max);
    bound!(ASSET_BYTES, Max);
    bound!(IMAGE_SIDE, Max);
    bound!(IMAGE_PIXELS, Max);
    bound!(ATTACHMENT_FILE_BYTES, Max);
    bound!(ATTACHMENT_BYTES, Max);
    bound!(ATTACHMENT_COUNT, Max);
    bound!(ATTACHMENT_NAME_BYTES, Max);
    bound!(SHAPE_PATH, Max);
    bound!(SHAPE_RADIUS, Max);
    bound!(SHAPE_VIEW_BOX, Max);
    bound!(DESCRIPTOR_DEPTH, Max);
    bound!(DESCRIPTOR_FIELDS, Max);
    bound!(ENUM_VALUES, Max);
    bound!(DESCRIPTOR_DESCRIPTION, Max);
    bound!(MAX_SCALAR_LIST, Max);
    bound!(RECORD_KEY_MAX, Max);
    bound!(SHAPE_COMMANDS, Max);
    bound!(SHAPE_SEGMENTS, Max);
    bound!(COMMAND_NAME_BYTES, Max);
    bound!(COMMANDS, Max);
    bound!(DEFINITION_BYTES, Max);
    bound!(OPEN_WINDOW_MAX, Max);
    bound!(CATEGORIES_MAX, Max);
    bound!(SHARE_ROOM_BYTES, Max);
    bound!(SHARE_ENDPOINT_BYTES, Max);
    bound!(WEBM_HEADER_BYTES, Max);
    bound!(OPEN_WINDOW_MIN_WIDTH, Min);
    bound!(OPEN_WINDOW_MIN_HEIGHT, Min);
    bound!(SHAPE_VIEW_BOX_MIN, Min);
    bound!(CATEGORIES_MIN, Min);
    bound!(STORAGE_ROW_OVERHEAD, Min);
    bound!(ATTACHMENT_ID_LENGTH, Exact);
    limits.push(Limit { name: "ROW_ID_CHARACTERS".into(), value: spelling(ROW_ID_CHARACTERS), kind: LimitKind::Exact });
    limits.push(Limit {
        name: "ATTACHMENT_ID_CHARACTERS".into(),
        value: spelling(ATTACHMENT_ID_CHARACTERS),
        kind: LimitKind::Exact,
    });
    limits.push(Limit {
        name: "DEFAULT_WINDOW_RADIUS".into(),
        value: spelling(DEFAULT_WINDOW_RADIUS),
        kind: LimitKind::Exact,
    });
    limits.push(Limit {
        name: "THEME_RESERVED_PREFIXES".into(),
        value: spelling(&THEME_RESERVED_PREFIXES.join(",")),
        kind: LimitKind::Exact,
    });
    // Positive IEEE-754 encodings preserve order, including these fractional minima.
    limits.push(Limit {
        name: "SHAPE_ARC_MIN_RADIUS".into(),
        value: SHAPE_ARC_MIN_RADIUS.to_bits(),
        kind: LimitKind::Min,
    });
    limits.push(Limit {
        name: "SHAPE_ARC_MIN_CHORD".into(),
        value: SHAPE_ARC_MIN_CHORD.to_bits(),
        kind: LimitKind::Min,
    });
    limits.push(Limit {
        name: "SHAPE_ARC_TOLERANCE".into(),
        value: SHAPE_ARC_TOLERANCE.to_bits(),
        kind: LimitKind::Exact,
    });
    limits.push(Limit { name: "MAX_SAFE".into(), value: crate::MAX_SAFE as u64, kind: LimitKind::Max });
    limits.sort_by(|a, b| a.name.cmp(&b.name));
    limits
}

#[cfg(feature = "storage")]
#[path = "acceptance.rs"]
mod acceptance;
#[cfg(feature = "storage")]
impl crate::app::AppDefinition {
    /// The app reader's stored-data bounds, including the resources and state it owns.
    pub fn open_limits() -> Vec<Limit> {
        open_limits()
    }

    /// Checks a release's limits table without passing u64 values through JavaScript numbers.
    pub fn check_recorded_limits(input: &str) -> crate::Result<()> {
        let recorded: Vec<Limit> = serde_json::from_str(input)
            .map_err(|error| crate::err(crate::Code::InvalidBytes, format!("Missing or invalid limits: {error}")))?;
        if recorded.is_empty() {
            return Err(crate::err(crate::Code::InvalidBytes, "Empty limits table"));
        }
        let current = open_limits();
        let mut names = std::collections::HashSet::new();
        for old in recorded {
            if !names.insert(old.name.clone()) {
                return Err(crate::err(crate::Code::InvalidBytes, format!("Duplicate limit {}", old.name)));
            }
            match current.iter().find(|limit| limit.name == old.name) {
                Some(now) if !now.accepts_recorded(&old) => {
                    return Err(crate::err(
                        crate::Code::InvalidBytes,
                        format!(
                            "{} became stricter: recorded {:?} {}, current {:?} {}",
                            old.name, old.kind, old.value, now.kind, now.value
                        ),
                    ));
                }
                None if !RETIRED_LIMITS.contains(&old.name.as_str()) => {
                    return Err(crate::err(
                        crate::Code::InvalidBytes,
                        format!("Recorded limit {} disappeared", old.name),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "storage"))]
mod tests {
    use super::*;

    #[test]
    fn limit_directions_and_u64_recording_are_lossless() {
        for (kind, lower, equal, higher) in [
            (LimitKind::Max, false, true, true),
            (LimitKind::Min, true, true, false),
            (LimitKind::Exact, false, true, false),
        ] {
            let old = Limit { name: "bound".into(), value: 5, kind };
            for (value, accepted) in [(4, lower), (5, equal), (6, higher)] {
                assert_eq!(Limit { value, ..old.clone() }.accepts_recorded(&old), accepted);
            }
        }
        let limit = Limit { name: "exact".into(), value: u64::MAX, kind: LimitKind::Exact };
        let encoded = serde_json::to_string(&limit).unwrap();
        assert!(encoded.contains("\"18446744073709551615\""));
        assert_eq!(serde_json::from_str::<Limit>(&encoded).unwrap(), limit);
    }

    #[test]
    fn every_constant_is_a_recorded_limit_or_explicitly_not_persisted() {
        let limits = open_limits();
        let names: std::collections::HashSet<_> = limits.iter().map(|limit| limit.name.as_str()).collect();
        assert_eq!(names.len(), limits.len(), "duplicate limit names");
        for line in include_str!("limits.rs").lines() {
            let line = line.trim_start();
            if line.starts_with("//") {
                continue;
            }
            let mut words = line.split_whitespace();
            while let Some(word) = words.next() {
                if word != "const" {
                    continue;
                }
                let name = words.next().unwrap().trim_end_matches(':');
                if name == "_" {
                    break;
                }
                assert!(names.contains(name) || NOT_PERSISTED.contains(&name), "unlisted acceptance constant: {name}");
                break;
            }
        }
        for name in NOT_PERSISTED {
            assert!(!names.contains(name), "{name} is both recorded and excluded");
        }
    }
}
