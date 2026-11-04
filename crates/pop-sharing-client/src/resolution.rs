//! Creator recovery before a local file or document owner exists. Validated seed bytes
//! stay in Rust-owned private staging; native callers receive only an opaque plan.
use super::*;
use pop_sharing_protocol::lifecycle::EpochState;
use std::path::PathBuf;

pub struct ResolvedRoom {
    pub(crate) key: RoomKey,
    pub(crate) info: RoomInfo,
    pub(crate) epoch: EpochState,
    credential: String,
    pub(crate) seed: PrivateSeed,
}
impl ResolvedRoom {
    pub fn document(&self) -> &str {
        &self.info.document
    }
    pub fn room(&self) -> &str {
        &self.key.room
    }
    pub fn seed_hash(&self) -> &str {
        &self.epoch.seed_hash
    }
    pub fn validate_authority(&self, authority: &pop_library::CatalogAuthority) -> Result<()> {
        let binding = authority.relay_binding().map_err(fail)?.ok_or("The catalog has no relay authority")?;
        if binding.room != self.key.room {
            return Err("Different catalog relay room".into());
        }
        if let pop_library::CatalogAuthority::Handoff { freeze, .. } = authority {
            let frozen: pop_sharing_protocol::lifecycle::Handoff = serde_json::from_str(freeze).map_err(fail)?;
            if self.epoch.handoff.as_ref() != Some(&frozen) {
                return Err("Different frozen catalog handoff".into());
            }
        }
        Ok(())
    }
    pub fn credential(&self) -> &str {
        &self.credential
    }
    /// Canonical metadata only; checkpoint/history bytes never cross the native bridge.
    pub fn handoff(&self) -> Result<Option<String>> {
        self.epoch.handoff.as_ref().map(serde_json::to_string).transpose().map_err(fail)
    }
    /// Installs only a missing canonical file. Existing local state is never replaced by
    /// resolution; its owner must explicitly adopt an epoch with recovery safeguards.
    pub fn install(&self, destination: &Path, state_directory: &Path) -> Result<()> {
        let journal = journal::Journal::new(state_directory, &self.key, self.document())?;
        let transaction = journal.transaction()?;
        let source = Store::open(&self.seed.path, Mode::Snapshot).map_err(fail)?;
        let version = source.document().map_err(fail)?.version();
        source.close().map_err(fail)?;
        let receipt = journal.receipt(self.epoch.clone(), version.clone(), None);
        if destination.exists() {
            // Only finish our own interrupted installation; arbitrary existing edits require an owner.
            if transaction.pending()?.as_ref() != Some(&receipt) {
                return Err("A local document already exists; resolve it through its owner".into());
            }
            let local = Store::open(destination, Mode::Snapshot).map_err(fail)?;
            if local.app().document_uuid.as_deref() != Some(self.document())
                || local.document().map_err(fail)?.version() != version
            {
                return Err("The local document changed during installation".into());
            }
            local.close().map_err(fail)?;
        } else {
            transaction.prepare(&receipt)?;
            let bytes = std::fs::read(&self.seed.path).map_err(fail)?;
            install_seed(destination, &self.key, &self.info, &bytes)?;
        }
        transaction.promote(&receipt)
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
/// unfinished upload has no accepted live plan yet; its catalog Preparing state resumes it.
pub fn resolve(document: &str, token: &str) -> Result<Option<ResolvedRoom>> {
    resolve_at(document, token, pop_sharing_protocol::TRUSTED_ORIGIN)
}
pub(super) fn resolve_at(document: &str, token: &str, origin: &str) -> Result<Option<ResolvedRoom>> {
    if !pop_sharing_protocol::valid_copy(document) {
        return Err("Invalid document identity".into());
    }
    let room: Option<AccountRoom> =
        account_request_at(origin, "GET", &format!("account/rooms/{document}"), token, &[])?;
    let Some(room) = room else { return Ok(None) };
    if !pop_sharing_protocol::valid_room(&room.room) {
        return Err("Invalid account room".into());
    }
    let Some((credential, info)) = creator_credential_at(document, &room.room, token, origin)? else { return Ok(None) };
    if info.phase != RoomPhase::Live {
        return Ok(None);
    }
    let key = RoomKey { origin: origin.into(), ..credential.runtime()? };
    let epoch = epoch(&key, token)?;
    download(key, info, epoch, credential.encode()?, token).map(Some).map_err(String::from)
}

pub(crate) fn download(
    key: RoomKey,
    info: RoomInfo,
    epoch: EpochState,
    credential: String,
    token: &str,
) -> std::result::Result<ResolvedRoom, transport::HttpFailure> {
    if info.seed_hash.as_deref() != Some(epoch.seed_hash.as_str()) {
        return Err("The relay epoch changed while resolving; retry".into());
    }
    let bytes = transport::http_typed(&key, "GET", &format!("seed?hash={}", epoch.seed_hash), Some(token), &[], &[])?;
    if bytes.len() > pop_sharing_protocol::SEED_BYTES
        || Some(bytes.len() as u32) != info.seed_size
        || hex(&Sha256::digest(&bytes)) != epoch.seed_hash
    {
        return Err("Seed checksum mismatch".into());
    }
    let seed = PrivateSeed::write(&bytes)?;
    let store = Store::open(&seed.path, Mode::Snapshot).map_err(transport::HttpFailure::from)?;
    if store.app().document_uuid.as_deref() != Some(info.document.as_str())
        || Some(hex(store.app_revision_id().map_err(transport::HttpFailure::from)?.as_bytes())) != info.app
    {
        return Err("Resolved seed document/app mismatch".into());
    }
    let root = store.document().map_err(transport::HttpFailure::from)?.wire_history();
    if root != epoch.root {
        return Err("Resolved seed frontier mismatch".into());
    }
    store.close().map_err(transport::HttpFailure::from)?;
    Ok(ResolvedRoom { key, info, epoch, credential, seed })
}

pub(crate) fn epoch(key: &RoomKey, identity: &str) -> std::result::Result<EpochState, transport::HttpFailure> {
    epoch_at(&key.origin, &key.room, identity)
}
pub(crate) fn epoch_at(
    origin: &str,
    room: &str,
    identity: &str,
) -> std::result::Result<EpochState, transport::HttpFailure> {
    let bytes = transport::room_control(origin, room, "GET", "epoch", identity, &[])?;
    let epoch: EpochState = serde_json::from_slice(&bytes).map_err(fail)?;
    epoch.validate()?;
    Ok(epoch)
}
