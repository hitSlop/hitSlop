//! Typed app interpretation. Storage checks resources separately before publishing an
//! opened app. A summary uses only the scalar metadata checker, never this decoder.
pub(crate) mod package_format_1;

use crate::arguments::Arguments;
use crate::build::BuildDeclaration;
use crate::{AppSpec, Code, Result, err, shape};
pub use package_format_1::{AppMetadata, Author, Background, Category, CommandInput, ThemeInput, Views};

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
        app.spec.schema.validate(&declaration.initial, false)?;
        Ok(app)
    }

    pub fn metadata(&self) -> &AppMetadata {
        &self.metadata
    }
    pub fn window(&self) -> &WindowDefinition {
        &self.window
    }
    pub fn theme(&self) -> &[ThemeInput] {
        &self.stored.theme
    }
    pub fn views(&self) -> Views {
        self.stored.views
    }
    pub fn commands(&self) -> &[CommandDefinition] {
        &self.commands
    }
    pub fn spec(&self) -> &AppSpec {
        &self.spec
    }

    /// Original descriptor representation for the bundled SDK's mount check. Never
    /// serialize the parsed Node here: that would discard descriptions.
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
                    background: *background,
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

/// The one check shared by display metadata and complete definition acceptance.
pub fn validate_metadata(metadata: &AppMetadata, package_format: u64) -> Result<()> {
    requirements(package_format.into(), 1)?;
    package_format_1::metadata(metadata)
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
