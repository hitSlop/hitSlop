//! Package-format-1 syntax and acceptance. Later host structs are not its decoder.
//!
//! Two rule sets live here. `checked` is what opening a format-1 file requires: the shape
//! the host can interpret safely, under this format's own limits. Released acceptance is
//! frozen by behavior: acceptance vectors, the limits ratchet and corpus replay. It may only
//! loosen. `authoring` is what `pack` and `init` require of a new app; it may tighten at any
//! time, because it never runs on a saved file.
//!
//! The stored types are open readers: a member this format does not define is ignored, so a
//! later build can add an optional member without a new package format. What decides what a
//! value may be stays exact: the descriptor and the commands' arguments.
use super::{
    AppDefinition, AppMetadata, Background, Capability, Category, CommandDefinition, CommandInput, StandardFrame,
    ThemeInput, Views, WidgetOptions, WindowDefinition, WindowFrame,
};
use crate::arguments::Arguments;
use crate::build::{BuildDeclaration, WindowInput};
use crate::wire::{APP_TEXT_BYTES, ASSET_FILE_BYTES, MANIFEST_BYTES, THEME_LIMIT, present_option};
use crate::{AppSpec, Code, Result, descriptor, encode, err, shape, theme};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::collections::HashSet;

pub(crate) use crate::wire::{
    COMMANDS, DEFINITION_BYTES, OPEN_WINDOW_MAX as WINDOW_MAX, OPEN_WINDOW_MIN_HEIGHT as WINDOW_MIN_HEIGHT,
    OPEN_WINDOW_MIN_WIDTH as WINDOW_MIN_WIDTH,
};
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

