//! Full-file library snapshots. CloudKit and document bytes never meet in Swift.
use crate::{
    file,
    owner::{Owner, Reply, Request},
    store::{Mode, Store},
};
use loro::ExportMode;
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
pub fn request_typed(owner: &Owner, request: Request) -> std::result::Result<Reply, crate::owner::Failure> {
    let (send, receive) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |result| {
            let _ = send.send(result);
        }),
    );
    receive
        .recv_timeout(Duration::from_secs(20))
        .map_err(|e| crate::owner::Failure::new(crate::owner::FailureKind::Failed, e.to_string()))?
}
fn bounded(path: &Path) -> Result<()> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES {
        return Err("This preview syncs documents up to 16 MiB; the local document is retained".into());
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
    let connection = file::reader(source).map_err(|e| e.to_string())?;
    let read = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    file::copy(&read, destination, false, true, None).map_err(|e| e.to_string())
}
/// Validate a complete incoming file, bring its immutable blobs across, then merge
/// history through the live owner. A download never replaces that owner's backing file.
pub fn merge(owner: &Owner, source: &Path) -> Result<Info> {
    let incoming = inspect(source)?;
    let local = inspect(owner.path())?;
    if incoming.id != local.id
        || !compatible(&owner.app().app, &Store::open(source, Mode::Snapshot).map_err(|e| e.to_string())?.app().app)
    {
        return Err("Snapshot has a different document identity, data schema or theme palette".into());
    }
    let store = Store::open(source, Mode::Snapshot).map_err(|e| e.to_string())?;
    let doc = store.document().map_err(|e| e.to_string())?;
    for attachment in store.attachments().map_err(|e| e.to_string())? {
        let bytes = store.attachment(&attachment.id).map_err(|e| e.to_string())?;
        request(owner, Request::PutAttachment { bytes })?;
    }
    let bytes = doc.doc.export(ExportMode::Snapshot).map_err(|e| e.to_string())?;
    request(owner, Request::MergeSnapshot { bytes })?;
    request(owner, Request::Flush)?;
    Ok(incoming)
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
pub fn compatible(left: &crate::app::AppDefinition, right: &crate::app::AppDefinition) -> bool {
    let tokens = |app: &crate::app::AppDefinition| {
        app.spec().theme_tokens().iter().cloned().collect::<std::collections::BTreeMap<_, _>>()
    };
    left.metadata().slug == right.metadata().slug
        && left.spec().schema == right.spec().schema
        && tokens(left) == tokens(right)
}
