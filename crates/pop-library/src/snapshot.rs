//! Full-file library snapshots. CloudKit and document bytes never meet in Swift.
use pop_runtime::owner::{Owner, Reply, Request};
use pop_storage::{
    file,
    store::{Mode, Store},
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::mpsc, time::Duration};

pub const MAX_FILE_BYTES: u64 = 16 << 20;
type Result<T> = std::result::Result<T, String>;
#[derive(Debug)]
pub struct Info {
    pub id: String,
    pub version: String,
    pub fingerprint: String,
    pub app_digest: [u8; 32],
    pub artwork_digest: String,
}
pub fn request(owner: &Owner, request: Request) -> Result<Reply> {
    request_typed(owner, request).map_err(|e| e.to_string())
}
pub fn request_typed(owner: &Owner, request: Request) -> std::result::Result<Reply, pop_runtime::owner::Failure> {
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
fn bounded(path: &Path) -> Result<()> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES {
        return Err("iCloud sync supports pop up to 16 MiB; the local document is retained".into());
    }
    Ok(())
}
pub fn inspect(path: &Path) -> Result<Info> {
    bounded(path)?;
    let store = Store::open(path, Mode::Snapshot).map_err(|e| e.to_string())?;
    let reading = store.document().map_err(|e| e.to_string())?.reading().map_err(|e| e.to_string())?;
    let attachment_ids: Vec<_> = store.attachments().map_err(|e| e.to_string())?.into_iter().map(|a| a.id).collect();
    let state = serde_json::json!({"value": reading.value, "theme": reading.theme, "attachments": attachment_ids});
    Ok(Info {
        id: store.app().document_uuid.clone().ok_or("Not a document")?,
        version: reading.version,
        fingerprint: crate::hex(&Sha256::digest(crate::encode(&state))),
        app_digest: store.app_digest().map_err(|e| e.to_string())?,
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
/// Validate a complete incoming file, bring its immutable blobs across, then merge
/// history through the live owner. A download never replaces that owner's backing file.
pub fn merge(owner: &Owner, source: &Path) -> Result<Info> {
    merge_typed(owner, source).map_err(|error| error.to_string())
}
/// Keeps semantic refusal codes so a confirmed authority receipt can select an
/// explicit recovery/adoption path; ordinary snapshots never get that permission.
pub fn merge_typed(owner: &Owner, source: &Path) -> std::result::Result<Info, pop_runtime::owner::Failure> {
    use pop_runtime::owner::{Failure, FailureKind};
    let failed = |message: String| Failure { kind: FailureKind::Failed, message, reason: None, op_index: None };
    let incoming = inspect(source).map_err(&failed)?;
    let local = owner.app();
    let store = Store::open(source, Mode::Snapshot).map_err(Failure::from)?;
    if local.document_uuid.as_deref() != Some(incoming.id.as_str()) || !compatible(&local.app, &store.app().app) {
        return Err(failed("Snapshot has a different document identity, data schema or theme palette".into()));
    }
    let doc = store.document().map_err(Failure::from)?;
    for attachment in store.attachments().map_err(Failure::from)? {
        let bytes = store.attachment(&attachment.id).map_err(Failure::from)?;
        request_typed(owner, Request::PutAttachment { bytes })?;
    }
    let bytes = doc.checkpoint().map_err(Failure::from)?;
    request_typed(owner, Request::MergeSnapshot { bytes })?;
    request_typed(owner, Request::Flush)?;
    Ok(incoming)
}

/// A receipt must never confirm history absent from the flushed owner.
pub fn require_history(owner: &Owner, source: &Path) -> Result<()> {
    let local = Store::open(owner.path(), Mode::Snapshot).map_err(|e| e.to_string())?;
    let remote = Store::open(source, Mode::Snapshot).map_err(|e| e.to_string())?;
    if !local.document().map_err(|e| e.to_string())?.includes_history(&remote.document().map_err(|e| e.to_string())?) {
        return Err("Incoming document history is incomplete; will retry".into());
    }
    Ok(())
}

/// Cosmetic bytes are independent of the Loro version. Hash only the bounded saved PNGs.
pub fn artwork_digest(store: &Store) -> Result<String> {
    let mut hash = Sha256::new();
    for name in file::Artwork::ALL {
        hash.update(name.as_str().as_bytes());
        if let Some(bytes) = store.artwork(name).map_err(|e| e.to_string())? {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    Ok(crate::hex(&hash.finalize()))
}
/// Stale fallback images do not make replicas with identical state fight over caches.
pub fn asset_revision(version: &str, artwork_version: &str, digest: &str) -> String {
    let artwork = if version == artwork_version { digest } else { "" };
    crate::hex(&Sha256::digest(crate::encode(&(version, artwork))))
}

/// UI assets and descriptions may differ without changing the state contract. Theme
/// defaults belong to that contract: stored overrides equal to a default are invalid.
/// Schema evolution still needs an explicit migration; matching current values alone is
/// insufficient because later edits must remain valid on both replicas.
pub fn compatible(left: &pop_core::app::AppDefinition, right: &pop_core::app::AppDefinition) -> bool {
    left.replica_compatible(right)
}
