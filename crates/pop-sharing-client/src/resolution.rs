//! Creator downloads before a local file or document owner exists. Validated seed bytes
//! stay in Rust-owned private staging; native callers receive only an opaque plan.
use super::*;
use std::path::PathBuf;

pub struct ResolvedRoom {
    pub(crate) key: RoomKey,
    pub(crate) info: RoomInfo,
    pub(crate) seed: PrivateSeed,
}
impl ResolvedRoom {
    pub fn document(&self) -> &str {
        &self.info.document
    }
    pub fn room(&self) -> &str {
        &self.key.room
    }
    /// Installs only a missing file; existing local state is never replaced. The relay
    /// already holds the seed, so the new replica starts confirmed.
    pub fn install(&self, destination: &Path, state_directory: &Path) -> Result<()> {
        if destination.exists() {
            return Err("A local document already exists; resolve it through its owner".into());
        }
        let journal = journal::Journal::new(state_directory, &self.key, self.document())?;
        let source = Store::open(&self.seed.path, Mode::Snapshot).map_err(fail)?;
        let version = source.document().map_err(fail)?.version();
        source.close().map_err(fail)?;
        let bytes = std::fs::read(&self.seed.path).map_err(fail)?;
        install_seed(destination, &self.key, &self.info, &bytes)?;
        journal::confirm_seed(&journal, version)?;
        Ok(())
    }
}
pub(crate) struct PrivateSeed {
    pub(crate) path: PathBuf,
}
impl PrivateSeed {
    fn write(bytes: &[u8]) -> Result<Self> {
        let path = std::env::temp_dir().join(format!("pop-resolved-{}.pop", random()?));
        if let Err(error) = private_file(&path, bytes) {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        Ok(Self { path })
    }
}
impl Drop for PrivateSeed {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Resolve creator access by account identity, without a local file or Owner. An
/// unfinished upload has no live seed yet; its catalog Preparing state resumes it.
pub fn resolve(document: &str, token: &str) -> Result<Option<ResolvedRoom>> {
    let key = RoomKey::identity(document, token)?;
    let info = room_info(&key, Some(token))?;
    if info.phase != RoomPhase::Live {
        return Ok(None);
    }
    let bytes = transport::http_typed(&key, "GET", "seed", Some(token), &[], &[])?;
    if bytes.len() > pop_sharing_protocol::SEED_BYTES
        || Some(bytes.len() as u32) != info.seed_size
        || Some(hex(&Sha256::digest(&bytes))) != info.seed_hash
    {
        return Err("Seed checksum mismatch".into());
    }
    let seed = PrivateSeed::write(&bytes)?;
    let store = Store::open(&seed.path, Mode::Snapshot).map_err(fail)?;
    if store.app().document_uuid.as_deref() != Some(info.document.as_str())
        || Some(hex(store.app_revision_id().map_err(fail)?.as_bytes())) != info.app
    {
        return Err("Resolved seed document/app mismatch".into());
    }
    store.close().map_err(fail)?;
    Ok(Some(ResolvedRoom { key, info, seed }))
}
