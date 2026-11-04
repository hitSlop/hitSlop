use crate::app::Category;
use std::path::PathBuf;
/// Where a listed template comes from: the starters bundled with the app, or the installed
/// templates folder that `pop register` builds into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "engine.generated.ts"))]
pub enum TemplateSource {
    Bundled,
    Installed,
}

/// A listed template: what its manifest says, its file, and the folder it was listed from.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "engine.generated.ts"))]
pub struct Template {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<Category>,
    pub source: TemplateSource,
    pub path: PathBuf,
}
/// A template folder hosts list, whether or not it exists yet: `pop register` builds into
/// the installed one.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "engine.generated.ts"))]
pub struct Folder {
    pub source: TemplateSource,
    pub path: PathBuf,
}
/// The template folders, the templates listed from them, and why each other `.pop` file
/// in them was left out.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "engine.generated.ts"))]
pub struct Catalog {
    pub folders: Vec<Folder>,
    pub templates: Vec<Template>,
    pub issues: Vec<String>,
}
