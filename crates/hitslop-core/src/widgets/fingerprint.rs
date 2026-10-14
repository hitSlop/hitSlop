//! A disposable cache identity for the actual installed renderer, including its fonts.
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl From<Metadata> for Stamp {
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

#[derive(Clone)]
pub(super) struct Fingerprint {
    pub digest: [u8; 32],
    stamp: Stamp,
}
impl Fingerprint {
    pub fn check(&self, path: &Path) -> io::Result<()> {
        if Stamp::from(std::fs::metadata(path)?) != self.stamp {
            return Err(io::Error::other("Widget renderer changed during capture; retry with the updated renderer"));
        }
        Ok(())
    }
}

pub(super) fn renderer(path: &Path) -> io::Result<Fingerprint> {
    // One evaluator is normal. Bound the cache even when a development host switches
    // helpers repeatedly; never hold the mutex while reading or hashing the binary.
    static CACHE: OnceLock<Mutex<Option<(PathBuf, Fingerprint)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut file = File::open(path)?;
    let stamp = Stamp::from(file.metadata()?);
    if let Some((cached_path, cached)) = &*cache.lock().unwrap_or_else(|e| e.into_inner())
        && cached_path == path
        && cached.stamp == stamp
    {
        return Ok(cached.clone());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let fingerprint = Fingerprint { digest: hash.finalize().into(), stamp };
    fingerprint.check(path)?;
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((path.to_owned(), fingerprint.clone()));
    Ok(fingerprint)
}
