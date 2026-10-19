//! Opt-in whole-file CloudKit proof. No network, CloudKit types or Swift document state.
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
}
pub fn request(owner: &Owner, request: Request) -> Result<Reply> {
    let (send, receive) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |result| {
            let _ = send.send(result);
        }),
    );
    receive.recv_timeout(Duration::from_secs(20)).map_err(|e| e.to_string())?.map_err(|e| e.to_string())
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
    if incoming.id != local.id || incoming.app_digest != local.app_digest {
        return Err("Snapshot belongs to another document or app".into());
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
