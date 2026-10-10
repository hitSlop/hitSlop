//! Code-generation values. Types use ts-rs; these small registries are serialized
//! from the same limits and enum variants the Rust checks consume.
use crate::app::package_format_1::*;
pub use crate::descriptor::Node;
use crate::wire::engine::ExportFormat;
use crate::wire::*;
use serde_json::{Value, json};

pub fn constants() -> Vec<(&'static str, Value)> {
    vec![
        (
            "BrowserHost",
            json!({"port": browser::PORT, "protocol": browser::PROTOCOL, "containerFormat": browser::CONTAINER_FORMAT, "chunkBytes": browser::TRANSFER_BYTES, "fileBytes": browser::FILE_BYTES}),
        ),
        ("BrowserResourcePolicy", json!(browser::page_policy())),
        ("BrowserHostPolicy", json!(browser::host_policy())),
        ("NativeResourcePolicy", json!(crate::NATIVE_RESOURCE_POLICY)),
        // The CLI substitutes its loopback origin; all policy directives live here.
        (
            "PreviewResourcePolicy",
            json!(
                "default-src 'none'; script-src {origin}/__shell__/ {origin}/__app__/ {origin}/__preview__/native.js 'wasm-unsafe-eval'; connect-src 'self' ws://127.0.0.1:* https: blob:; media-src 'self' https: blob:; frame-src https:; style-src {origin}/__app__/ 'unsafe-inline'; img-src 'self' data: https: blob:; font-src 'self' data:"
            ),
        ),
        (
            "PreviewHostPolicy",
            json!(
                "default-src 'none'; script-src {origin}/__preview__/resize.js; style-src 'unsafe-inline'; img-src 'self'; frame-src 'self'"
            ),
        ),
        ("SlopCategories", json!(crate::app::Category::ALL)),
        (
            "ManifestText",
            json!({
                "slug": {"minLength": SLUG_MIN, "maxLength": SLUG_MAX, "pattern": "^[a-z0-9]+(?:-[a-z0-9]+)*$"},
                "title": {"minLength": 1, "maxLength": TITLE_MAX},
                "description": {"minLength": 1, "maxLength": DESCRIPTION_MAX},
                "authorName": {"minLength": 1, "maxLength": AUTHOR_NAME_MAX, "pattern": "\\S"},
            }),
        ),
        (
            "WindowBounds",
            json!({"minWidth": crate::wire::WINDOW_MIN_WIDTH, "minHeight": crate::wire::WINDOW_MIN_HEIGHT, "max": crate::wire::WINDOW_MAX}),
        ),
        (
            "AttachmentLimits",
            json!({"file": ATTACHMENT_FILE_BYTES, "total": ATTACHMENT_BYTES, "count": ATTACHMENT_COUNT, "name": ATTACHMENT_NAME_BYTES}),
        ),
        ("AttachmentIdRule", json!({"length": ATTACHMENT_ID_LENGTH, "characters": ATTACHMENT_ID_CHARACTERS})),
        ("ExportFormats", json!([ExportFormat::Png, ExportFormat::Pdf])),
        ("ThemeFileLimit", json!(THEME_FILE_LIMIT)),
        // Shared host/SDK values also come from the Rust-owned boundary.
        ("ErrorTextLimit", json!(ERROR_TEXT)),
        ("OperationErrorBrand", json!("slop.operation-error")),
        ("PushLimits", json!({"items": 256, "bytes": 4 * 1024 * 1024})),
        ("SocketLimits", json!({"request": SOCKET_REQUEST, "attachment": SOCKET_ATTACHMENT})),
        (
            "RowIdRule",
            json!({"characters": ROW_ID_CHARACTERS, "maximum": ROW_ID_MAX,
            "mintAlphabet": std::str::from_utf8(ID_ALPHABET).expect("ASCII ID alphabet")}),
        ),
        ("CoreErrorCodes", json!(Code::ALL)),
        ("PackageFormat", json!(PACKAGE_FORMAT)),
        ("RuntimeABI", json!(RUNTIME_ABI)),
        ("HelperProtocol", json!({"version": HELPER_PROTOCOL})),
        ("OutcomeCodes", json!(OutcomeCode::ALL)),
    ]
}
