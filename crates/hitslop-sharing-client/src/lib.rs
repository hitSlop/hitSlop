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
use hitslop_sharing_protocol::http::{
    AccountRoomInit, Invitation, ReserveRoom, RoomIdentity, RoomInit, RoomPhase, RoomReservation,
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
type Notice = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
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

/// Tokens are transient request arguments; never saved with a document or credential.
pub fn verify_account(token: &str) -> Result<hitslop_sharing_protocol::http::VerifiedAccount> {
    account_request("GET", "auth/probe", token, &[])
}
fn account_request<T: serde::de::DeserializeOwned>(method: &str, route: &str, token: &str, body: &[u8]) -> Result<T> {
    if token.is_empty() || token.len() > hitslop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES {
        return Err("Sign in to your sharing account first".into());
    }
    let bytes = transport::http_request(
        method,
        &format!("{}/v1/{route}", hitslop_sharing_protocol::TRUSTED_ORIGIN),
        Some(token),
        body,
        &[("Content-Type", "application/json")],
        hitslop_sharing_protocol::CONTROL_BYTES as u64,
    )?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid account reply".into())
}
fn access(document: &str, token: &str) -> Result<Option<RoomReservation>> {
    let result: Option<RoomReservation> = account_request("GET", &format!("account/access/{document}"), token, &[])?;
    if let Some(r) = &result {
        validate_reservation(r, document)?;
    }
    Ok(result)
}
fn validate_reservation(r: &RoomReservation, document: &str) -> Result<()> {
    let room =
        |value: &str| value.len() == 32 && value.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c));
    if r.document != document
        || !hitslop_sharing_protocol::valid_copy(document)
        || r.generation == 0
        || !room(&r.room)
        || r.previous_room.as_deref().is_some_and(|s| !room(s))
        || r.capability.as_deref().is_some_and(|s| !hitslop_sharing_protocol::resources::hash_id(s))
    {
        return Err("Invalid account reservation".into());
    }
    Ok(())
}
fn owner_credential(owner: &Owner, r: &RoomReservation) -> Result<Credential> {
    let key = Credential { invitation: r.invitation.clone().ok_or("Missing published invitation")?, ended: false };
    key.invitation.validate()?;
    if key.document != r.document || key.room != r.room || r.capability.as_ref() != Some(&key.capability) {
        return Err("Invalid published reservation".into());
    }
    let share = owner.share().map_err(fail)?;
    if share.as_ref().is_some_and(|s| s.endpoint == key.origin && r.previous_room.as_ref() == Some(&s.room)) {
        request(owner, Request::Share { share: Share { room: key.room.clone(), endpoint: key.origin.clone() } })?;
    } else {
        validate_membership(share, &key)?;
    }
    Ok(key)
}
fn identity(r: &RoomReservation) -> RoomIdentity {
    RoomIdentity { document: r.document.clone(), generation: r.generation, room: r.room.clone() }
}
fn validate_reservation_membership(share: Option<Share>, r: &RoomReservation) -> Result<()> {
    if share.is_some_and(|s| {
        s.endpoint != "cloudkit://library"
            && (s.endpoint != hitslop_sharing_protocol::TRUSTED_ORIGIN
                || (s.room != r.room && r.previous_room.as_ref() != Some(&s.room)))
    }) {
        return Err("Different room membership".into());
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
struct Credential {
    invitation: Invitation,
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
        key.invitation.validate()?;
        Ok(key)
    }
    fn encode(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|_| "Cannot preserve sharing credentials".into())
    }
}
/// The immutable, owner-local SQLite seed IS the creation journal. Its stable room
/// filename lets retries reconstruct metadata without a second JSON state machine.
fn stage(owner: &Owner, root: &Path, reservation: &RoomReservation) -> Result<(PathBuf, RoomInit, Invitation)> {
    validate_reservation(reservation, owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?)?;
    if owner.app().document_uuid.as_deref() != Some(&reservation.document) {
        return Err("Different sharing document".into());
    }
    let capability = reservation.capability.clone().ok_or("Sharing ended")?;
    let binding = Share { room: reservation.room.clone(), endpoint: hitslop_sharing_protocol::TRUSTED_ORIGIN.into() };
    validate_reservation_membership(owner.share().map_err(fail)?, reservation)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(root).map_err(fail)?;
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
    let path = root.join(format!("{}.slop", reservation.room));
    request(owner, Request::Share { share: binding.clone() })?;
    if !path.exists() {
        // The core already stages, fsyncs and atomically publishes without replacing
        // a destination. Reuse it instead of adding a second file publication path.
        request(owner, Request::Backup { destination: path.clone() })?;
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(fail)?;
    std::fs::File::open(&path).and_then(|f| f.sync_all()).map_err(fail)?;
    let size = std::fs::metadata(&path).map_err(fail)?.len();
    if size > hitslop_sharing_protocol::SEED_BYTES as u64 {
        return Err("Sharing starts files up to 16 MiB. Your local document is retained.".into());
    }
    let store = Store::open(&path, Mode::Snapshot).map_err(fail)?;
    if store.app().document_uuid.as_deref() != Some(&reservation.document)
        || store.share().map_err(fail)? != Some(binding)
    {
        return Err("Pending seed identity mismatch".into());
    }
    let checkpoint = store.document().map_err(fail)?.checkpoint().map_err(fail)?;
    let meta = loro::LoroDoc::decode_import_blob_meta(&checkpoint, true).map_err(|_| "Invalid seed history")?;
    let mut seed: Vec<_> = meta.partial_end_vv.iter().map(|(&p, &n)| (format!("{p:016x}"), n as u32)).collect();
    seed.sort();
    let seed_hash = hex(&Sha256::digest(std::fs::read(&path).map_err(fail)?));
    let init = RoomInit { app: hex(&store.app_digest().map_err(fail)?), seed, seed_hash: seed_hash.clone() };
    let invitation = Invitation {
        document: reservation.document.clone(),
        room: reservation.room.clone(),
        origin: hitslop_sharing_protocol::TRUSTED_ORIGIN.into(),
        capability,
        seed_hash,
        size: size as u32,
    };
    invitation.validate()?;
    Ok((path, init, invitation))
}

/// Initial seed catch-up belongs to the reconnecting transport. A document opened
/// offline must not need a UI action or a credential-poll loop to recover later.
fn bootstrap(owner: &Owner, key: &Credential) -> Result<()> {
    let bytes = transport::http(key, "GET", "seed", Some(&key.capability), &[], &[])?;
    if hex(&Sha256::digest(&bytes)) != key.seed_hash {
        return Err("Seed checksum mismatch".into());
    }
    let stage = std::env::temp_dir().join(format!("hitslop-sharing-{}.slop", random()?));
    let result = (|| {
        private_file(&stage, &bytes)?;
        resources::merge_seed(owner, &stage)
    })();
    let _ = std::fs::remove_file(&stage);
    result?;
    Ok(())
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
pub struct AccountAccess {
    pub credential: Option<String>,
    pub phase: Option<Phase>,
}

pub struct Client {
    owner: Arc<Owner>,
    key: Mutex<Credential>,
    connection: Mutex<Option<transport::Connection>>,
    creator: std::sync::atomic::AtomicBool,
    notice: Notice,
}
impl Client {
    /// Creation is idempotent across devices and process restarts. Only the server
    /// holds ownership; the local file freezes retry bytes, never admin authority.
    pub fn create(owner: &Owner, root: &Path, token: &str) -> Result<String> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let prior = access(document, token)?;
        if let Some(r) = &prior {
            if r.phase == RoomPhase::Published {
                return owner_credential(owner, r)?.encode();
            }
            if r.phase == RoomPhase::Stopping {
                return Err("Finish stopping this share before creating another".into());
            }
        }
        let generation = prior
            .as_ref()
            .map_or(
                Some(1),
                |r| if r.phase == RoomPhase::Ended { r.generation.checked_add(1) } else { Some(r.generation) },
            )
            .ok_or("Sharing generation limit reached")?;
        let reservation: RoomReservation = account_request(
            "POST",
            "account/reserve",
            token,
            &serde_json::to_vec(&ReserveRoom { document: document.into(), generation }).map_err(fail)?,
        )?;
        if reservation.phase != RoomPhase::Pending {
            return Err("Reservation changed; reopen sharing and retry".into());
        }
        let (path, init, invitation) = stage(owner, root, &reservation)?;
        let key = Credential { invitation, ended: false };
        transport::http(
            &key,
            "POST",
            "init",
            Some(token),
            &serde_json::to_vec(&AccountRoomInit { identity: identity(&reservation), init }).map_err(fail)?,
            &[("Content-Type", "application/json")],
        )?;
        let bytes = std::fs::read(&path).map_err(fail)?;
        transport::http(&key, "PUT", "seed", Some(token), &bytes, &[])?;
        let store = Store::open(&path, Mode::Snapshot).map_err(fail)?;
        resources::upload_app(&key, &store, &path, token)?;
        resources::upload_blobs(owner, &key, &mut resources::remote_blobs(&key)?)?;
        let published: RoomReservation = account_request(
            "POST",
            "account/publish",
            token,
            &serde_json::to_vec(&identity(&reservation)).map_err(fail)?,
        )?;
        let invitation = published.invitation.ok_or("Publication is not ready")?;
        // The server's canonical seed must be precisely the immutable file we staged.
        if invitation.document != key.document
            || invitation.room != key.room
            || invitation.capability != key.capability
            || invitation.origin != key.origin
            || invitation.seed_hash != key.seed_hash
            || invitation.size != key.size
        {
            return Err("Canonical seed differs; finish creation on the original device".into());
        }
        let _ = std::fs::remove_file(path);
        Credential { invitation, ended: false }.encode()
    }
    pub fn recover(owner: &Owner, token: &str) -> Result<AccountAccess> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let Some(r) = access(document, token)? else {
            return Ok(AccountAccess { credential: None, phase: None });
        };
        if r.invitation.is_some() {
            let key = owner_credential(owner, &r)?;
            return Ok(AccountAccess { credential: Some(key.encode()?), phase: Some(Phase::Offline) });
        }
        if validate_reservation_membership(owner.share().map_err(fail)?, &r).is_err() {
            return Ok(AccountAccess { credential: None, phase: None });
        }
        Ok(AccountAccess {
            credential: None,
            phase: Some(if r.phase == RoomPhase::Ended { Phase::Ended } else { Phase::Preparing }),
        })
    }
    pub fn stop_document(owner: &Owner, token: &str) -> Result<()> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let r = access(document, token)?.ok_or("Only the creator can stop this share")?;
        validate_reservation_membership(owner.share().map_err(fail)?, &r)?;
        let _: RoomReservation =
            account_request("POST", "account/stop", token, &serde_json::to_vec(&identity(&r)).map_err(fail)?)?;
        Ok(())
    }
    pub fn restore(owner: Arc<Owner>, value: &str) -> Result<Self> {
        let key = Credential::decode(value)?;
        if owner.app().document_uuid.as_deref() != Some(&key.document) {
            return Err("Different sharing document".into());
        }
        validate_membership(owner.share().map_err(fail)?, &key)?;
        Ok(Self {
            owner,
            key: Mutex::new(key),
            connection: Mutex::new(None),
            creator: std::sync::atomic::AtomicBool::new(false),
            notice: Arc::new(Mutex::new(None)),
        })
    }
    pub fn connect(&self) -> Result<()> {
        self.pause();
        let key = self.key.lock().expect("sharing key").clone();
        if key.ended {
            return Ok(());
        }
        key.invitation.validate()?;
        let binding = Share { room: key.room.clone(), endpoint: key.origin.clone() };
        if self.owner.share().map_err(fail)?.is_some_and(|s| s.endpoint != "cloudkit://library" && s != binding) {
            return Err("Conflicting room membership".into());
        }
        request(&self.owner, Request::Share { share: binding })?;
        *self.connection.lock().expect("sharing connection") =
            Some(transport::Connection::start(self.owner.clone(), key, self.notice.clone()));
        Ok(())
    }
    pub fn pause(&self) {
        let connection = self.connection.lock().expect("sharing connection").take();
        if let Some(connection) = connection {
            connection.stop();
        }
    }
    pub fn stop(&self, token: &str) -> Result<String> {
        let mut key = self.key.lock().expect("sharing key").clone();
        let r = access(&key.document, token)?.ok_or("Only the creator can stop this share")?;
        if r.room != key.room {
            return Err("This link was replaced; refresh sharing before stopping it".into());
        }
        let _: RoomReservation =
            account_request("POST", "account/stop", token, &serde_json::to_vec(&identity(&r)).map_err(fail)?)?;
        self.pause();
        key.ended = true;
        let value = key.encode()?;
        *self.key.lock().expect("sharing key") = key;
        Ok(value)
    }
    pub fn set_creator(&self, creator: bool) {
        self.creator.store(creator, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn listen(&self, notice: Arc<dyn Fn() + Send + Sync>) {
        *self.notice.lock().unwrap() = Some(notice);
    }
    pub fn status(&self) -> ClientStatus {
        let key = self.key.lock().expect("sharing key");
        let detail =
            self.connection.lock().expect("sharing connection").as_ref().map_or("Offline".into(), |c| c.status());
        let phase = if key.ended || detail == "Ended" {
            Phase::Ended
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
            creator: self.creator.load(std::sync::atomic::Ordering::Relaxed),
            link: key.invitation.link().ok(),
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.pause();
    }
}

pub fn joined_credential(link: &str) -> Result<String> {
    Credential { invitation: Invitation::parse(link)?, ended: false }.encode()
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
