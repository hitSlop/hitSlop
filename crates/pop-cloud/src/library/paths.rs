//! Device-index paths. Library-owned locations are relative; external documents
//! retain their absolute location. Public entries always expose resolved paths.
use super::{Result, failed};
use std::path::{Component, Path, PathBuf};

fn relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() || path.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(failed("Invalid managed library path"));
    }
    Ok(())
}
pub(super) fn resolve(root: &Path, stored: &str) -> Result<PathBuf> {
    let path = Path::new(stored);
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    relative(path)?;
    Ok(root.join(path))
}
pub(super) fn store(root: &Path, path: &Path) -> Result<String> {
    if !path.is_absolute() {
        return Err(failed("Library path must be resolved before indexing"));
    }
    let stored = match path.strip_prefix(root) {
        Ok(path) => {
            relative(path)?;
            path
        }
        Err(_) => path,
    };
    Ok(stored.to_string_lossy().into_owned())
}
