//! The templates hosts list: the files in the template folders (`places`) that open as a
//! template named for its slug. The app's catalog, the engine's `templates` and
//! `create --from SLUG` share this rule.

use super::places::TemplateSource;
use super::{Kind, Summary, summary};
use crate::app::Category;
use crate::error::{Result, invalid};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

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
/// A template folder hosts list, whether or not it exists yet: `slop register` builds into
/// the installed one.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "engine.generated.ts"))]
pub struct Folder {
    pub source: TemplateSource,
    pub path: PathBuf,
}
/// The template folders, the templates listed from them, and why each other `.slop` file
/// in them was left out.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "engine.generated.ts"))]
pub struct Catalog {
    pub folders: Vec<Folder>,
    pub templates: Vec<Template>,
    pub issues: Vec<String>,
}

fn template(path: &Path) -> Result<Summary> {
    let summary = summary(path)?;
    if summary.kind != Kind::Template {
        return Err(invalid("An installed template holds no document"));
    }
    if path.file_name() != Some(OsStr::new(&format!("{}.slop", summary.metadata.slug))) {
        return Err(invalid("An installed template's file name must be its slug"));
    }
    Ok(summary)
}
/// Display summary, intentionally without accepting the recursive app or its resources.
pub fn open_template(path: &Path) -> Result<Summary> {
    template(path)
}

/// The templates in `roots`, in order, each folder's sorted by file name. Hidden files and
/// files without the `.slop` extension are skipped; a folder that does not exist lists
/// nothing, and listing never creates one.
pub fn list_templates(roots: &[(TemplateSource, PathBuf)]) -> Catalog {
    let folders = roots.iter().map(|(source, path)| Folder { source: *source, path: path.clone() }).collect();
    let mut catalog = Catalog { folders, ..Catalog::default() };
    for (source, root) in roots {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                catalog.issues.push(format!("{}: {error}", root.display()));
                continue;
            }
        };
        let root = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        let mut names: Vec<_> = entries
            .filter_map(|entry| Some(entry.ok()?.file_name()))
            .filter(|name| {
                !name.as_encoded_bytes().starts_with(b".")
                    && Path::new(name).extension().is_some_and(|ext| ext == "slop")
            })
            .collect();
        names.sort();
        for name in names {
            let path = root.join(&name);
            match template(&path) {
                Ok(summary) => {
                    let m = summary.metadata;
                    catalog.templates.push(Template {
                        slug: m.slug,
                        title: m.title,
                        description: m.description,
                        categories: m.categories,
                        source: *source,
                        path,
                    })
                }
                Err(error) => catalog.issues.push(format!("{}: {error}", name.to_string_lossy())),
            }
        }
    }
    catalog
}

/// The listed template named `slug` in `roots`. An installed template shadows a bundled
/// starter with its slug: an author who registers one replaces what the app ships.
pub fn find_template(slug: &str, roots: &[(TemplateSource, PathBuf)]) -> Result<PathBuf> {
    let name = format!("{slug}.slop");
    if Path::new(&name).file_name() == Some(OsStr::new(&name)) {
        for source in [TemplateSource::Installed, TemplateSource::Bundled] {
            for (_, root) in roots.iter().filter(|(listed, _)| *listed == source) {
                let path = root.join(&name);
                if fs::symlink_metadata(&path).is_ok() {
                    return template(&path).map(|_| path);
                }
            }
        }
    }
    Err(invalid(format!("No installed or bundled template is named {slug}")))
}

/// The template `from` names for a new document: a listed template's slug when `from` is
/// a bare name (no folder and no extension), otherwise the path to its file.
pub fn template_source(from: &str) -> Result<PathBuf> {
    let path = Path::new(from);
    if path.components().count() == 1 && path.extension().is_none() {
        return find_template(from, &super::template_roots());
    }
    Ok(path.to_owned())
}
