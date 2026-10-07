//! Package-format-1 syntax and acceptance. Later host structs are not its decoder.
use super::{AppDefinition, CommandDefinition, WindowDefinition};
use crate::arguments::Arguments;
use crate::build::{BuildDeclaration, WindowInput};
use crate::wire::{
    APP_TEXT_BYTES, ASSET_FILE_BYTES, DEFAULT_WINDOW_RADIUS, MANIFEST_BYTES, THEME_LIMIT, WINDOW_MAX,
    WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH, present_option,
};
use crate::{AppSpec, Code, Result, descriptor, encode, err, shape, theme};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::collections::HashSet;

// Fixed metadata types belong to this format. Build input reuses them; later
// formats must not decode released files with a newly extended input DTO.
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
pub(crate) const SLUG_MIN: usize = 2;
pub(crate) const SLUG_MAX: usize = 64;
pub(crate) const TITLE_MAX: usize = 80;
pub(crate) const DESCRIPTION_MAX: usize = 240;
pub(crate) const AUTHOR_NAME_MAX: usize = 80;

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
// A descriptor plus the previous metadata, palette and command metadata budgets.
pub(crate) const DEFINITION_BYTES: usize = APP_TEXT_BYTES + MANIFEST_BYTES + THEME_LIMIT + ASSET_FILE_BYTES;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Definition {
    pub(super) window: Window,
    pub(super) document: Box<RawValue>,
    pub(super) theme: Vec<ThemeInput>,
    commands: Vec<CommandInput>,
    pub(super) views: Views,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub(super) enum Window {
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
        shape: Option<shape::Shape>,
    },
    Skin {
        width: f64,
        height: f64,
        skin: String,
    },
}

pub(super) fn decode(metadata_json: &str, definition_json: &str) -> Result<AppDefinition> {
    bound(metadata_json.len(), MANIFEST_BYTES, "metadata")?;
    bound(definition_json.len(), DEFINITION_BYTES, "definition")?;
    let metadata = serde_json::from_str(metadata_json).map_err(|e| err(Code::InvalidRequest, e))?;
    let definition = serde_json::from_str(definition_json).map_err(|e| err(Code::InvalidRequest, e))?;
    checked(metadata, definition)
}

pub(super) fn from_declaration(input: &BuildDeclaration, skin: Option<&str>) -> Result<AppDefinition> {
    let window = match &input.window {
        WindowInput::Standard { width, height, resizable, lock_aspect, background, shape } => {
            if skin.is_some() {
                return Err(err(Code::InvalidRequest, "A standard window has no skin").at("window"));
            }
            Window::Standard {
                width: *width,
                height: *height,
                resizable: *resizable,
                lock_aspect: *lock_aspect,
                background: *background,
                shape: shape.clone(),
            }
        }
        WindowInput::Skin { width, height, image } => {
            let key = skin.ok_or_else(|| err(Code::InvalidRequest, "Missing imported skin").at("window"))?;
            if image != &format!("/assets/{key}") {
                return Err(err(Code::InvalidRequest, "The skin image must resolve to its bundled asset")
                    .at("image")
                    .at("window"));
            }
            Window::Skin { width: *width, height: *height, skin: key.into() }
        }
    };
    bound(encode(&input.initial).len(), APP_TEXT_BYTES, "initial")?;
    let definition = Definition {
        window,
        document: RawValue::from_string(encode(&input.document)).expect("JSON value"),
        theme: input.theme.clone(),
        commands: input.commands.clone(),
        views: input.views,
    };
    checked(input.metadata.clone(), definition)
}

fn checked(metadata_value: AppMetadata, stored: Definition) -> Result<AppDefinition> {
    metadata(&metadata_value)?;
    bound(encode(&stored).len(), DEFINITION_BYTES, "definition")?;
    bound(encode(&stored.window).len(), MANIFEST_BYTES, "window")?;
    bound(encode(&stored.theme).len(), THEME_LIMIT, "theme")?;
    bound(encode(&stored.commands).len(), ASSET_FILE_BYTES, "commands")?;
    let window = window(&stored.window).map_err(|e| e.at("window"))?;
    let node = descriptor::descriptor(stored.document.get()).map_err(|e| e.at("document"))?;
    let tokens: Vec<_> = stored.theme.iter().map(|t| (t.token.clone(), t.color.clone())).collect();
    theme::validate_tokens(&tokens).map_err(|e| e.at("theme"))?;
    let mut names = HashSet::new();
    if stored.commands.len() > 64 {
        return Err(err(Code::TooLarge, "An app supports at most 64 commands").at("commands"));
    }
    let mut commands = vec![];
    for (index, command) in stored.commands.iter().enumerate() {
        let check = || -> Result<CommandDefinition> {
            let name = &command.name;
            if name.is_empty()
                || name.len() > 80
                || !name.as_bytes()[0].is_ascii_lowercase()
                || !name.bytes().all(|b| b.is_ascii_alphanumeric())
            {
                return Err(err(
                    Code::InvalidRequest,
                    "Command names start with a lowercase letter and contain at most 80 letters or digits",
                )
                .at("name"));
            }
            text(&command.description, 1, 500).map_err(|e| e.at("description"))?;
            let args = Arguments::parse(&encode(&command.args)).map_err(|e| e.at("args"))?;
            Ok(CommandDefinition { name: name.clone(), description: command.description.clone(), args })
        };
        let command = check().map_err(|e| e.at(index).at("commands"))?;
        if !names.insert(command.name.clone()) {
            return Err(err(Code::InvalidRequest, "Repeated command name").at("name").at(index).at("commands"));
        }
        commands.push(command);
    }
    let spec = AppSpec::of(node, &metadata_value.slug, tokens);
    Ok(AppDefinition { metadata: metadata_value, stored, window, commands, spec })
}

