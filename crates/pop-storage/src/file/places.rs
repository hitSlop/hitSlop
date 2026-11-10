//! Where documents may live: never in iCloud Drive, and new ones never among the templates.
//! Also where templates are listed from (`catalog`).

use crate::error::{Result, invalid};
use std::fs;
use std::path::{Path, PathBuf};

use pop_contracts::catalog::TemplateSource;

/// Where documents may not live, for this account and executable: iCloud Drive, whose
/// syncing documents do not support yet, and the installed and bundled templates.
struct Places {
    cloud: Option<PathBuf>,
    /// The template folders hosts list, the bundled starters first.
    listed: Vec<(TemplateSource, PathBuf)>,
    /// A template folder hosts do not list, which new documents still stay out of: the
    /// account's while a development app's replaces it.
    unlisted: Option<PathBuf>,
}
impl Places {
    fn current() -> Self {
        let home = crate::registry::home();
        let account = home.as_ref().map(|home| home.join(".pop/templates"));
        // A development app's catalog, which replaces the account's in the listing.
        let development = std::env::var_os("POP_TEMPLATES_ROOT").filter(|root| !root.is_empty()).map(PathBuf::from);
        let bundled =
            application().ok().map(|app| app.join("Contents/Resources/StarterTemplates")).filter(|path| path.is_dir());
        let (installed, unlisted) = match development {
            Some(root) => (Some(root), account),
            None => (account, None),
        };
        let listed = bundled
            .map(|root| (TemplateSource::Bundled, root))
            .into_iter()
            .chain(installed.map(|root| (TemplateSource::Installed, root)))
            .collect();
        Self { cloud: home.map(|home| home.join("Library/Mobile Documents")), listed, unlisted }
    }
    /// Every template folder new documents stay out of.
    fn templates(&self) -> impl Iterator<Item = &Path> {
        self.listed.iter().map(|(_, root)| root.as_path()).chain(self.unlisted.as_deref())
    }
}
/// The template folders hosts list, for this account and executable: the app's bundled
/// starters (when this executable is inside the app), then the installed templates folder.
pub fn template_roots() -> Vec<(TemplateSource, PathBuf)> {
    Places::current().listed
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
/// Where a document may open for writing: a `.pop` file outside iCloud Drive.
fn check_location(path: &Path, places: &Places) -> Result<()> {
    if path.extension().is_none_or(|ext| ext != "pop") {
        return Err(invalid("A document's name must end in .pop"));
    }
    if places.cloud.as_deref().is_some_and(|cloud| within(path, cloud, true)) {
        return Err(invalid("iCloud document locations are not supported. Move the document to a local folder."));
    }
    Ok(())
}
/// Where a new document may go: where a document may open, outside the templates.
fn check_destination(path: &Path, places: &Places) -> Result<()> {
    check_location(path, places)?;
    if places.templates().any(|root| within(path, root, false)) {
        return Err(invalid("A document cannot be created among installed templates"));
    }
    Ok(())
}
/// Refuses a document opened for writing where documents may not live. The app and the
/// engine both open documents through the store, so they share the rule.
pub(crate) fn document_location(path: &Path) -> Result<()> {
    check_location(path, &Places::current())
}
/// Refuses a new document (created or copied) where documents may not go.
pub(crate) fn document_destination(path: &Path) -> Result<()> {
    check_destination(path, &Places::current())
}

/// Finds the containing, explicitly selected, or installed app for this account.
pub fn application() -> Result<PathBuf> {
    let executable = std::env::current_exe().and_then(fs::canonicalize).ok();
    let named = std::env::var_os("POP_APP").filter(|app| !app.is_empty()).map(PathBuf::from);
    let installed: Vec<_> = [PathBuf::from("/Applications/dotpop.app")]
        .into_iter()
        .chain(crate::registry::home().map(|home| home.join("Applications/dotpop.app")))
        .collect();
    discover_application(executable.as_deref(), named.as_deref(), &installed).map_err(invalid)
}

/// The dotpop.app that opens documents: the app this executable runs inside, else the
/// development app `POP_APP` names, else the first installed one present.
fn discover_application(
    executable: Option<&Path>,
    named: Option<&Path>,
    installed: &[PathBuf],
) -> std::result::Result<PathBuf, String> {
    let containing = executable.and_then(|executable| {
        let contents = executable.parent()?.parent()?;
        let app = contents.parent()?;
        (contents.file_name()?.eq_ignore_ascii_case("Contents") && app.extension()?.eq_ignore_ascii_case("app"))
            .then(|| app.to_owned())
    });
    if let Some(app) = containing {
        return Ok(app);
    }
    if let Some(named) = named {
        // An explicit choice never falls back to another copy.
        return if named.is_dir() {
            Ok(named.to_owned())
        } else {
            Err(format!("POP_APP names no app: {}", named.display()))
        };
    }
    installed
        .iter()
        .find(|app| app.is_dir())
        .cloned()
        .ok_or_else(|| "Install dotpop.app in /Applications or ~/Applications to open documents".into())
}

#[cfg(test)]
mod application_tests {
    use super::*;

    #[test]
    fn the_containing_app_then_the_named_one_then_an_installed_one_opens_documents() {
        let dir = tempfile::tempdir().unwrap();
        let app = |name: &str| {
            let app = dir.path().join(name);
            std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
            app
        };
        let (inside, named) = (app("Inside.app"), app("Development/dotpop.app"));
        let installed = [app("Applications/dotpop.app"), app("Home/Applications/dotpop.app")];
        let engine = inside.join("Contents/MacOS/pop-engine");
        let outside = dir.path().join("bin/pop-engine");
        assert_eq!(discover_application(Some(&engine), Some(&named), &installed), Ok(inside));
        assert_eq!(discover_application(Some(&outside), Some(&named), &installed), Ok(named.clone()));
        let missing = dir.path().join("Missing.app");
        assert!(discover_application(Some(&outside), Some(&missing), &installed).unwrap_err().contains("POP_APP"));
        assert_eq!(discover_application(Some(&outside), None, &installed), Ok(installed[0].clone()));
        std::fs::remove_dir_all(&installed[0]).unwrap();
        assert_eq!(discover_application(None, None, &installed), Ok(installed[1].clone()));
        std::fs::remove_dir_all(&installed[1]).unwrap();
        assert!(discover_application(None, None, &installed).unwrap_err().contains("Install dotpop.app"));
    }
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
        let places = Places {
            cloud: Some(cloud.clone()),
            listed: vec![(TemplateSource::Installed, templates.clone())],
            unlisted: None,
        };
        for (path, location, destination) in [
            (root.join("notes"), false, false),
            (root.join("unsupported.txt"), false, false),
            (root.join("unsupported.pop.zip"), false, false),
            (root.join("Notes.POP"), false, false),
            (cloud.clone(), false, false),
            (cloud.join("new/Doc.pop"), false, false),
            (root.join("alias/Doc.pop"), false, false),
            (root.join("CLOUD/Doc.pop"), false, false),
            // A template folder refuses new documents; a document already there still opens.
            (templates.join("Doc.pop"), true, false),
            (templates.join("cache/publisher/1.pop"), true, false),
            (root.join("TEMPLATES/Doc.pop"), true, false),
            (root.join("templates-backup/Doc.pop"), true, true),
            (root.join("cloud-backup/Doc.pop"), true, true),
            (root.join("documents/Doc.pop"), true, true),
        ] {
            assert_eq!(check_location(&path, &places).is_ok(), location, "open {}", path.display());
            assert_eq!(check_destination(&path, &places).is_ok(), destination, "create {}", path.display());
        }
        assert!(!cloud.join("new").exists(), "checking a place creates nothing");
    }
}
