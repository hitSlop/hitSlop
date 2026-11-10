//! File identity and freshness for disposable host caches.
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl From<Metadata> for FileStamp {
    fn from(value: Metadata) -> Self {
        Self {
            device: value.dev(),
            inode: value.ino(),
            size: value.len(),
            modified: (value.mtime(), value.mtime_nsec()),
            changed: (value.ctime(), value.ctime_nsec()),
        }
    }
}
