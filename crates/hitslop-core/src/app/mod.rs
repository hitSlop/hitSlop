//! Typed app interpretation. Storage checks resources separately before publishing an
//! opened app. A summary uses only the scalar metadata checker, never this decoder.
pub(crate) mod package_format_1;

use crate::arguments::Arguments;
use crate::build::BuildDeclaration;
use crate::wire::present_option;
use crate::{AppSpec, Code, Result, err, shape};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// The host and authoring model. A format module translates its stored structs into these,
// so they may grow or change without changing how a released file decodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum Category {
    Productivity,
    Utilities,
    Finance,
    Media,
    Games,
    DeveloperTools,
    Education,
    Business,
    Personal,
    Health,
    Creative,
    Music,
    Other,
}

impl Category {
    pub fn name(self) -> String {
        serde_json::to_value(self).expect("category").as_str().expect("string category").into()
    }
    pub fn parse(value: String) -> Result<Self> {
        serde_json::from_value(Value::String(value)).map_err(|_| err(Code::InvalidRequest, "Unknown category"))
    }

    /// Catalog order is declaration order, independent of its stored string spelling.
    pub const ALL: &[Self] = &[
        Self::Productivity,
        Self::Utilities,
        Self::Finance,
        Self::Media,
        Self::Games,
        Self::DeveloperTools,
        Self::Education,
        Self::Business,
        Self::Personal,
        Self::Health,
        Self::Creative,
        Self::Music,
        Self::Other,
    ];
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "app.generated.ts"))]
pub struct Author {
    pub name: String,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct AppMetadata {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: Author,
    pub categories: Vec<Category>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum Background {
    Transparent,
    Glass,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct ThemeInput {
    pub token: String,
    pub color: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct CommandInput {
    pub name: String,
    pub description: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub args: Value,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct Views {
    pub export: bool,
    pub icon: bool,
}

/// Native window semantics, independent of the stored definition's decoding rules.
#[derive(Clone, Debug)]
pub enum WindowDefinition {
    Standard {
        width: u32,
        height: u32,
        resizable: bool,
        lock_aspect: bool,
        background: Option<Background>,
        shape: shape::Silhouette,
    },
    Skin {
        width: u32,
        height: u32,
        skin: String,
    },
}

#[derive(Clone, Debug)]
pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    pub args: Arguments,
}

/// Structurally accepted definition. The file boundary additionally checks resource
/// keys, types, bytes and budgets before exposing it as an opened app.
#[derive(Clone, Debug)]
pub struct AppDefinition {
    metadata: AppMetadata,
    stored: package_format_1::Definition,
    window: WindowDefinition,
    theme: Vec<ThemeInput>,
    views: Views,
    commands: Vec<CommandDefinition>,
    spec: AppSpec,
}

impl AppDefinition {
    /// Dispatch before decoding either current metadata or the recursive definition.
    /// Keeping the decoder in its format module lets a future host model evolve.
    pub fn decode(package_format: u64, runtime_abi: u64, metadata: &str, definition: &str) -> Result<Self> {
        requirements(package_format.into(), runtime_abi.into())?;
        match package_format {
            1 => package_format_1::decode(metadata, definition),
            _ => unreachable!("requirements dispatch covers every admitted format"),
        }
    }

    /// Authoring and pack use the stored format's checks. The caller resolves the
    /// imported skin URL to a resource key; the URL itself never enters stored JSON.
    pub fn from_declaration(declaration: &BuildDeclaration, skin: Option<&str>) -> Result<Self> {
        let app = package_format_1::from_declaration(declaration, skin)?;
        app.spec.schema.validate(&app.spec.schema.with_defaults(&declaration.initial), false)?;
        Ok(app)
    }

    pub fn metadata(&self) -> &AppMetadata {
        &self.metadata
    }
    pub fn window(&self) -> &WindowDefinition {
        &self.window
    }
    pub fn theme(&self) -> &[ThemeInput] {
        &self.theme
    }
    pub fn views(&self) -> Views {
        self.views
    }
    pub fn commands(&self) -> &[CommandDefinition] {
        &self.commands
    }
    pub fn spec(&self) -> &AppSpec {
        &self.spec
    }

    /// Original descriptor representation for the bundled SDK's mount check. Never
    /// serialize the parsed Node here: that would discard descriptions.
    pub fn document_raw(&self) -> &serde_json::value::RawValue {
        &self.stored.document
    }
    pub fn document_json(&self) -> &str {
        self.stored.document.get()
    }
    /// The shell's window projection keeps authored geometry; the native window uses
    /// the normalized silhouette. Skin references are package URLs, never source paths.
    pub fn page_window(&self) -> crate::build::WindowInput {
        use package_format_1::Window;
        match &self.stored.window {
            Window::Standard { width, height, resizable, lock_aspect, background, shape } => {
                crate::build::WindowInput::Standard {
                    width: *width,
                    height: *height,
                    resizable: *resizable,
                    lock_aspect: *lock_aspect,
                    background: background.map(Into::into),
                    shape: shape.clone(),
                }
            }
            Window::Skin { width, height, skin } => {
                crate::build::WindowInput::Skin { width: *width, height: *height, image: format!("/assets/{skin}") }
            }
        }
    }
    pub fn theme_json(&self) -> String {
        crate::encode(&self.spec.theme_tokens().iter().cloned().collect::<std::collections::BTreeMap<_, _>>())
    }
    /// Tool metadata is a projection; JSON Schema never enters the saved definition.
    pub fn command_metadata(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.commands
                .iter()
                .map(|c| (c.name.clone(), serde_json::json!({"description":c.description,"args":c.args.json_schema()})))
                .collect(),
        )
    }
    pub fn definition_json(&self) -> String {
        crate::encode(&self.stored)
    }
}

/// The current authoring rules for a new app's metadata (`slop init`); never a saved file's.
pub fn validate_metadata(metadata: &AppMetadata) -> Result<()> {
    package_format_1::authoring_metadata(metadata)
}

/// Permanent marker checks, before interpreting any current-format fields. i128
/// accommodates unsigned wire markers and signed SQLite values without truncation.
pub(crate) fn requirements(package_format: i128, runtime_abi: i128) -> Result<()> {
    let markers =
        [("package format", package_format, crate::PACKAGE_FORMAT), ("runtime ABI", runtime_abi, crate::RUNTIME_ABI)];
    if let Some((name, level, supported)) = markers.iter().find(|(_, level, supported)| *level > *supported as i128) {
        return Err(err(
            Code::RequiresUpdate,
            format!("This slop needs {name} {level}; this hitSlop supports {supported}"),
        ));
    }
    if let Some((name, ..)) = markers.iter().find(|(_, level, _)| *level < 1) {
        return Err(err(Code::InvalidRequest, format!("Invalid {name}")));
    }
    Ok(())
}

// Freeze this module when format 1 releases. Pre-launch fixtures may start fresh.
const _: () = assert!(crate::PACKAGE_FORMAT == 1, "add a package reader and keep the released format's reader");
