//! Code-generation values. Types use ts-rs; these small registries are serialized
//! from the same limits and enum variants the Rust checks consume.
use crate::app::format::*;
pub use crate::descriptor::Node;
use crate::wire::engine::ExportFormat;
use crate::wire::*;
use serde_json::{Value, json};

pub fn constants() -> Vec<(&'static str, Value)> {
    vec![
        ("DocumentLineage", json!(pop_contracts::lineage::DOCUMENT_LINEAGE)),
        ("CompactLayoutWidth", json!(pop_contracts::wire::page::COMPACT_LAYOUT_WIDTH)),
        ("NativeResourcePolicy", json!(pop_contracts::policy::NATIVE_RESOURCE_POLICY)),
        // The CLI substitutes its loopback origin; all policy directives live here.
        ("PreviewResourcePolicy", json!(pop_contracts::policy::PREVIEW_RESOURCE_POLICY)),
        ("PreviewHostPolicy", json!(pop_contracts::policy::PREVIEW_HOST_POLICY)),
        ("PopCategories", json!(pop_contracts::app::Category::ALL)),
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
        ("OperationErrorBrand", json!("pop.operation-error")),
        ("SpeechErrorBrand", json!("pop.speech-error")),
        (
            "SpeechLimits",
            json!({"localeBytes": SPEECH_LOCALE_BYTES, "localePattern": SPEECH_LOCALE_PATTERN, "textBytes": SPEECH_TEXT_BYTES}),
        ),
        ("WidgetViewports", json!(pop_contracts::widgets::widget_viewports())),
        ("PushLimits", json!({"items": PUSH_ITEMS, "bytes": PUSH_BYTES})),
        ("ResyncPush", json!(PagePush::<pop_contracts::wire::Publication>::Resync)),
        ("SocketLimits", json!({"request": SOCKET_REQUEST, "attachment": SOCKET_ATTACHMENT})),
        (
            "RowIdRule",
            json!({"characters": ROW_ID_CHARACTERS, "maximum": ROW_ID_MAX,
            "mintAlphabet": std::str::from_utf8(ID_ALPHABET).expect("ASCII ID alphabet")}),
        ),
        ("CoreErrorCodes", json!(Code::ALL)),
        ("PackageFormat", json!(PACKAGE_FORMAT)),
        ("RuntimeABI", json!(RUNTIME_ABI)),
        ("EngineProtocol", json!({"version": ENGINE_PROTOCOL})),
        ("OutcomeCodes", json!(OutcomeCode::ALL)),
    ]
}
