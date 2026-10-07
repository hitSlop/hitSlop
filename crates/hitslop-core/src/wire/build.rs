//! Untrusted output of the two Vite builds. This is input to app acceptance, not an
//! accepted app or a stored format. Descriptors and initial values are checked by the core.
use super::present_option;
use crate::shape::Shape;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::app::{AppMetadata, Author, Background, Category, CommandInput, ThemeInput, Views};

/// A source declaration: the skin image is an imported URL. Acceptance resolves it
/// to a resource key; the stored window never keeps a source URL.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "app.generated.ts"))]
pub enum WindowInput {
    Standard {
        width: f64,
        height: f64,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        resizable: Option<bool>,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        lock_aspect: Option<bool>,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        background: Option<Background>,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        shape: Option<Shape>,
    },
    Skin {
        width: f64,
        height: f64,
        image: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct BuildDeclaration {
    pub metadata: AppMetadata,
    pub window: WindowInput,
    pub theme: Vec<ThemeInput>,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub document: Value,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub initial: Value,
    pub commands: Vec<CommandInput>,
    pub views: Views,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "app.generated.ts"))]
pub struct BuildRoles {
    pub ui: String,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub commands: Option<String>,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub skin: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum ResourceKind {
    App,
    Command,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct ResourceInput {
    pub kind: ResourceKind,
    pub key: String,
    pub media_type: String,
    /// A regular file beneath the private staging root, checked before reading.
    pub path: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "app.generated.ts"))]
pub struct ArtworkInput {
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct BuildInput {
    #[serde(rename = "packageFormat")]
    pub package_format: u64,
    #[serde(rename = "runtimeABI")]
    pub runtime_abi: u64,
    pub declaration: BuildDeclaration,
    pub roles: BuildRoles,
    pub resources: Vec<ResourceInput>,
    pub artwork: ArtworkInput,
}

impl BuildInput {
    /// Future payloads stay opaque until the permanent requirements are checked.
    pub fn decode(input: &str) -> crate::Result<Self> {
        let max = 2 * super::APP_TEXT_BYTES
            + super::ASSET_FILE_BYTES
            + 2 * super::MANIFEST_BYTES
            + super::THEME_LIMIT
            + super::ASSET_COUNT * (2 * super::ASSET_PATH_BYTES + 128);
        if input.len() > max {
            return Err(crate::err(super::Code::TooLarge, "Build input exceeds its byte limit"));
        }
        #[derive(Deserialize)]
        struct Markers {
            #[serde(rename = "packageFormat")]
            package_format: u64,
            #[serde(rename = "runtimeABI")]
            runtime_abi: u64,
        }
        let markers: Markers = serde_json::from_str(input).map_err(|e| crate::err(super::Code::InvalidRequest, e))?;
        crate::app::requirements(markers.package_format.into(), markers.runtime_abi.into())?;
        serde_json::from_str(input).map_err(|e| crate::err(super::Code::InvalidRequest, e))
    }
}
