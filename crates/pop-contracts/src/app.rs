use crate::{Code, Result, err, shape, wire::present_option};
use serde::{Deserialize, Serialize};
use serde_json::Value;
// The host and authoring model. A format module translates its stored structs into these,
// so they may grow or change without changing how a released file decodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum Category {
    Popular,
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
        Self::Popular,
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

/// A device or host service an app declares it uses. The host admits a request only for a
/// declared capability; the OS or browser permission still applies. Authoring refuses any
/// value not listed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum Capability {
    Camera,
    Microphone,
    Speech,
}
impl Capability {
    pub const ALL: &[Self] = &Self::all();

    const fn all() -> [Self; 3] {
        let mut all = [Self::Camera; 3];
        let mut next = Some(Self::Camera);
        let mut index = 0;
        while let Some(capability) = next {
            all[index] = capability;
            index += 1;
            next = match capability {
                Self::Camera => Some(Self::Microphone),
                Self::Microphone => Some(Self::Speech),
                Self::Speech => None,
            };
        }
        assert!(index == all.len());
        all
    }
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
    pub preview: bool,
    pub export: bool,
    pub icon: bool,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub widget: Option<WidgetOptions>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "app.generated.ts"))]
pub struct WidgetOptions {
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub refresh_minutes: Option<u32>,
}

/// Resolved host window semantics, independent of the stored definition's decoding rules.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct WindowDefinition {
    pub width: u32,
    pub height: u32,
    pub fullscreenable: bool,
    pub frame: WindowFrame,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum WindowFrame {
    Standard { frame: StandardFrame },
    Skin { skin: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub struct StandardFrame {
    /// Whether fullscreen fits the authored composition instead of reflowing it.
    pub fullscreen_fit: bool,
    pub resizable: bool,
    pub lock_aspect: bool,
    pub background: Option<Background>,
    /// CSS radius token exposed by the page shell; paths expose zero.
    pub radius: String,
    pub shape: shape::Silhouette,
}

/// Permanent marker checks, before interpreting any current-format fields. i128
/// accommodates unsigned wire markers and signed SQLite values without truncation.
pub fn requirements(package_format: i128, runtime_abi: i128) -> Result<()> {
    let markers = [
        ("package format", package_format, crate::wire::PACKAGE_FORMAT),
        ("runtime ABI", runtime_abi, crate::wire::RUNTIME_ABI),
    ];
    if let Some((name, level, supported)) = markers.iter().find(|(_, level, supported)| *level > *supported as i128) {
        return Err(err(Code::RequiresUpdate, format!("This pop needs {name} {level}; this pop supports {supported}")));
    }
    if let Some((name, ..)) = markers.iter().find(|(_, level, _)| *level < 1) {
        return Err(err(Code::InvalidRequest, format!("Invalid {name}")));
    }
    Ok(())
}
