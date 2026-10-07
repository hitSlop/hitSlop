//! Package-format-1 syntax and acceptance. Later host structs are not its decoder.
//!
//! Two rule sets live here. `checked` is what opening a format-1 file requires: the shape
//! the host can interpret safely, under this format's own limits. It is frozen when format
//! 1 releases and may only loosen. `authoring` is what `pack` and `init` require of a new
//! app; it may tighten at any time, because it never runs on a saved file.
use super::{
    AppDefinition, AppMetadata, Background, Category, CommandDefinition, CommandInput, ThemeInput, Views,
    WindowDefinition,
};
use crate::arguments::Arguments;
use crate::build::{BuildDeclaration, WindowInput};
use crate::wire::{APP_TEXT_BYTES, ASSET_FILE_BYTES, MANIFEST_BYTES, THEME_LIMIT, present_option};
use crate::{AppSpec, Code, Result, descriptor, encode, err, shape, theme};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::collections::HashSet;

/// Format-1 window geometry. Authoring limits (`crate::wire`) stay within these; a test
/// holds them there.
pub(crate) const WINDOW_MIN_WIDTH: u32 = 240;
pub(crate) const WINDOW_MIN_HEIGHT: u32 = 180;
pub(crate) const WINDOW_MAX: u32 = 4096;
pub(crate) const COMMANDS: usize = 64;
// Authoring may tighten; what it admits must always open.
const _: () = assert!(
    crate::wire::WINDOW_MIN_WIDTH >= WINDOW_MIN_WIDTH
        && crate::wire::WINDOW_MIN_HEIGHT >= WINDOW_MIN_HEIGHT
        && crate::wire::WINDOW_MAX <= WINDOW_MAX,
    "authoring window limits must stay within package format 1"
);
pub(crate) const SLUG_MIN: usize = 2;
pub(crate) const SLUG_MAX: usize = 64;
pub(crate) const TITLE_MAX: usize = 80;
pub(crate) const DESCRIPTION_MAX: usize = 240;
pub(crate) const AUTHOR_NAME_MAX: usize = 80;

// A descriptor plus the previous metadata, palette and command metadata budgets.
pub(crate) const DEFINITION_BYTES: usize = APP_TEXT_BYTES + MANIFEST_BYTES + THEME_LIMIT + ASSET_FILE_BYTES;

/// The stored `definition_json` of format 1. These structs are this format's alone: the
/// host model (`super`) translates from them and may change without changing them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Definition {
    pub(super) window: Window,
    pub(super) document: Box<RawValue>,
    theme: Vec<Token>,
    commands: Vec<Command>,
    views: StoredViews,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Token {
    token: String,
    color: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    name: String,
    description: String,
    args: Value,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredViews {
    export: bool,
    icon: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum StoredBackground {
    Transparent,
    Glass,
}
impl From<StoredBackground> for Background {
    fn from(value: StoredBackground) -> Self {
        match value {
            StoredBackground::Transparent => Self::Transparent,
            StoredBackground::Glass => Self::Glass,
        }
    }
}
impl From<Background> for StoredBackground {
    fn from(value: Background) -> Self {
        match value {
            Background::Transparent => Self::Transparent,
            Background::Glass => Self::Glass,
        }
    }
}
/// The `category_primary` and `category_secondary` spellings of format 1.
const CATEGORY_COLUMNS: [(Category, &str); 13] = [
    (Category::Productivity, "productivity"),
    (Category::Utilities, "utilities"),
    (Category::Finance, "finance"),
    (Category::Media, "media"),
    (Category::Games, "games"),
    (Category::DeveloperTools, "developer-tools"),
    (Category::Education, "education"),
    (Category::Business, "business"),
    (Category::Personal, "personal"),
    (Category::Health, "health"),
    (Category::Creative, "creative"),
    (Category::Music, "music"),
    (Category::Other, "other"),
];
pub(crate) fn category_column(category: Category) -> &'static str {
    CATEGORY_COLUMNS.iter().find(|(c, _)| *c == category).map(|(_, name)| *name).expect("every category has a column")
}
pub(crate) fn column_category(name: &str) -> Result<Category> {
    CATEGORY_COLUMNS
        .iter()
        .find(|(_, column)| *column == name)
        .map(|(category, _)| *category)
        .ok_or_else(|| err(Code::InvalidRequest, "Unknown category"))
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
        background: Option<StoredBackground>,
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
    authoring(&input.metadata, &input.window, &input.commands)?;
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
                background: background.map(Into::into),
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
        theme: input.theme.iter().map(|t| Token { token: t.token.clone(), color: t.color.clone() }).collect(),
        commands: input
            .commands
            .iter()
            .map(|c| Command { name: c.name.clone(), description: c.description.clone(), args: c.args.clone() })
            .collect(),
        views: StoredViews { export: input.views.export, icon: input.views.icon },
    };
    checked(input.metadata.clone(), definition)
}

