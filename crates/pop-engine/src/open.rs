//! Opening a document in a window: the one request that needs dotpop.app, so macOS only.
use super::refusal;
use pop_contracts::wire::Code;
use pop_storage::{file, store};
use std::path::Path;
use std::process::{Command, Stdio};

/// Opens the document at `path` in dotpop.app and returns its canonical path. A template
/// is refused: the app opens one by creating a document from it.
pub(super) fn document(path: &Path) -> store::Result<String> {
    if !cfg!(target_os = "macos") {
        return Err(refusal(
            Code::InvalidRequest,
            "Opening windows requires macOS and dotpop.app; document edits and authoring run anywhere.",
        ));
    }
    let document = file::existing(path)?;
    file::document_location(&document)?;
    if file::kind(&document)? == file::Kind::Template {
        return Err(refusal(Code::IsTemplate, "This is a template; create a document from it first"));
    }
    let app = file::application()?;
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