pub(super) fn metadata(value: &AppMetadata) -> Result<()> {
    bound(encode(value).len(), MANIFEST_BYTES, "metadata")?;
    text(&value.slug, SLUG_MIN, SLUG_MAX).map_err(|e| e.at("slug"))?;
    if !value
        .slug
        .split('-')
        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
    {
        return Err(err(Code::InvalidRequest, "Use lowercase letters, digits and single hyphens").at("slug"));
    }
    text(&value.title, 1, TITLE_MAX).map_err(|e| e.at("title"))?;
    text(&value.description, 1, DESCRIPTION_MAX).map_err(|e| e.at("description"))?;
    text(&value.author.name, 1, AUTHOR_NAME_MAX).map_err(|e| e.at("name").at("author"))?;
    if value.author.name.trim().is_empty() {
        return Err(err(Code::InvalidRequest, "Author name must contain non-whitespace text").at("name").at("author"));
    }
    if let Some(url) = &value.author.url {
        let valid = url.chars().count() <= 2048
            && fluent_uri::Uri::parse(url.as_str()).is_ok_and(|uri| {
                matches!(uri.scheme().as_str(), "http" | "https")
                    && uri.authority().is_some_and(|a| !a.host().is_empty())
            });
        if !valid {
            return Err(err(
                Code::InvalidRequest,
                "Author URL must be an http or https URI with a host, at most 2048 characters",
            )
            .at("url")
            .at("author"));
        }
    }
    if !(1..=2).contains(&value.categories.len())
        || (value.categories.len() == 2 && value.categories[0] == value.categories[1])
    {
        return Err(err(Code::InvalidRequest, "Choose one or two different categories").at("categories"));
    }
    Ok(())
}

fn window(value: &Window) -> Result<WindowDefinition> {
    let (width, height) = match value {
        Window::Standard { width, height, .. } | Window::Skin { width, height, .. } => (*width, *height),
    };
    for (name, n, min) in
        [("width", width, f64::from(WINDOW_MIN_WIDTH)), ("height", height, f64::from(WINDOW_MIN_HEIGHT))]
    {
        if !n.is_finite() || n.fract() != 0.0 || !(min..=f64::from(WINDOW_MAX)).contains(&n) {
            return Err(err(Code::InvalidRequest, format!("Must be an integer from {min} to {WINDOW_MAX}")).at(name));
        }
    }
    Ok(match value {
        Window::Standard { resizable, lock_aspect, background, shape: geometry, .. } => WindowDefinition::Standard {
            width: width as u32,
            height: height as u32,
            resizable: resizable.unwrap_or(true),
            lock_aspect: lock_aspect.unwrap_or(false),
            background: *background,
            shape: shape::normalize(
                geometry.clone().unwrap_or_else(|| shape::Shape::Radius(DEFAULT_WINDOW_RADIUS.into())),
                width,
                height,
            )
            .map_err(|e| e.at("shape"))?,
        },
        Window::Skin { skin, .. } => {
            if crate::media::asset_key(skin).is_none_or(|kind| kind.media_type != "image/png") {
                return Err(err(Code::InvalidRequest, "Skin must name a content-addressed PNG asset").at("skin"));
            }
            WindowDefinition::Skin { width: width as u32, height: height as u32, skin: skin.clone() }
        }
    })
}

fn text(value: &str, min: usize, max: usize) -> Result<()> {
    if !(min..=max).contains(&value.chars().count()) {
        return Err(err(Code::InvalidRequest, format!("Must contain {min}–{max} characters")));
    }
    Ok(())
}

fn bound(bytes: usize, max: usize, field: &str) -> Result<()> {
    if bytes > max { Err(err(Code::TooLarge, format!("{field} exceeds its byte limit")).at(field)) } else { Ok(()) }
}
