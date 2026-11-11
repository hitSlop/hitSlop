//! Creator downloads before a local file or document owner exists. Validated seed bytes
//! stay in Rust-owned private staging; native callers receive only an opaque plan.
use crate::client::*;
use crate::library::receipts;
use crate::{attachments, local, transport};
use pop_contracts::hex;
use pop_runtime::owner::Owner;
use pop_sharing_protocol::http::{RoomInfo, RoomPhase};
use pop_storage::store::{Mode, Store};
use reqwest::Method;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use transport::{Endpoint, HttpOperation, OwnerEffect};

pub struct ResolvedRoom {
    pub(crate) key: RoomKey,
    pub(crate) info: RoomInfo,
    pub(crate) seed: PrivateSeed,
}
impl ResolvedRoom {
    pub fn document(&self) -> &str {
        &self.info.document
    }
    /// Installs only a missing file; existing local state is never replaced. The relay
    /// already holds the seed, so the new replica starts confirmed. Check the caller's
    /// account-operation fence after network IO, before publishing.
    pub fn install_if_current(
        &self,
        destination: &Path,
        receipt: receipts::Receipt,
        current: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        if destination.exists() {
            return Err("A local document already exists; resolve it through its owner".into());
        }
        let document = self.document().to_owned();
        let key = self.key.clone();
        let info = self.info.clone();
        let path = self.seed.path.clone();
        let destination = destination.to_owned();
        transport::block_on(async move {
            let install_destination = destination.clone();
            let (stage, owner, version) = transport::blocking(move || {
                let source = Store::open(&path, Mode::Snapshot)?;
                let version = source.document()?.version();
                source.close()?;
                let bytes = std::fs::read(path)?;
                if Some(hex(&Sha256::digest(&bytes))) != info.seed_hash || Some(bytes.len() as u32) != info.seed_size {
                    return Err("Seed checksum mismatch".into());
                }
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let stage = destination.with_file_name(format!(
                    ".pop-sharing-{}.pop",
                    pop_core::random_hex(32).map_err(|_| Error::from("Cannot generate document identity"))?
                ));
                let stage = PrivateSeed { path: stage };
                private_file(&stage.path, &bytes)?;
                let owner = Arc::new(Owner::open(&stage.path, Mode::Document, Arc::new(|_| {}))?);
                Ok((stage, owner, version))
            })
            .await?;
            let hydrated = async {
                transport::owner_request(
                    &owner,
                    &transport::OwnerOutcome::default(),
                    |owner, options, callback| {
                        owner.set_share(pop_storage::store::Share { endpoint: key.origin.clone() }, options, callback)
                    },
                    OwnerEffect::Other,
                )
                .await?;
                attachments::download_blobs(&owner, &transport::OwnerOutcome::default(), &key, &mut Vec::new()).await
            }
            .await;
            let closed = transport::owner_request(
                &owner,
                &transport::OwnerOutcome::default(),
                |owner, options, callback| owner.close(None, None, options, callback),
                OwnerEffect::Other,
            )
            .await;
            hydrated?;
            closed?;
            current()?;
            transport::blocking(move || local::install(&stage.path, &install_destination, &document)).await?;
            transport::blocking(move || receipt(version)).await
        })
    }
}
pub(crate) struct PrivateSeed {
    pub(crate) path: PathBuf,
}
impl PrivateSeed {
    fn write(bytes: &[u8]) -> Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "pop-resolved-{}.pop",
            pop_core::random_hex(32).map_err(|_| "Cannot generate document identity")?
        ));
        if let Err(error) = private_file(&path, bytes) {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        Ok(Self { path })
    }
}
impl Drop for PrivateSeed {
    fn drop(&mut self) {
        transport::discard_files(vec![self.path.clone()], None);
    }
}

/// Resolve creator access by account identity, without a local file or Owner. An
/// unfinished upload has no live seed yet; its catalog Preparing state resumes it.
pub fn resolve(document: &str, token: &str) -> Result<Option<ResolvedRoom>> {
    transport::block_on(resolve_async(document, token))
}
async fn resolve_async(document: &str, token: &str) -> Result<Option<ResolvedRoom>> {
    let key = RoomKey::identity(document, token)?;
    let info = room_info(&key).await?;
    if info.phase != RoomPhase::Live {
        return Ok(None);
    }
    let bytes = transport::http(&key.origin, token, HttpOperation::new(Method::GET, Endpoint::Seed(document)))
        .await?
        .unwrap_or_default();
    transport::blocking(move || {
        if bytes.len() > pop_sharing_protocol::SEED_BYTES
            || Some(bytes.len() as u32) != info.seed_size
            || Some(hex(&Sha256::digest(&bytes))) != info.seed_hash
        {
            return Err("Seed checksum mismatch".into());
        }
        let seed = PrivateSeed::write(&bytes)?;
        let store = Store::open(&seed.path, Mode::Snapshot).map_err(Error::from)?;
        if store.app().document_uuid.as_deref() != Some(info.document.as_str())
            || Some(hex(store.app_digest().map_err(Error::from)?.as_bytes())) != info.app
        {
            return Err("Resolved seed document/app mismatch".into());
        }
        store.close().map_err(Error::from)?;
        Ok(Some(ResolvedRoom { key, info, seed }))
    })
    .await
}
