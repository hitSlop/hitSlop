//! Device-index paths. Library-owned locations are relative; external documents
//! retain their absolute location. Public entries always expose resolved paths.
use crate::{Result, failed};
use rusqlite::{Connection, params};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

fn relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() || path.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(failed("Invalid managed library path"));
    }
    Ok(())
}
pub(super) fn resolve(root: &Path, stored: &str) -> Result<PathBuf> {
    let path = Path::new(stored);
    if path.is_absolute() {
        return Ok(pop_storage::file::rebase_ios_container_path(path, root).unwrap_or_else(|| path.to_owned()));
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
/// Run before file discovery or Inbox inference. No document bytes are changed.
pub(super) fn migrate(db: &Connection, root: &Path) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    let rows = tx
        .prepare("SELECT account,id,path FROM documents")?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut locations = vec![];
    for (account, id, path) in rows {
        let updated = store(root, &resolve(root, &path)?)?;
        if updated != path {
            tx.execute("UPDATE documents SET path=?3 WHERE account=?1 AND id=?2", params![account, id, updated])?;
        }
        locations.push((id, updated));
    }
    let sources = tx
        .prepare("SELECT path,id,fingerprint FROM legacy_sources ORDER BY path")?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut grouped: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    for (path, id, fingerprint) in sources {
        let key = store(root, &resolve(root, &path)?)?;
        grouped.entry(key).or_default().push((path, id, fingerprint));
    }
    // Affected updates may have recorded both container prefixes. Prefer the unique
    // mapping whose indexed document actually owns the source path; keep every file
    // and recovery index row. Otherwise retain the current key conservatively.
    tx.execute("DELETE FROM legacy_sources", [])?;
    for (key, candidates) in grouped {
        let matching = candidates
            .iter()
            .filter(|(_, id, _)| locations.iter().any(|(doc, path)| doc == id && path == &key))
            .collect::<Vec<_>>();
        let preferred = if matching.len() == 1 {
            matching[0]
        } else {
            candidates.iter().find(|(path, _, _)| path == &key).unwrap_or(&candidates[0])
        };
        tx.execute(
            "INSERT INTO legacy_sources(path,id,fingerprint) VALUES(?1,?2,?3)",
            params![key, preferred.1, preferred.2],
        )?;
    }
    tx.commit()?;
    Ok(())
}
