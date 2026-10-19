//! Native sharing transport over the existing owner. Swift persists opaque credentials.
mod resources;
#[cfg(test)]
mod tests;
mod transport;
use hitslop_core::{
    library::snapshot,
    owner::{Owner, Reply, Request},
    store::{Mode, Share, Store},
};
use hitslop_sharing_protocol::{
    http::{Invitation, RoomInit},
    resources::hash_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};
type Result<T> = std::result::Result<T, String>;
fn fail(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn request(owner: &Owner, request: Request) -> Result<Reply> {
    snapshot::request(owner, request)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(fail)?;
    file.write_all(bytes).map_err(fail)?;
    file.sync_all().map_err(fail)
}
fn random() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "Cannot create sharing credentials")?;
    Ok(hex(&bytes))
}

/// Auth spike only. The token is transient and never enters document credentials.
pub fn verify_account(token: &str) -> Result<hitslop_sharing_protocol::http::VerifiedAccount> {
    if token.is_empty() || token.len() > hitslop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES {
        return Err("Sign in required".into());
    }
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build(),
    );
    let mut response = agent
        .get(format!("{}/v1/auth/probe", hitslop_sharing_protocol::TRUSTED_ORIGIN))
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|_| "Identity service unavailable")?;
    if response.status().as_u16() == 401 {
        return Err("Sign in required".into());
    }
    if response.status().as_u16() != 200 {
        return Err("Identity verification unavailable".into());
    }
    let bytes =
        response.body_mut().with_config().limit(4096).read_to_vec().map_err(|_| "Identity reply unavailable")?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid identity reply".into())
}

#[derive(Clone, Serialize, Deserialize)]
struct Credential {
    invitation: Invitation,
    admin: String,
    seed: PathBuf,
    init: Option<RoomInit>,
    published: bool,
    ended: bool,
}
impl Deref for Credential {
    type Target = Invitation;
    fn deref(&self) -> &Invitation {
        &self.invitation
    }
}
impl Credential {
    fn decode(value: &str) -> Result<Self> {
        if value.len() > 16384 {
            return Err("Invalid sharing credentials".into());
        }
        let key: Self = serde_json::from_str(value).map_err(|_| "Invalid sharing credentials")?;
        if key.origin != hitslop_sharing_protocol::TRUSTED_ORIGIN
            || !hitslop_sharing_protocol::valid_copy(&key.document)
            || key.room.len() != 32
            || !key.room.bytes().all(|b| b.is_ascii_hexdigit())
            || !hash_id(&key.capability)
            || (!key.admin.is_empty() && !hash_id(&key.admin))
        {
            return Err("Untrusted sharing credentials".into());
        }
        if key.size != 0 {
            key.invitation.validate()?;
        }
        Ok(key)
    }
    fn encode(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|_| "Cannot preserve sharing credentials".into())
    }
}

#[derive(Clone, Debug)]
pub enum Phase {
    Preparing,
    Offline,
    Syncing,
    Synced,
    Paused,
    Ended,
}
pub struct ClientStatus {
    pub phase: Phase,
    pub detail: String,
    pub link: Option<String>,
    pub creator: bool,
}

