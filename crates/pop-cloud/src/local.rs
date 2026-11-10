//! Accepted local document inspection, owner calls and exclusive installation.
use pop_contracts::hex;
use pop_runtime::owner::{Owner, Reply, Request};
use pop_storage::file;
use pop_storage::store::{Mode, Store};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

type Result<T> = std::result::Result<T, String>;
#[derive(Debug)]
pub struct Info {
    pub id: String,
    pub version: String,
    pub artwork_digest: String,
}
pub fn request(owner: &Owner, request: Request) -> Result<Reply> {
    request_typed(owner, request).map_err(|e| e.to_string())
}
pub(crate) fn request_typed(
    owner: &Owner,
    request: Request,
) -> std::result::Result<Reply, pop_runtime::owner::Failure> {
    let (send, receive) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |result| {
            let _ = send.send(result);
        }),
    );
    receive.recv_timeout(Duration::from_secs(20)).map_err(|e| pop_runtime::owner::Failure {
        kind: pop_runtime::owner::FailureKind::Failed,
        message: e.to_string(),
        reason: None,
        op_index: None,
    })?
}
pub fn inspect(path: &Path) -> Result<Info> {
    let store = Store::open(path, Mode::Snapshot).map_err(|e| e.to_string())?;
    let reading = store.document().map_err(|e| e.to_string())?.reading().map_err(|e| e.to_string())?;
    Ok(Info {
        id: store.app().document_uuid.clone().ok_or("Not a document")?,
        version: reading.version,
        artwork_digest: artwork_digest(&store)?,
    })
}
/// A new replica keeps the canonical identity. This never replaces an existing file.
pub fn install(source: &Path, destination: &Path, expected_id: &str) -> Result<()> {
    let info = inspect(source)?;
    if info.id != expected_id {
        return Err("Snapshot identity does not match its record".into());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    file::copy_replica(source, destination).map_err(|e| e.to_string())
}
/// Cosmetic bytes are independent of the Loro version. Hash only the bounded saved PNGs.
pub(crate) fn artwork_digest(store: &Store) -> Result<String> {
    let mut hash = Sha256::new();
    for name in file::Artwork::ALL {
        hash.update(name.as_str().as_bytes());
        if let Some(bytes) = store.artwork(name).map_err(|e| e.to_string())? {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    Ok(hex(&hash.finalize()))
}
/// Stale fallback images do not make replicas with identical state fight over caches.
pub(crate) fn asset_revision(version: &str, artwork_version: &str, digest: &str) -> String {
    let artwork = if version == artwork_version { digest } else { "" };
    hex(&Sha256::digest(serde_json::to_string(&(version, artwork)).unwrap()))
}
