//! Where documents may live: never in iCloud Drive, and new ones never among the templates.

use crate::error::{Result, invalid};
use std::fs;
use std::path::{Path, PathBuf};

/// Where documents may not live, for this account and executable: iCloud Drive, whose
/// syncing documents do not support yet, and the installed and bundled templates.
struct Places {
    cloud: Option<PathBuf>,
    templates: Vec<PathBuf>,
}
impl Places {
    fn current() -> Self {
        let home = crate::registry::home();
        let mut templates: Vec<PathBuf> = home.map(|home| home.join(".hitslop/templates")).into_iter().collect();
        // A development app's catalog.
        templates.extend(std::env::var_os("HITSLOP_TEMPLATES_ROOT").filter(|root| !root.is_empty()).map(PathBuf::from));
        // The app's bundled starters, for the app itself (`Contents/MacOS`) and the tools
        // beside its helper (`Contents/Helpers`).
        let contents = std::env::current_exe()
            .and_then(fs::canonicalize)
            .ok()
            .and_then(|executable| Some(executable.parent()?.parent()?.to_owned()))
            .filter(|contents| {
                contents.file_name().is_some_and(|name| name.eq_ignore_ascii_case("Contents"))
                    && contents.parent().and_then(Path::extension).is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
            });
        templates.extend(contents.map(|contents| contents.join("Resources/StarterTemplates")));
        Self { cloud: home.map(|home| home.join("Library/Mobile Documents")), templates }
    }
}
/// `path` made absolute with its existing ancestors resolved: a destination need not exist
/// yet, and a link above it counts as where it leads.
fn real(path: &Path) -> PathBuf {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let mut missing = vec![];
    let mut ancestor = path.as_path();
    loop {
        if let Ok(found) = fs::canonicalize(ancestor) {
            return missing.iter().rev().fold(found, |found, name| found.join(name));
        }
        match (ancestor.file_name(), ancestor.parent()) {
            (Some(name), Some(parent)) => {
                missing.push(name.to_owned());
                ancestor = parent;
            }
            _ => return path,
        }
    }
}
/// Whether `path` lies inside `root` (or is it, when `inclusive`), comparing resolved
/// components without case, as the Mac's file system does.
fn within(path: &Path, root: &Path, inclusive: bool) -> bool {
    let (path, root) = (real(path), real(root));
    let (path, root): (Vec<_>, Vec<_>) = (path.components().collect(), root.components().collect());
    let lower = |part: &std::path::Component| part.as_os_str().to_string_lossy().to_lowercase();
    (path.len() > root.len() || inclusive && path.len() == root.len())
        && root.iter().zip(&path).all(|(root, part)| lower(root) == lower(part))
}
/// Where a document may open for writing: a `.slop` file outside iCloud Drive.
fn check_location(path: &Path, places: &Places) -> Result<()> {
    if path.extension().is_none_or(|ext| ext != "slop") {
        return Err(invalid("A document's name must end in .slop"));
    }
    if places.cloud.as_deref().is_some_and(|cloud| within(path, cloud, true)) {
        return Err(invalid("iCloud document locations are not supported. Move the document to a local folder."));
    }
    Ok(())
}
/// Where a new document may go: where a document may open, outside the templates.
fn check_destination(path: &Path, places: &Places) -> Result<()> {
    check_location(path, places)?;
    if places.templates.iter().any(|root| within(path, root, false)) {
        return Err(invalid("A document cannot be created among installed templates"));
    }
    Ok(())
}
/// Refuses a document opened for writing where documents may not live. The app, its
/// helper and the engine all open documents through the store, so they share the rule.
pub(crate) fn document_location(path: &Path) -> Result<()> {
    check_location(path, &Places::current())
}
/// Refuses a new document (created or copied) where documents may not go.
pub(crate) fn document_destination(path: &Path) -> Result<()> {
    check_destination(path, &Places::current())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Places stand in for the account's: a test never names the real iCloud Drive.
    #[test]
    fn documents_stay_out_of_icloud_and_new_ones_out_of_the_templates() {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let (cloud, templates) = (root.join("cloud"), root.join("templates"));
        fs::create_dir_all(&cloud).unwrap();
        fs::create_dir_all(&templates).unwrap();
        std::os::unix::fs::symlink(&cloud, root.join("alias")).unwrap();
        let places = Places { cloud: Some(cloud.clone()), templates: vec![templates.clone()] };
        for (path, location, destination) in [
            (root.join("notes"), false, false),
            (root.join("Notes.SLOP"), false, false),
            (cloud.clone(), false, false),
            (cloud.join("new/Doc.slop"), false, false),
            (root.join("alias/Doc.slop"), false, false),
            (root.join("CLOUD/Doc.slop"), false, false),
            // A template folder refuses new documents; a document already there still opens.
            (templates.join("Doc.slop"), true, false),
            (templates.join("cache/publisher/1.slop"), true, false),
            (root.join("TEMPLATES/Doc.slop"), true, false),
            (root.join("templates-backup/Doc.slop"), true, true),
            (root.join("cloud-backup/Doc.slop"), true, true),
            (root.join("documents/Doc.slop"), true, true),
        ] {
            assert_eq!(check_location(&path, &places).is_ok(), location, "open {}", path.display());
            assert_eq!(check_destination(&path, &places).is_ok(), destination, "create {}", path.display());
        }
        assert!(!cloud.join("new").exists(), "checking a place creates nothing");
    }
}
