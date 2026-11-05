//! Completed independent copies are their own durable local discovery index.
//! Rolling authority receipts may advance without forgetting earlier local edits.
use super::*;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileStamp {
    device: u64,
    inode: u64,
    bytes: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
pub(crate) fn stamp(path: &Path) -> Option<FileStamp> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).ok()?;
    Some(FileStamp {
        device: meta.dev(),
        inode: meta.ino(),
        bytes: meta.len(),
        modified: (meta.mtime(), meta.mtime_nsec()),
        changed: (meta.ctime(), meta.ctime_nsec()),
    })
}

pub(crate) fn directory(state_directory: &Path) -> Result<PathBuf> {
    if !state_directory.is_absolute() {
        return Err("Invalid relay recovery location".into());
    }
    let directory = state_directory.join("Recovered");
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&directory).map_err(fail)?;
    std::fs::File::open(state_directory).and_then(|parent| parent.sync_all()).map_err(fail)?;
    Ok(directory)
}
pub(crate) fn accepted(path: &Path, original: Option<&str>) -> bool {
    let Ok(store) = Store::open(path, Mode::Snapshot) else { return false };
    let valid = store.app().document_uuid.as_deref().is_some_and(|id| Some(id) != original)
        && store.share().is_ok_and(|share| share.is_none())
        && store.document().is_ok();
    store.close().is_ok() && valid
}
pub(crate) fn paths(state_directory: &Path) -> Result<Vec<PathBuf>> {
    if !state_directory.is_absolute() {
        return Err("Invalid relay recovery location".into());
    }
    let entries = match std::fs::read_dir(state_directory.join("Recovered")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(fail(error)),
    };
    let mut paths = vec![];
    for entry in entries {
        let entry = entry.map_err(fail)?;
        if entry.file_type().map_err(fail)?.is_file()
            && entry.path().extension().is_some_and(|extension| extension == "pop")
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}
/// Discover every accepted independent recovery, including copies completed before
/// an interrupted canonical installation. Discovery does not enable cloud sync for these local files.
pub fn recovered_documents(state_directory: &Path) -> Result<Vec<String>> {
    Ok(paths(state_directory)?
        .into_iter()
        .filter(|path| accepted(path, None))
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}
