//! Typed app interpretation. Storage checks resources separately before publishing an
//! opened app. A summary uses only the scalar metadata checker, never this decoder.
mod identity;
pub(crate) mod package_format_1;

use crate::arguments::Arguments;
use crate::build::BuildDeclaration;
use crate::{AppSpec, Result};

pub use pop_contracts::StateContractId;
pub use pop_contracts::app::*;

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
    capabilities: Vec<Capability>,
    spec: AppSpec,
    state_contract_id: std::sync::OnceLock<StateContractId>,
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
    /// The declared capabilities this host knows, sorted and without repeats. A saved
    /// value this build does not know is never granted, so it is left out here.
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }
    /// Whether replicas share the data schema and theme defaults required for edits.
    /// UI resources and descriptions may differ without changing that contract.
    pub fn replica_compatible(&self, other: &Self) -> bool {
        self.state_contract_id() == other.state_contract_id()
    }

    /// Identity of the interpreted schema, slug, and unordered theme defaults.
    /// Presentation, commands, descriptions, and embedded resources are not part of it.
    pub fn state_contract_id(&self) -> StateContractId {
        *self.state_contract_id.get_or_init(|| identity::state_contract_id(self))
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
    /// CLI inspection retains authored geometry. Hosts use `window()` for resolved
    /// semantics and the normalized silhouette. Skin references are package URLs.
    pub fn page_window(&self) -> crate::build::WindowInput {
        use package_format_1::Window;
        match &self.stored.window {
            Window::Standard { width, height, fullscreenable, resizable, lock_aspect, background, shape } => {
                crate::build::WindowInput::Standard {
                    width: *width,
                    height: *height,
                    fullscreenable: *fullscreenable,
                    resizable: *resizable,
                    lock_aspect: *lock_aspect,
                    background: background.map(Into::into),
                    shape: shape.clone().map(Into::into),
                }
            }
            Window::Skin { width, height, fullscreenable, skin } => crate::build::WindowInput::Skin {
                width: *width,
                height: *height,
                fullscreenable: *fullscreenable,
                image: format!("/assets/{skin}"),
            },
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

/// The current authoring rules for a new app's metadata (`pop init`); never a saved file's.
pub fn validate_metadata(metadata: &AppMetadata) -> Result<()> {
    package_format_1::authoring_metadata(metadata)
}

// Freeze this module when format 1 releases. Pre-launch fixtures may start fresh.
const _: () = assert!(crate::PACKAGE_FORMAT == 1, "add a package reader and keep the released format's reader");