/// The stored `definition_json` of format 1. These structs are this format's alone: the
/// host model (`super`) translates from them and may change without changing them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Definition {
    pub(super) window: Window,
    pub(super) document: Box<RawValue>,
    theme: Vec<Token>,
    commands: Vec<Command>,
    views: StoredViews,
    /// Stored spellings (`capability_name`), sorted and unique when packed. Absent means
    /// none, so an app without capabilities stores exactly what it did before they existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Token {
    token: String,
    color: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Command {
    name: String,
    description: String,
    args: Value,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct StoredViews {
    #[serde(default)]
    preview: bool,
    export: bool,
    icon: bool,
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    widget: Option<StoredWidget>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredWidget {
    #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    refresh_minutes: Option<u32>,
}
impl From<StoredWidget> for WidgetOptions {
    fn from(value: StoredWidget) -> Self {
        Self { refresh_minutes: value.refresh_minutes }
    }
}
impl From<WidgetOptions> for StoredWidget {
    fn from(value: WidgetOptions) -> Self {
        Self { refresh_minutes: value.refresh_minutes }
    }
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
/// A window silhouette as format 1 stores it: a CSS radius, or bounded SVG path data.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum StoredShape {
    Radius(String),
    Path(StoredPath),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct StoredPath {
    path: String,
    #[serde(rename = "viewBox", default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    view_box: Option<[f64; 2]>,
    #[serde(
        rename = "fillRule",
        default,
        deserialize_with = "present_option",
        skip_serializing_if = "Option::is_none"
    )]
    fill_rule: Option<StoredFillRule>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum StoredFillRule {
    Nonzero,
    Evenodd,
}
impl From<StoredShape> for shape::Shape {
    fn from(value: StoredShape) -> Self {
        match value {
            StoredShape::Radius(radius) => Self::Radius(radius),
            StoredShape::Path(path) => Self::Path(shape::PathShape {
                path: path.path,
                view_box: path.view_box,
                fill_rule: path.fill_rule.map(Into::into),
            }),
        }
    }
}
impl From<shape::Shape> for StoredShape {
    fn from(value: shape::Shape) -> Self {
        match value {
            shape::Shape::Radius(radius) => Self::Radius(radius),
            shape::Shape::Path(path) => Self::Path(StoredPath {
                path: path.path,
                view_box: path.view_box,
                fill_rule: path.fill_rule.map(Into::into),
            }),
        }
    }
}
impl From<StoredFillRule> for shape::FillRule {
    fn from(value: StoredFillRule) -> Self {
        match value {
            StoredFillRule::Nonzero => Self::Nonzero,
            StoredFillRule::Evenodd => Self::Evenodd,
        }
    }
}
impl From<shape::FillRule> for StoredFillRule {
    fn from(value: shape::FillRule) -> Self {
        match value {
            shape::FillRule::Nonzero => Self::Nonzero,
            shape::FillRule::Evenodd => Self::Evenodd,
        }
    }
}
/// The `category_primary` and `category_secondary` spellings of format 1.
const CATEGORY_COLUMNS: [(Category, &str); 14] = [
    (Category::Popular, "popular"),
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
/// The `capabilities` spellings of format 1. Opening ignores any other string: it can only
/// name a capability a later build added, which this host does not implement and so never
/// grants. That lets a later build add a capability without a new package format; one that
/// needs new app-facing API raises `runtimeABI` instead. Authoring refuses unknown values.
fn capability_name(capability: Capability) -> &'static str {
    match capability {
        Capability::Camera => "camera",
        Capability::Microphone => "microphone",
        Capability::Speech => "speech",
    }
}
fn stored_capability(name: &str) -> Option<Capability> {
    Capability::ALL.iter().copied().find(|&capability| capability_name(capability) == name)
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn category_column(category: Category) -> &'static str {
    CATEGORY_COLUMNS.iter().find(|(c, _)| *c == category).map(|(_, name)| *name).expect("every category has a column")
}
/// The category a stored spelling names. A spelling this build does not know can only come
/// from a later build that added a category, so it reads as `Other` and the file still lists
/// and opens. Authoring parses strictly (`Category::parse`).
fn column_category(name: &str) -> Category {
    CATEGORY_COLUMNS.iter().find(|(_, column)| *column == name).map_or(Category::Other, |(category, _)| *category)
}
/// The categories the two columns hold: one or two different ones, as `stored_metadata`
/// requires. Spellings that read as one category are one.
pub(crate) fn column_categories(primary: &str, secondary: Option<&str>) -> Vec<Category> {
    let mut categories = vec![column_category(primary)];
    if let Some(secondary) = secondary.map(column_category)
        && !categories.contains(&secondary)
    {
        categories.push(secondary);
    }
    categories
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub(super) enum Window {
    Standard {
        width: f64,
        height: f64,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        fullscreenable: Option<bool>,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        resizable: Option<bool>,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        lock_aspect: Option<bool>,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        background: Option<StoredBackground>,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        shape: Option<StoredShape>,
    },
    Skin {
        width: f64,
        height: f64,
        #[serde(default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
        fullscreenable: Option<bool>,
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
    authoring_widget(input.views.widget)?;
    if !input.views.preview || !input.views.icon {
        return Err(err(Code::InvalidRequest, "Authoring requires Preview.svelte and Icon.svelte").at("views"));
    }
    let window = match &input.window {
        WindowInput::Standard { width, height, fullscreenable, resizable, lock_aspect, background, shape } => {
            if skin.is_some() {
                return Err(err(Code::InvalidRequest, "A standard window has no skin").at("window"));
            }
            Window::Standard {
                width: *width,
                height: *height,
                fullscreenable: *fullscreenable,
                resizable: *resizable,
                lock_aspect: *lock_aspect,
                background: background.map(Into::into),
                shape: shape.clone().map(Into::into),
            }
        }
        WindowInput::Skin { width, height, fullscreenable, image } => {
            let key = skin.ok_or_else(|| err(Code::InvalidRequest, "Missing imported skin").at("window"))?;
            if image != &format!("/assets/{key}") {
                return Err(err(Code::InvalidRequest, "The skin image must resolve to its bundled asset")
                    .at("image")
                    .at("window"));
            }
            Window::Skin { width: *width, height: *height, fullscreenable: *fullscreenable, skin: key.into() }
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
        views: StoredViews {
            preview: input.views.preview,
            export: input.views.export,
            icon: input.views.icon,
            widget: input.views.widget.map(Into::into),
        },
        capabilities: {
            let mut declared = input.capabilities.clone();
            declared.sort();
            declared.dedup();
            declared.into_iter().map(|c| capability_name(c).into()).collect()
        },
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
    bound(encode(&stored.capabilities).len(), MANIFEST_BYTES, "capabilities")?;
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
            // The name every caller sends: the socket and engine grammar.
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
    let views = Views {
        preview: stored.views.preview,
        export: stored.views.export,
        icon: stored.views.icon,
        widget: stored.views.widget.map(Into::into),
    };

    let mut capabilities: Vec<_> = stored.capabilities.iter().filter_map(|name| stored_capability(name)).collect();
    capabilities.sort();
    capabilities.dedup();
    Ok(AppDefinition { metadata: metadata_value, stored, window, theme, views, commands, capabilities, spec })
}

/// The stored metadata's shape: bounded, with the one or two different categories its
/// columns hold.
fn stored_metadata(value: &AppMetadata) -> Result<()> {
    bound(encode(value).len(), MANIFEST_BYTES, "metadata")?;
    if !(crate::wire::CATEGORIES_MIN..=crate::wire::CATEGORIES_MAX).contains(&value.categories.len())
        || (value.categories.len() == crate::wire::CATEGORIES_MAX && value.categories[0] == value.categories[1])
    {
        return Err(err(Code::InvalidRequest, "Choose one or two different categories").at("categories"));
    }
    Ok(())
}

/// The current authoring rules for a new app, on top of what opening requires.
fn authoring_widget(widget: Option<super::WidgetOptions>) -> Result<()> {
    if let Some(minutes) = widget.and_then(|widget| widget.refresh_minutes)
        && !(crate::widgets::MIN_REFRESH_MINUTES..=crate::widgets::MAX_REFRESH_MINUTES).contains(&minutes)
    {
        return Err(err(
            Code::InvalidRequest,
            format!(
                "Widget refreshMinutes must be between {} and {}",
                crate::widgets::MIN_REFRESH_MINUTES,
                crate::widgets::MAX_REFRESH_MINUTES
            ),
        )
        .at("refreshMinutes")
        .at("widget")
        .at("views"));
    }
    Ok(())
}

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
    let (width, height, fullscreenable) = match value {
        Window::Standard { width, height, fullscreenable, .. } | Window::Skin { width, height, fullscreenable, .. } => {
            (*width, *height, fullscreenable.unwrap_or(false))
        }
    };
    for (name, n, min) in
        [("width", width, f64::from(WINDOW_MIN_WIDTH)), ("height", height, f64::from(WINDOW_MIN_HEIGHT))]
    {
        if !n.is_finite() || n.fract() != 0.0 || !(min..=f64::from(WINDOW_MAX)).contains(&n) {
            return Err(err(Code::InvalidRequest, format!("Must be an integer from {min} to {WINDOW_MAX}")).at(name));
        }
    }
    let frame = match value {
        Window::Standard { resizable, lock_aspect, background, shape: geometry, .. } => {
            let geometry: Option<shape::Shape> = geometry.clone().map(Into::into);
            let radius = match &geometry {
                Some(shape::Shape::Radius(radius)) => radius.clone(),
                Some(shape::Shape::Path(_)) => "0".into(),
                None => crate::wire::DEFAULT_WINDOW_RADIUS.into(),
            };
            WindowFrame::Standard {
                frame: StandardFrame {
                    fullscreen_fit: !resizable.unwrap_or(true) || lock_aspect.unwrap_or(false) || geometry.is_some(),
                    resizable: resizable.unwrap_or(true),
                    lock_aspect: lock_aspect.unwrap_or(false),
                    background: background.map(Into::into),
                    radius,
                    shape: shape::normalize(
                        geometry.unwrap_or_else(|| shape::Shape::Radius(crate::wire::DEFAULT_WINDOW_RADIUS.into())),
                        width,
                        height,
                    )
                    .map_err(|e| e.at("shape"))?,
                },
            }
        }
        Window::Skin { skin, .. } => {
            if crate::media::asset_key(skin).is_none_or(|kind| kind.media_type != "image/png") {
                return Err(err(Code::InvalidRequest, "Skin must name a content-addressed PNG asset").at("skin"));
            }
            WindowFrame::Skin { skin: skin.clone() }
        }
    };
    Ok(WindowDefinition { width: width as u32, height: height as u32, fullscreenable, frame })
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
            assert_eq!(column_category(category_column(category)), category);
        }
        assert_eq!(CATEGORY_COLUMNS.len(), Category::ALL.len());
    }
    /// A capability reads back from the spelling packing stores, and `ALL` lists every one.
    #[test]
    fn every_capability_has_one_stored_spelling() {
        for &capability in Capability::ALL {
            let name = capability_name(capability);
            assert_eq!(stored_capability(name), Some(capability));
            assert_eq!(serde_json::to_value(capability).unwrap(), name, "the authored and stored spellings agree");
        }
    }
}
