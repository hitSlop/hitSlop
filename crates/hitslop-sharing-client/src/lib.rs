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
use hitslop_sharing_protocol::http::{AccountRoom, CreateRoom, CreatedRoom, Invitation, RoomInfo, RoomPhase};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
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

/// Tokens are transient request arguments, never persisted with a document.
pub fn verify_account(token: &str) -> Result<()> {
    let _: Option<AccountRoom> =
        account_request("GET", "account/rooms/00000000-0000-4000-8000-000000000000", token, &[])?;
    Ok(())
}
/// End all indexed shares and remove the relay account. Tokens are never persisted.
pub fn delete_account(token: &str) -> Result<()> {
    delete_account_at(hitslop_sharing_protocol::TRUSTED_ORIGIN, token)
}
fn delete_account_at(origin: &str, token: &str) -> Result<()> {
    account_request_at::<()>(origin, "DELETE", "account", token, &[])
}
fn account_request<T: serde::de::DeserializeOwned>(method: &str, route: &str, token: &str, body: &[u8]) -> Result<T> {
    account_request_at(hitslop_sharing_protocol::TRUSTED_ORIGIN, method, route, token, body)
}
fn account_request_at<T: serde::de::DeserializeOwned>(
    origin: &str,
    method: &str,
    route: &str,
    token: &str,
    body: &[u8],
) -> Result<T> {
    if token.is_empty() || token.len() > hitslop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES {
        return Err("Sign in to your sharing account first".into());
    }
    let bytes = transport::http_request(
        method,
        &format!("{origin}/v1/{route}"),
        Some(token),
        body,
        &[("Content-Type", "application/json")],
        hitslop_sharing_protocol::CONTROL_BYTES as u64,
    )?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid account reply".into())
}
/// Identify locally saved credentials belonging to this account before deletion.
/// An unreadable saved credential is not owned; relay lookup failures still propagate.
pub fn account_owns(document: &str, credential: &str, token: &str) -> Result<bool> {
    if !hitslop_sharing_protocol::valid_copy(document) {
        return Ok(false);
    }
    let Ok(saved) = Credential::decode(credential).and_then(|saved| saved.runtime()) else {
        return Ok(false);
    };
    Ok(access(document, token)?.is_some_and(|room| room.room == saved.room))
}
fn access(document: &str, token: &str) -> Result<Option<AccountRoom>> {
    if !hitslop_sharing_protocol::valid_copy(document) {
        return Err("Invalid document identity".into());
    }
    let result: Option<AccountRoom> = account_request("GET", &format!("account/rooms/{document}"), token, &[])?;
    if result.as_ref().is_some_and(|r| !hitslop_sharing_protocol::valid_room(&r.room)) {
        return Err("Invalid account room".into());
    }
    Ok(result)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Credential {
    link: String,
    ended: bool,
}
impl Credential {
    fn decode(value: &str) -> Result<Self> {
        if value.len() > 1024 {
            return Err("Sharing link is no longer valid".into());
        }
        let key: Self = serde_json::from_str(value).map_err(|_| "Sharing link is no longer valid")?;
        Invitation::parse(&key.link)?;
        Ok(key)
    }
    fn encode(&self) -> Result<String> {
        serde_json::to_string(self).map_err(fail)
    }
    fn runtime(&self) -> Result<RoomKey> {
        let invitation = Invitation::parse(&self.link)?;
        Ok(RoomKey {
            room: invitation.room,
            capability: invitation.capability,
            origin: hitslop_sharing_protocol::TRUSTED_ORIGIN.into(),
        })
    }
}
/// Transient transport addressing. Only the pinned-origin link is persisted.
#[derive(Clone)]
struct RoomKey {
    room: String,
    capability: String,
    origin: String,
}
fn room_info(key: &RoomKey, token: Option<&str>) -> std::result::Result<RoomInfo, transport::HttpFailure> {
    let bytes = transport::http_typed(key, "GET", "", token.or(Some(&key.capability)), &[], &[])?;
    let info: RoomInfo = serde_json::from_slice(&bytes).map_err(|_| "Invalid room reply")?;
    if !hitslop_sharing_protocol::valid_copy(&info.document)
        || info.seed_hash.as_deref().is_some_and(|h| !hitslop_sharing_protocol::resources::hash_id(h))
        || info.app.as_deref().is_some_and(|h| !hitslop_sharing_protocol::resources::hash_id(h))
        || info.seed_size.is_some_and(|n| n < 100 || n as usize > hitslop_sharing_protocol::SEED_BYTES)
        || info.capability.as_deref().is_some_and(|c| c != key.capability)
    {
        return Err("Invalid room metadata".into());
    }
    Ok(info)
}
fn bind_owner(owner: &Owner, key: &RoomKey, document: &str) -> Result<()> {
    if owner.app().document_uuid.as_deref() != Some(document) {
        return Err("Different sharing document".into());
    }
    if owner.share().map_err(fail)?.is_some_and(|s| {
        s.endpoint != hitslop_core::LIBRARY_SHARE_ENDPOINT && s.endpoint != hitslop_sharing_protocol::TRUSTED_ORIGIN
    }) {
        return Err("Conflicting room membership".into());
    }
    request(owner, Request::Share { share: Share { room: key.room.clone(), endpoint: key.origin.clone() } })?;
    Ok(())
}
fn owner_credential(owner: &Owner, room: &str, token: &str) -> Result<Option<(Credential, RoomInfo)>> {
    // The room owner is authenticated by Firebase; its capability is recovered here.
    let bytes = match transport::http_request_typed(
        "GET",
        &format!("{}/v1/rooms/{room}", hitslop_sharing_protocol::TRUSTED_ORIGIN),
        Some(token),
        &[],
        &[],
        hitslop_sharing_protocol::CONTROL_BYTES as u64,
    ) {
        Ok(bytes) => bytes,
        Err(transport::HttpFailure::Ended) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let info: RoomInfo = serde_json::from_slice(&bytes).map_err(|_| "Invalid room reply")?;
    if owner.app().document_uuid.as_deref() != Some(&info.document) {
        return Err("Different sharing document".into());
    }
    let invitation = Invitation {
        room: room.into(),
        capability: info.capability.clone().ok_or("Only the creator can recover this share")?,
    };
    let key = Credential { link: invitation.link()?, ended: info.phase == RoomPhase::Ended };
    if info.phase == RoomPhase::Live {
        bind_owner(owner, &key.runtime()?, &info.document)?;
    }
    Ok(Some((key, info)))
}
/// Every attempt takes a fresh, private backup. An Uploading room accepts replacement.
struct SeedBackup {
    path: std::path::PathBuf,
    version: Vec<(String, u32)>,
    app: String,
    hash: String,
}
fn stage(owner: &Owner) -> Result<SeedBackup> {
    let directory = std::env::temp_dir().join(format!("hitslop-sharing-{}", random()?));
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(&directory).map_err(fail)?;
    let path = directory.join("seed.slop");
    let result = (|| {
        request(owner, Request::Backup { destination: path.clone() })?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(fail)?;
        if std::fs::metadata(&path).map_err(fail)?.len() > hitslop_sharing_protocol::SEED_BYTES as u64 {
            return Err("Sharing starts files up to 16 MiB. Your local document is retained.".into());
        }
        let store = Store::open(&path, Mode::Snapshot).map_err(fail)?;
        let checkpoint = store.document().map_err(fail)?.checkpoint().map_err(fail)?;
        let meta = loro::LoroDoc::decode_import_blob_meta(&checkpoint, true).map_err(|_| "Invalid seed history")?;
        let mut seed: Vec<_> = meta.partial_end_vv.iter().map(|(&p, &n)| (format!("{p:016x}"), n as u32)).collect();
        seed.sort();
        Ok(SeedBackup {
            path: path.clone(),
            version: seed,
            app: hex(&store.app_digest().map_err(fail)?),
            hash: hex(&Sha256::digest(std::fs::read(&path).map_err(fail)?)),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&directory);
    }
    result
}

/// Initial seed catch-up belongs to the reconnecting transport. A document opened
/// offline must not need a UI action or a credential-poll loop to recover later.
fn bootstrap(owner: &Owner, key: &RoomKey) -> std::result::Result<(), transport::HttpFailure> {
    let info = room_info(key, None)?;
    if info.phase != RoomPhase::Live {
        return Err("Sharing is not live".into());
    }
    if owner.app().document_uuid.as_deref() != Some(&info.document) {
        return Err("Different sharing document".into());
    }
    let bytes = transport::http_typed(key, "GET", "seed", Some(&key.capability), &[], &[])?;
    if Some(hex(&Sha256::digest(&bytes))) != info.seed_hash || Some(bytes.len() as u32) != info.seed_size {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Preparing,
    Offline,
    Syncing,
    Synced,
    Paused,
    Ended,
}
pub use hitslop_core::sync::PauseKind;
pub struct ClientStatus {
    pub pause_kind: Option<PauseKind>,
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
    /// The account index makes creation retryable from any device of the owner.
    pub fn create(owner: &Owner, token: &str) -> Result<String> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let room: CreatedRoom = account_request(
            "POST",
            "rooms",
            token,
            &serde_json::to_vec(&CreateRoom { document: document.into() }).map_err(fail)?,
        )?;
        let key =
            Credential { link: Invitation { room: room.room, capability: room.capability }.link()?, ended: false };
        let runtime = key.runtime()?;
        if room.phase == RoomPhase::Live {
            let info = room_info(&runtime, Some(token))?;
            bind_owner(owner, &runtime, &info.document)?;
            return key.encode();
        }
        if room.phase != RoomPhase::Uploading {
            return Err("Sharing ended".into());
        }
        let SeedBackup { path, version, app, hash } = stage(owner)?;
        let result = (|| {
            let bytes = std::fs::read(&path).map_err(fail)?;
            let version = serde_json::to_string(&version).map_err(fail)?;
            transport::http(
                &runtime,
                "PUT",
                "seed",
                Some(token),
                &bytes,
                &[("X-Sharing-Version", &version), ("X-Sharing-App", &app), ("X-Sharing-SHA256", &hash)],
            )?;
            let store = Store::open(&path, Mode::Snapshot).map_err(fail)?;
            resources::upload_app(&runtime, &store, &path, token)?;
            resources::upload_initial_blobs(&runtime, &store, token)?;
            transport::http(&runtime, "POST", "open", Some(token), &[], &[])?;
            bind_owner(owner, &runtime, document)?;
            key.encode()
        })();
        let _ = std::fs::remove_file(&path);
        if let Some(directory) = path.parent() {
            let _ = std::fs::remove_dir(directory);
        }
        result
    }
    pub fn recover(owner: &Owner, token: &str) -> Result<AccountAccess> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let Some(room) = access(document, token)? else {
            return Ok(AccountAccess { credential: None, phase: None });
        };
        let Some((key, info)) = owner_credential(owner, &room.room, token)? else {
            return Ok(AccountAccess { credential: None, phase: Some(Phase::Ended) });
        };
        Ok(AccountAccess {
            credential: if info.phase == RoomPhase::Live { Some(key.encode()?) } else { None },
            phase: Some(match info.phase {
                RoomPhase::Uploading => Phase::Preparing,
                RoomPhase::Live => Phase::Offline,
                RoomPhase::Ended => Phase::Ended,
            }),
        })
    }
    pub fn stop_document(owner: &Owner, token: &str) -> Result<()> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let room = access(document, token)?.ok_or("Only the creator can stop this share")?;
        let Some((key, _)) = owner_credential(owner, &room.room, token)? else {
            return Ok(());
        };
        transport::http(&key.runtime()?, "DELETE", "", Some(token), &[], &[])?;
        Ok(())
    }
    pub fn restore(owner: Arc<Owner>, value: &str) -> Result<Self> {
        let key = Credential::decode(value)?;
        validate_membership(owner.share().map_err(fail)?, &key.runtime()?)?;
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
        let key = key.runtime()?;
        let binding = Share { room: key.room.clone(), endpoint: key.origin.clone() };
        if self
            .owner
            .share()
            .map_err(fail)?
            .is_some_and(|s| s.endpoint != hitslop_core::LIBRARY_SHARE_ENDPOINT && s != binding)
        {
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
        transport::http(&key.runtime()?, "DELETE", "", Some(token), &[], &[])?;
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
        let (phase, detail, pause_kind) = self
            .connection
            .lock()
            .expect("sharing connection")
            .as_ref()
            .map_or((Phase::Offline, "Offline".into(), None), |c| c.status());
        let (phase, detail) = if key.ended { (Phase::Ended, "Ended".into()) } else { (phase, detail) };
        ClientStatus {
            pause_kind,
            phase,
            detail,
            creator: self.creator.load(std::sync::atomic::Ordering::Relaxed),
            link: Some(key.link.clone()),
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.pause();
    }
}

pub fn joined_credential(link: &str) -> Result<String> {
    Credential { link: Invitation::parse(link)?.link()?, ended: false }.encode()
}
pub fn invitation_document(link: &str) -> Result<String> {
    let key = Credential::decode(&joined_credential(link)?)?.runtime()?;
    let info = room_info(&key, None)?;
    if info.phase != RoomPhase::Live {
        return Err("Sharing is not live".into());
    }
    Ok(info.document)
}
pub fn local_document(path: &Path) -> Result<Option<String>> {
    Ok(Store::open(path, Mode::Snapshot).map_err(fail)?.app().document_uuid.clone())
}
fn validate_membership(share: Option<Share>, key: &RoomKey) -> Result<()> {
    if share.is_some_and(|s| {
        s.endpoint != hitslop_core::LIBRARY_SHARE_ENDPOINT && (s.endpoint != key.origin || s.room != key.room)
    }) {
        return Err("Conflicting room membership".into());
    }
    Ok(())
}
/// Replace credentials only after the relay confirms the predecessor ended.
pub fn accept_invitation(existing: Option<&str>, link: &str) -> Result<String> {
    accept_with(existing, link, |key| room_info(key, None))
}
fn accept_with(
    existing: Option<&str>,
    link: &str,
    lookup: impl Fn(&RoomKey) -> std::result::Result<RoomInfo, transport::HttpFailure>,
) -> Result<String> {
    let incoming = joined_credential(link)?;
    if let Some(value) = existing {
        let saved = Credential::decode(value)?;
        let next = Credential::decode(&incoming)?;
        if saved.link == next.link && !saved.ended {
            return Ok(value.to_owned());
        }
        match lookup(&saved.runtime()?) {
            Err(transport::HttpFailure::Ended) => {}
            Err(error) => return Err(error.into()),
            Ok(_) => return Err("This document is still shared with another live link. Stop that share first.".into()),
        }
    }
    Ok(incoming)
}
fn bind_invitation(
    owner: &Owner,
    key: &RoomKey,
    info: &RoomInfo,
    lookup: impl Fn(&RoomKey) -> std::result::Result<RoomInfo, transport::HttpFailure>,
) -> Result<()> {
    if info.phase != RoomPhase::Live {
        return Err("Sharing is not live".into());
    }
    if owner.app().document_uuid.as_deref() != Some(&info.document) {
        return Err("Different sharing document".into());
    }
    if let Some(old) = owner.share().map_err(fail)?
        && old.endpoint != hitslop_core::LIBRARY_SHARE_ENDPOINT
        && (old.room != key.room || old.endpoint != key.origin)
    {
        if old.endpoint != hitslop_sharing_protocol::TRUSTED_ORIGIN {
            return Err("This document already uses another sharing service".into());
        }
        // Ended rooms return 410 before capability authorization. This also
        // works for a local replica whose device has no saved credential.
        let old = RoomKey { room: old.room, origin: old.endpoint, capability: key.capability.clone() };
        match lookup(&old) {
            Err(transport::HttpFailure::Ended) => {}
            _ => return Err("Could not confirm the previous link has ended. Try again.".into()),
        }
    }
    bind_owner(owner, key, &info.document)
}
/// Rebind through the serial owner, preserving all current edits and the local identity.
pub fn join(owner: &Owner, existing: Option<&str>, link: &str) -> Result<String> {
    join_at(owner, existing, link, hitslop_sharing_protocol::TRUSTED_ORIGIN)
}
fn join_at(owner: &Owner, existing: Option<&str>, link: &str, origin: &str) -> Result<String> {
    let lookup = |key: &RoomKey| room_info(&RoomKey { origin: origin.into(), ..key.clone() }, None);
    let key = Credential::decode(&joined_credential(link)?)?.runtime()?;
    let info = lookup(&key)?;
    let saved = accept_with(existing, link, lookup)?;
    bind_invitation(owner, &key, &info, lookup)?;
    Ok(saved)
}
/// Install only a missing canonical file. Matching existing files merge after opening.
pub fn install(destination: &Path, link: &str) -> Result<()> {
    let key = Credential::decode(&joined_credential(link)?)?.runtime()?;
    let info = room_info(&key, None)?;
    if info.phase != RoomPhase::Live {
        return Err("Sharing is not live".into());
    }
    if destination.exists() {
        let store = Store::open(destination, Mode::Snapshot).map_err(fail)?;
        if store.app().document_uuid.as_deref() != Some(&info.document) {
            return Err("Existing document identity mismatch".into());
        }
        validate_membership(store.share().map_err(fail)?, &key)?;
        return Ok(());
    }
    let bytes = transport::http(&key, "GET", "seed", Some(&key.capability), &[], &[])?;
    if Some(hex(&Sha256::digest(&bytes))) != info.seed_hash || Some(bytes.len() as u32) != info.seed_size {
        return Err("Seed checksum mismatch".into());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let stage = destination.with_extension(format!("{}.stage", random()?));
    let result = (|| {
        private_file(&stage, &bytes)?;
        // A re-share seed may still contain its predecessor's membership. Bind the
        // new replica through its owner before callers restore the new credential.
        let owner = Owner::open(&stage, Mode::Document, Arc::new(|_| {})).map_err(fail)?;
        let binding =
            request(&owner, Request::Share { share: Share { room: key.room.clone(), endpoint: key.origin.clone() } });
        let closed = request(&owner, Request::Close { preview: None, icon: None });
        binding?;
        closed?;
        snapshot::install(&stage, destination, &info.document)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(stage);
    result
}