/// What opening requires. Display text is bounded, not judged: a saved title, URL or
/// description is shown as it is.
fn checked(metadata_value: AppMetadata, stored: Definition) -> Result<AppDefinition> {
    stored_metadata(&metadata_value)?;
    bound(encode(&stored).len(), DEFINITION_BYTES, "definition")?;
    bound(encode(&stored.window).len(), MANIFEST_BYTES, "window")?;
    bound(encode(&stored.theme).len(), THEME_LIMIT, "theme")?;
    bound(encode(&stored.commands).len(), ASSET_FILE_BYTES, "commands")?;
    let window = window(&stored.window).map_err(|e| e.at("window"))?;
    let node = descriptor::descriptor(stored.document.get()).map_err(|e| e.at("document"))?;
    let tokens: Vec<_> = stored.theme.iter().map(|t| (t.token.clone(), t.color.clone())).collect();
    theme::validate_tokens(&tokens).map_err(|e| e.at("theme"))?;
    let mut names = HashSet::new();
    if stored.commands.len() > COMMANDS {
        return Err(err(Code::TooLarge, format!("An app supports at most {COMMANDS} commands")).at("commands"));
    }
    let mut commands = vec![];
    for (index, command) in stored.commands.iter().enumerate() {
        let check = || -> Result<CommandDefinition> {
            // The name every caller sends: the socket, engine and page grammar.
            let name = &command.name;
            if !crate::wire::engine::valid_command_name(name) {
                return Err(err(
                    Code::InvalidRequest,
                    "Command names start with a lowercase letter and contain at most 80 letters or digits",
                )
                .at("name"));
            }
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
    let theme = stored.theme.iter().map(|t| ThemeInput { token: t.token.clone(), color: t.color.clone() }).collect();
    let views = Views { export: stored.views.export, icon: stored.views.icon };
    Ok(AppDefinition { metadata: metadata_value, stored, window, theme, views, commands, spec })
}

/// The stored metadata's shape: bounded, with the one or two different categories its
/// columns hold.
fn stored_metadata(value: &AppMetadata) -> Result<()> {
    bound(encode(value).len(), MANIFEST_BYTES, "metadata")?;
    if !(1..=2).contains(&value.categories.len())
        || (value.categories.len() == 2 && value.categories[0] == value.categories[1])
    {
        return Err(err(Code::InvalidRequest, "Choose one or two different categories").at("categories"));
    }
    Ok(())
}

/// The current authoring rules for a new app, on top of what opening requires.
pub(super) fn authoring(metadata: &AppMetadata, window: &WindowInput, commands: &[CommandInput]) -> Result<()> {
    authoring_metadata(metadata)?;
    let (width, height) = match window {
        WindowInput::Standard { width, height, .. } | WindowInput::Skin { width, height, .. } => (*width, *height),
    };
    use crate::wire::{WINDOW_MAX as MAX, WINDOW_MIN_HEIGHT as MIN_HEIGHT, WINDOW_MIN_WIDTH as MIN_WIDTH};
    for (name, n, min) in [("width", width, f64::from(MIN_WIDTH)), ("height", height, f64::from(MIN_HEIGHT))] {
        if !(min..=f64::from(MAX)).contains(&n) {
            return Err(err(Code::InvalidRequest, format!("Must be an integer from {min} to {MAX}"))
                .at(name)
                .at("window"));
        }
    }
    for (index, command) in commands.iter().enumerate() {
        text(&command.description, 1, 500).map_err(|e| e.at("description").at(index).at("commands"))?;
    }
    Ok(())
}

pub(super) fn authoring_metadata(value: &AppMetadata) -> Result<()> {
    stored_metadata(value)?;
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
            background: background.map(Into::into),
            shape: shape::normalize(
                geometry.clone().unwrap_or_else(|| shape::Shape::Radius(crate::wire::DEFAULT_WINDOW_RADIUS.into())),
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

#[cfg(test)]
mod tests {
    use super::*;
    /// A new host category needs its format-1 column spelling before any pack writes it.
    #[test]
    fn every_category_has_one_stored_spelling() {
        for &category in Category::ALL {
            assert_eq!(column_category(category_column(category)).unwrap(), category);
        }
        assert_eq!(CATEGORY_COLUMNS.len(), Category::ALL.len());
    }
}