pub struct Client {
    owner: Arc<Owner>,
    key: Mutex<Credential>,
    connection: Mutex<Option<transport::Connection>>,
}
impl Client {
    /// Allocate credentials first, without changing the document or contacting the service.
    pub fn prepare(owner: &Owner, root: &Path) -> Result<String> {
        if owner.share().map_err(fail)?.is_some_and(|s| s.endpoint != "cloudkit://library") {
            return Err("Sharing details are missing. Reopen the invitation or make an independent copy.".into());
        }
        let document = owner.app().document_uuid.clone().ok_or("Only documents can be shared")?;
        let room = random()?[..32].to_owned();
        std::fs::create_dir_all(root).map_err(fail)?;
        let root = std::fs::canonicalize(root).map_err(fail)?;
        private_file(&root.join(format!("{room}.staging")), b"hitSlop sharing preparation")?;
        Credential {
            invitation: Invitation {
                document,
                room: room.clone(),
                origin: hitslop_sharing_protocol::TRUSTED_ORIGIN.into(),
                capability: random()?,
                seed_hash: String::new(),
                size: 0,
            },
            admin: random()?,
            seed: root.join(format!("{room}.slop")),
            init: None,
            published: false,
            ended: false,
        }
        .encode()
    }
    /// Called only after the host has persisted the pending credentials in Keychain.
    pub fn stage(owner: &Owner, value: &str) -> Result<String> {
        let mut key = Credential::decode(value)?;
        if key.admin.is_empty() || owner.app().document_uuid.as_deref() != Some(&key.document) {
            return Err("Different sharing document".into());
        }
        // This marker is local to prepare(), never in the document or Keychain.
        // Another same-account device must not accidentally replace the pending seed.
        if !key.seed.with_extension("staging").is_file() || (key.size != 0 && !key.seed.is_file()) {
            return Err("Finish starting sharing on the original device".into());
        }
        let binding = Share { room: key.room.clone(), endpoint: key.origin.clone() };
        if owner.share().map_err(fail)?.is_some_and(|s| s.endpoint != "cloudkit://library" && s != binding) {
            return Err("Conflicting room membership".into());
        }
        request(owner, Request::Share { share: binding })?;
        if !key.seed.exists() {
            request(owner, Request::Backup { destination: key.seed.clone() })?;
        }
        let size = std::fs::metadata(&key.seed).map_err(|_| "Finish starting sharing on the original device")?.len();
        if size > hitslop_sharing_protocol::SEED_BYTES as u64 {
            return Err("This preview starts sharing files up to 16 MiB. Your local document is retained.".into());
        }
        let bytes = std::fs::read(&key.seed).map_err(fail)?;
        let store = Store::open(&key.seed, Mode::Snapshot).map_err(fail)?;
        if store.app().document_uuid.as_deref() != Some(&key.document)
            || store.share().map_err(fail)? != owner.share().map_err(fail)?
        {
            return Err("Pending seed identity mismatch".into());
        }
        let checkpoint = store.document().map_err(fail)?.checkpoint().map_err(fail)?;
        let meta = loro::LoroDoc::decode_import_blob_meta(&checkpoint, true).map_err(|_| "Invalid seed history")?;
        let mut seed: Vec<_> = meta.partial_end_vv.iter().map(|(&p, &n)| (format!("{p:016x}"), n as u32)).collect();
        seed.sort();
        key.invitation.seed_hash = hex(&Sha256::digest(&bytes));
        key.invitation.size = size as u32;
        key.init =
            Some(RoomInit { app: hex(&store.app_digest().map_err(fail)?), seed, seed_hash: key.seed_hash.clone() });
        key.encode()
    }
    pub fn restore(owner: Arc<Owner>, value: &str) -> Result<Self> {
        let key = Credential::decode(value)?;
        if owner.app().document_uuid.as_deref() != Some(&key.document) {
            return Err("Different sharing document".into());
        }
        validate_membership(owner.share().map_err(fail)?, &key)?;
        Ok(Self { owner, key: Mutex::new(key), connection: Mutex::new(None) })
    }
    /// The staged seed identity must already be persisted. Repeating this operation is safe.
    pub fn publish(&self, create_key: &str) -> Result<String> {
        let mut key = self.key.lock().expect("sharing key").clone();
        if key.ended {
            return Err("Sharing ended".into());
        }
        key.invitation.validate()?;
        let init = key.init.as_ref().ok_or("Sharing is not staged")?;
        if key.admin.is_empty() || create_key.is_empty() {
            return Err("Sharing preview creation is not configured".into());
        }
        let bytes = std::fs::read(&key.seed).map_err(|_| "Finish starting sharing on the original device")?;
        if hex(&Sha256::digest(&bytes)) != key.seed_hash {
            return Err("Pending sharing seed changed".into());
        }
        transport::http(
            &key,
            "POST",
            "init",
            None,
            &serde_json::to_vec(init).map_err(fail)?,
            &[
                ("X-Admin-Key", &key.admin),
                ("X-Write-Key", &key.capability),
                ("X-Preview-Create-Key", create_key),
                ("Content-Type", "application/json"),
            ],
        )?;
        transport::http(&key, "PUT", "seed", Some(&key.admin), &bytes, &[])?;
        let store = Store::open(&key.seed, Mode::Snapshot).map_err(fail)?;
        resources::upload_app(&key, &store, &key.seed)?;
        resources::upload_blobs(&self.owner, &key, &mut resources::remote_blobs(&key)?)?;
        key.published = true;
        let encoded = key.encode()?;
        *self.key.lock().expect("sharing key") = key;
        Ok(encoded)
    }
    pub fn connect(&self) -> Result<()> {
        self.pause();
        let key = self.key.lock().expect("sharing key").clone();
        if !key.published || key.ended {
            return Ok(());
        }
        key.invitation.validate()?;
        let binding = Share { room: key.room.clone(), endpoint: key.origin.clone() };
        if self.owner.share().map_err(fail)?.is_some_and(|s| s.endpoint != "cloudkit://library" && s != binding) {
            return Err("Conflicting room membership".into());
        }
        let bytes = match transport::http(&key, "GET", "seed", Some(&key.capability), &[], &[]) {
            Ok(bytes) => bytes,
            Err(message) if message == "Sharing ended" => {
                self.key.lock().expect("sharing key").ended = true;
                return Ok(());
            }
            Err(message) => return Err(message),
        };
        if hex(&Sha256::digest(&bytes)) != key.seed_hash {
            return Err("Seed checksum mismatch".into());
        }
        let stage = std::env::temp_dir().join(format!("hitslop-sharing-{}.slop", random()?));
        let result = (|| {
            private_file(&stage, &bytes)?;
            resources::merge_seed(&self.owner, &stage)
        })();
        let _ = std::fs::remove_file(&stage);
        result?;
        request(&self.owner, Request::Share { share: binding })?;
        *self.connection.lock().expect("sharing connection") =
            Some(transport::Connection::start(self.owner.clone(), key));
        Ok(())
    }
    pub fn pause(&self) {
        let connection = self.connection.lock().expect("sharing connection").take();
        if let Some(connection) = connection {
            connection.stop();
        }
    }
    pub fn stop(&self) -> Result<String> {
        let mut key = self.key.lock().expect("sharing key").clone();
        if key.admin.is_empty() {
            return Err("Only the creator can stop sharing".into());
        }
        transport::http(&key, "DELETE", "stop", Some(&key.admin), &[], &[])?;
        self.pause();
        key.ended = true;
        let value = key.encode()?;
        *self.key.lock().expect("sharing key") = key;
        Ok(value)
    }
    pub fn status(&self) -> ClientStatus {
        let key = self.key.lock().expect("sharing key");
        let detail =
            self.connection.lock().expect("sharing connection").as_ref().map_or("Offline".into(), |c| c.status());
        let phase = if key.ended || detail == "Ended" {
            Phase::Ended
        } else if !key.published {
            Phase::Preparing
        } else if detail == "Synced" {
            Phase::Synced
        } else if detail.starts_with("Paused") {
            Phase::Paused
        } else if detail.contains("Syncing") || detail.contains("files") {
            Phase::Syncing
        } else {
            Phase::Offline
        };
        ClientStatus {
            phase,
            detail,
            creator: !key.admin.is_empty(),
            link: if key.published { key.invitation.link().ok() } else { None },
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.pause();
    }
}

pub fn joined_credential(link: &str) -> Result<String> {
    Credential {
        invitation: Invitation::parse(link)?,
        admin: String::new(),
        seed: PathBuf::new(),
        init: None,
        published: true,
        ended: false,
    }
    .encode()
}
pub fn invitation_document(link: &str) -> Result<String> {
    Ok(Invitation::parse(link)?.document)
}
pub fn local_document(path: &Path) -> Result<Option<String>> {
    Ok(Store::open(path, Mode::Snapshot).map_err(fail)?.app().document_uuid.clone())
}
fn validate_membership(share: Option<Share>, key: &Credential) -> Result<()> {
    if share.is_some_and(|s| s.endpoint != "cloudkit://library" && (s.endpoint != key.origin || s.room != key.room)) {
        return Err("Conflicting room membership".into());
    }
    Ok(())
}
/// Joining the same invitation preserves creator authority; a different room never
/// silently replaces the saved membership of an existing document.
pub fn accept_invitation(existing: Option<&str>, link: &str) -> Result<String> {
    let incoming = Invitation::parse(link)?;
    if let Some(value) = existing {
        let saved = Credential::decode(value)?;
        if saved.document != incoming.document
            || saved.room != incoming.room
            || saved.capability != incoming.capability
            || saved.seed_hash != incoming.seed_hash
            || saved.ended
        {
            return Err("This document already has different sharing details".into());
        }
        return Ok(value.to_owned());
    }
    joined_credential(link)
}
/// Install only a missing canonical file. Existing matching files are merged after their owner opens.
pub fn install(destination: &Path, link: &str) -> Result<()> {
    let key = Credential::decode(&joined_credential(link)?)?;
    if destination.exists() {
        let store = Store::open(destination, Mode::Snapshot).map_err(fail)?;
        if store.app().document_uuid.as_deref() != Some(&key.document) {
            return Err("Existing document identity mismatch".into());
        }
        validate_membership(store.share().map_err(fail)?, &key)?;
        return Ok(());
    }
    let bytes = transport::http(&key, "GET", "seed", Some(&key.capability), &[], &[])?;
    if hex(&Sha256::digest(&bytes)) != key.seed_hash {
        return Err("Seed checksum mismatch".into());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let stage = destination.with_extension(format!("{}.stage", random()?));
    let result = (|| {
        private_file(&stage, &bytes)?;
        snapshot::install(&stage, destination, &key.document)
    })();
    let _ = std::fs::remove_file(stage);
    result
}
