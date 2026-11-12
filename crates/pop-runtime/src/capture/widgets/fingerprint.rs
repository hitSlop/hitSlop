//! A disposable cache identity for the actual installed renderer, including its fonts.
use pop_storage::file::FileStamp;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
pub(super) struct Fingerprint {
    pub digest: [u8; 32],
    stamp: FileStamp,
}
impl Fingerprint {
    pub fn check(&self, path: &Path) -> io::Result<()> {
        if FileStamp::from(std::fs::metadata(path)?) != self.stamp {
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
    let stamp = FileStamp::from(file.metadata()?);
    if let Some((cached_path, cached)) = &*pop_core::lock(cache)
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
    *pop_core::lock(cache) = Some((path.to_owned(), fingerprint.clone()));
    Ok(fingerprint)
}
