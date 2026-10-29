//! Opening a document in a window: the one request that needs tinys.app, so macOS only.
use super::refusal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinys_core::{Code, file, store};

/// Opens the document at `path` in tinys.app and returns its canonical path. A template
/// is refused: the app opens one by creating a document from it.
pub(super) fn document(path: &Path) -> store::Result<String> {
    if !cfg!(target_os = "macos") {
        return Err(refusal(
            Code::InvalidRequest,
            "Opening windows requires macOS and tinys.app; document edits and authoring run anywhere.",
        ));
    }
    let document = file::existing(path)?;
    file::document_location(&document)?;
    if file::kind(&document)? == file::Kind::Template {
        return Err(refusal(Code::IsTemplate, "This is a template; create a document from it first"));
    }
    let executable = std::env::current_exe().and_then(std::fs::canonicalize).ok();
    let named = std::env::var_os("TINYS_APP").filter(|app| !app.is_empty()).map(PathBuf::from);
    let home = std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Applications/tinys.app"));
    let installed: Vec<_> = [PathBuf::from("/Applications/tinys.app")].into_iter().chain(home).collect();
    let app = application(executable.as_deref(), named.as_deref(), &installed)
        .map_err(|message| refusal(Code::InvalidRequest, message))?;
    // `-a` opens with the app chosen here; `-b` would take LaunchServices' pick, which can be
    // a stale or development copy. A canonical path is absolute, so never read as an option.
    let opened = Command::new("/usr/bin/open")
        .arg("-a")
        .arg(&app)
        .arg(&document)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| refusal(Code::InvalidRequest, format!("Cannot open the document: {e}")))?;
    if !opened.status.success() {
        let error = String::from_utf8_lossy(&opened.stderr);
        return Err(refusal(
            Code::InvalidRequest,
            format!("{} could not open the document: {}", app.display(), error.trim()),
        ));
    }
    Ok(document.to_string_lossy().into_owned())
}

/// The tinys.app that opens documents: the app this executable runs inside, else the
/// development app `TINYS_APP` names, else the first installed one present.
fn application(executable: Option<&Path>, named: Option<&Path>, installed: &[PathBuf]) -> Result<PathBuf, String> {
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
            Err(format!("TINYS_APP names no app: {}", named.display()))
        };
    }
    installed
        .iter()
        .find(|app| app.is_dir())
        .cloned()
        .ok_or_else(|| "Install tinys.app in /Applications or ~/Applications to open documents".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_containing_app_then_the_named_one_then_an_installed_one_opens_documents() {
        let dir = tempfile::tempdir().unwrap();
        let app = |name: &str| {
            let app = dir.path().join(name);
            std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
            app
        };
        let (inside, named) = (app("Inside.app"), app("Development/tinys.app"));
        let installed = [app("Applications/tinys.app"), app("Home/Applications/tinys.app")];
        let engine = inside.join("Contents/MacOS/tiny-engine");
        let outside = dir.path().join("bin/tiny-engine");
        assert_eq!(application(Some(&engine), Some(&named), &installed), Ok(inside));
        assert_eq!(application(Some(&outside), Some(&named), &installed), Ok(named.clone()));
        let missing = dir.path().join("Missing.app");
        assert!(application(Some(&outside), Some(&missing), &installed).unwrap_err().contains("TINYS_APP"));
        assert_eq!(application(Some(&outside), None, &installed), Ok(installed[0].clone()));
        std::fs::remove_dir_all(&installed[0]).unwrap();
        assert_eq!(application(None, None, &installed), Ok(installed[1].clone()));
        std::fs::remove_dir_all(&installed[1]).unwrap();
        assert!(application(None, None, &installed).unwrap_err().contains("Install tinys.app"));
    }
}
