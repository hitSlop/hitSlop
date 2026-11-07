//! Device-index paths. Library-owned locations are relative; external documents
//! retain their absolute location. Public entries always expose resolved paths.
use crate::{Result, failed};
use rusqlite::Connection;
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
pub(super) fn indexed(db: &Connection, root: &Path, id: &str) -> Result<Vec<(String, PathBuf)>> {
    let rows = db
        .prepare("SELECT account,path FROM documents WHERE id=?1")?
        .query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter().map(|(account, path)| Ok((account, resolve(root, &path)?))).collect()
}
