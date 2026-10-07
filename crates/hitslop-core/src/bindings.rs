//! Code-generation values. Types use ts-rs; these small registries are serialized
//! from the same limits and enum variants the Rust checks consume.
use crate::app::package_format_1::*;
use crate::wire::engine::ExportFormat;
use crate::wire::*;
use serde_json::{Value, json};

pub fn constants() -> Vec<(&'static str, Value)> {
    vec![
        ("NativeResourcePolicy", json!(crate::NATIVE_RESOURCE_POLICY)),
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
        ("DefaultWindowRadius", json!(DEFAULT_WINDOW_RADIUS)),
        (
            "AttachmentLimits",
            json!({"file": ATTACHMENT_FILE_BYTES, "total": ATTACHMENT_BYTES, "count": ATTACHMENT_COUNT, "name": 255}),
        ),
        ("AttachmentIdRule", json!({"length": ATTACHMENT_ID_LENGTH, "characters": ATTACHMENT_ID_CHARACTERS})),
        (
            "ThemeTokenRule",
            json!({"nameLength": THEME_NAME_LIMIT, "reservedPrefix": THEME_RESERVED_PREFIX, "tokens": THEME_TOKENS}),
        ),
        (
            "AssetLimits",
            json!({"file": ASSET_FILE_BYTES, "count": ASSET_COUNT, "bytes": ASSET_BYTES, "imageSide": IMAGE_SIDE, "imagePixels": IMAGE_PIXELS}),
        ),
        ("AppLimits", json!({"manifest": MANIFEST_BYTES, "text": APP_TEXT_BYTES, "assetPath": ASSET_PATH_BYTES})),
        ("ShapeLimits", json!({"path": SHAPE_PATH, "radius": SHAPE_RADIUS, "viewBox": SHAPE_VIEW_BOX})),
        ("ExportFormats", json!([ExportFormat::Png, ExportFormat::Pdf])),
        ("BatchLimits", json!({"intents": BATCH_INTENTS, "pathSegments": PATH_SEGMENTS})),
        ("StorageLimits", json!({"bytes": STORAGE_BYTES, "rows": STORAGE_ROWS})),
        ("ThemeLimit", json!(THEME_LIMIT)),
        ("ThemeFileLimit", json!(THEME_FILE_LIMIT)),
        // These host/SDK values have no core consumer. They live here so generated
        // Swift and TypeScript still receive one value during boundary migration.
        ("ErrorTextLimit", json!(ERROR_TEXT)),
        ("OperationErrorBrand", json!("hitslop.operation-error")),
        ("PagePayloadLimit", json!(PAGE_PAYLOAD)),
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
