//! Native sharing transport over the existing owner. Swift persists opaque credentials.
use pop_library::snapshot;
mod epochs;
mod handoff;
pub use handoff::finalize_catalog_handoff;
mod journal;
mod preparation;
pub use preparation::{ActivatedRoom, PreparedRoom, cancel_preparation, reserve};
mod recovery;
pub use recovery::recovered_documents;
mod resolution;
mod resources;
pub use resolution::{ResolvedRoom, resolve};
#[cfg(test)]
mod tests;
mod transport;
use pop_runtime::owner::{Owner, Reply, Request};
use pop_sharing_protocol::http::{AccountRoom, CreateRoom, CreatedRoom, Invitation, RoomInfo, RoomPhase};
use pop_storage::store::{Mode, Share, Store};
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
fn request(owner: &Owner, request: Request) -> std::result::Result<Reply, transport::HttpFailure> {
    transport::owner_request(owner, request)
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
    delete_account_at(pop_sharing_protocol::TRUSTED_ORIGIN, token)
}
fn delete_account_at(origin: &str, token: &str) -> Result<()> {
    account_request_at::<()>(origin, "DELETE", "account", token, &[])
}
fn account_request<T: serde::de::DeserializeOwned>(method: &str, route: &str, token: &str, body: &[u8]) -> Result<T> {
    account_request_at(pop_sharing_protocol::TRUSTED_ORIGIN, method, route, token, body)
}
fn account_request_at<T: serde::de::DeserializeOwned>(
    origin: &str,
    method: &str,
    route: &str,
    token: &str,
    body: &[u8],
) -> Result<T> {
    if token.is_empty() || token.len() > pop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES {
        return Err("Sign in to your sharing account first".into());
    }
    let bytes = transport::http_request(
        method,
        &format!("{origin}/v1/{route}"),
        Some(token),
        body,
        &[("Content-Type", "application/json")],
        pop_sharing_protocol::CONTROL_BYTES as u64,
    )?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid account reply".into())
}
/// Identify locally saved credentials belonging to this account before deletion.
/// An unreadable saved credential is not owned; relay lookup failures still propagate.
pub fn account_owns(document: &str, credential: &str, token: &str) -> Result<bool> {
    if !pop_sharing_protocol::valid_copy(document) {
        return Ok(false);
    }
    let Ok(saved) = Credential::decode(credential).and_then(|saved| saved.runtime()) else {
        return Ok(false);
    };
    Ok(access(document, token)?.is_some_and(|room| room.room == saved.room))
}
fn access(document: &str, token: &str) -> Result<Option<AccountRoom>> {
    if !pop_sharing_protocol::valid_copy(document) {
        return Err("Invalid document identity".into());
    }
    let result: Option<AccountRoom> = account_request("GET", &format!("account/rooms/{document}"), token, &[])?;
    if result.as_ref().is_some_and(|r| !pop_sharing_protocol::valid_room(&r.room)) {
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
            origin: pop_sharing_protocol::TRUSTED_ORIGIN.into(),
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
    if !pop_sharing_protocol::valid_copy(&info.document)
        || info.seed_hash.as_deref().is_some_and(|h| !pop_sharing_protocol::resources::hash_id(h))
        || info.app.as_deref().is_some_and(|h| !pop_sharing_protocol::resources::hash_id(h))
        || info.seed_size.is_some_and(|n| n < 100 || n as usize > pop_sharing_protocol::SEED_BYTES)
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
        s.endpoint != pop_storage::LIBRARY_SHARE_ENDPOINT && s.endpoint != pop_sharing_protocol::TRUSTED_ORIGIN
    }) {
        return Err("Conflicting room membership".into());
    }
    request(owner, Request::Share { share: Share { room: key.room.clone(), endpoint: key.origin.clone() } })?;
    Ok(())
}
fn owner_credential(owner: &Owner, room: &str, token: &str) -> Result<Option<(Credential, RoomInfo)>> {
    let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
    let result = creator_credential_at(document, room, token, pop_sharing_protocol::TRUSTED_ORIGIN)?;
    if let Some((credential, info)) = &result
        && info.phase == RoomPhase::Live
    {
        bind_owner(owner, &credential.runtime()?, &info.document)?;
    }
    Ok(result)
}
fn creator_credential_at(
    document: &str,
    room: &str,
    token: &str,
    origin: &str,
) -> Result<Option<(Credential, RoomInfo)>> {
    // The room owner is authenticated by Firebase; its capability is recovered here.
    let bytes = match transport::http_request_typed(
        "GET",
        &format!("{origin}/v1/rooms/{room}"),
        Some(token),
        &[],
        &[],
        pop_sharing_protocol::CONTROL_BYTES as u64,
    ) {
        Ok(bytes) => bytes,
        Err(transport::HttpFailure::Ended) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let info: RoomInfo = serde_json::from_slice(&bytes).map_err(|_| "Invalid room reply")?;
    if info.document != document {
        return Err("Different sharing document".into());
    }
    let invitation = Invitation {
        room: room.into(),
        capability: info.capability.clone().ok_or("Only the creator can recover this share")?,
    };
    let key = Credential { link: invitation.link()?, ended: info.phase == RoomPhase::Ended };
    Ok(Some((key, info)))
}
/// Every attempt stages a fresh accepted epoch, without stale artwork or unreferenced
/// attachments. An Uploading room accepts replacement before becoming live.
struct SeedBackup {
    path: std::path::PathBuf,
    version: Vec<(String, u32)>,
    app: String,
    hash: String,
    source_version: String,
    epoch_version: String,
}
fn stage(owner: &Owner) -> Result<SeedBackup> {
    stage_required(owner, None).map_err(Into::into)
}
fn stage_required(
    owner: &Owner,
    required_heads: Option<pop_sharing_protocol::lifecycle::WireVersion>,
) -> std::result::Result<SeedBackup, transport::HttpFailure> {
    let directory = std::env::temp_dir().join(format!("pop-sharing-{}", random()?));
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(&directory).map_err(epochs::local)?;
    let path = directory.join("seed.pop");
    let result = (|| {
        let Reply::State { reading, .. } = request(owner, Request::State)? else {
            return Err(epochs::local("Owner did not provide the epoch source version"));
        };
        let Reply::EpochCandidate { path: staged, source_version, version: epoch_version } =
            epochs::retry_busy(owner, || Request::PrepareEpoch {
                destination: path.clone().into(),
                expected_version: reading.version.clone(),
                required_heads: required_heads.clone(),
            })?
        else {
            return Err(epochs::local("Owner did not prepare an epoch"));
        };
        if staged.native_path() != path.as_path() {
            return Err(epochs::local("Owner returned a different epoch destination"));
        }
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(epochs::local)?;
        if std::fs::metadata(&path).map_err(epochs::local)?.len() > pop_sharing_protocol::SEED_BYTES as u64 {
            return Err(transport::HttpFailure::Paused(pop_core::sync::Pause {
                kind: PauseKind::Outgrown,
                reason:
                    "The latest accepted document exceeds the 16 MiB sharing limit. Your local document is retained."
                        .into(),
            }));
        }
        let store = Store::open(&path, Mode::Snapshot).map_err(epochs::local)?;
        let seed = store.document().map_err(epochs::local)?.wire_history();
        Ok(SeedBackup {
            path: path.clone(),
            source_version,
            epoch_version,
            version: seed,
            app: hex(&store.app_digest().map_err(epochs::local)?),
            hash: hex(&Sha256::digest(std::fs::read(&path).map_err(epochs::local)?)),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&directory);
    }
    result
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
pub use pop_core::sync::PauseKind;
pub struct ClientStatus {
    pub pause_kind: Option<PauseKind>,
    pub phase: Phase,
    pub detail: String,
    pub link: Option<String>,
    pub creator: bool,
    pub recovery_path: Option<String>,
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
    access: Arc<epochs::Access>,
}
impl Client {
    pub fn recover(owner: &Owner, token: &str) -> Result<AccountAccess> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let Some(room) = access(document, token)? else {
            return Ok(AccountAccess { credential: None, phase: None });
        };
        let Some((key, info)) = owner_credential(owner, &room.room, token)? else {
            return Ok(AccountAccess { credential: None, phase: Some(Phase::Ended) });
        };
        let frozen = info.phase == RoomPhase::Live && resolution::epoch(&key.runtime()?, token)?.handoff.is_some();
        Ok(AccountAccess {
            credential: if info.phase == RoomPhase::Live { Some(key.encode()?) } else { None },
            phase: Some(match info.phase {
                RoomPhase::Uploading => Phase::Preparing,
                RoomPhase::Live if frozen => Phase::Paused,
                RoomPhase::Live => Phase::Offline,
                RoomPhase::Ended => Phase::Ended,
            }),
        })
    }
    pub fn restore(owner: Arc<Owner>, value: &str, state_directory: &Path, identity: Option<&str>) -> Result<Self> {
        let key = Credential::decode(value)?;
        validate_membership(owner.share().map_err(fail)?, &key.runtime()?)?;
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can be shared")?;
        let journal = journal::Journal::new(state_directory, &key.runtime()?, document)?;
        Ok(Self {
            owner,
            key: Mutex::new(key),
            connection: Mutex::new(None),
            creator: std::sync::atomic::AtomicBool::new(false),
            notice: Arc::new(Mutex::new(None)),
            access: Arc::new(epochs::Access { journal: Arc::new(journal), identity: identity.map(str::to_owned) }),
        })
    }
    pub fn connect(&self) -> Result<()> {
        self.pause();
        let key = self.key.lock().expect("sharing key").clone();
        if key.ended {
            return Ok(());
        }
        let key = key.runtime()?;
        if self.access.identity.is_none() && self.access.journal.frozen_handoff()? {
            return Ok(());
        }
        let binding = Share { room: key.room.clone(), endpoint: key.origin.clone() };
        if self
            .owner
            .share()
            .map_err(fail)?
            .is_some_and(|s| s.endpoint != pop_storage::LIBRARY_SHARE_ENDPOINT && s != binding)
        {
            return Err("Conflicting room membership".into());
        }
        request(&self.owner, Request::Share { share: binding })?;
        *self.connection.lock().expect("sharing connection") =
            Some(transport::Connection::start(self.owner.clone(), key, self.notice.clone(), self.access.clone()));
        Ok(())
    }
    pub fn pause(&self) {
        let connection = self.connection.lock().expect("sharing connection").take();
        if let Some(connection) = connection {
            connection.stop();
        }
    }
    pub fn set_creator(&self, creator: bool) {
        self.creator.store(creator, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn listen(&self, notice: Arc<dyn Fn() + Send + Sync>) {
        *self.notice.lock().unwrap() = Some(notice);
    }
    pub fn status(&self) -> ClientStatus {
        let key = self.key.lock().expect("sharing key");
        let (mut phase, mut detail, mut pause_kind) = self
            .connection
            .lock()
            .expect("sharing connection")
            .as_ref()
            .map_or((Phase::Offline, "Offline".into(), None), |c| c.status());
        let frozen_pending = self.access.journal.frozen_handoff().unwrap_or(false);
        if !key.ended && self.access.identity.is_none() && frozen_pending {
            phase = Phase::Paused;
            pause_kind = Some(PauseKind::Refused);
            detail = "Sign in to the creator's sharing account to finish this frozen handoff.".into();
        }
        let (phase, detail) = if key.ended { (Phase::Ended, "Ended".into()) } else { (phase, detail) };
        let recovery_path = self.access.journal.completed_recovery().map(|path| path.to_string_lossy().into_owned());
        ClientStatus {
            recovery_path,
            pause_kind,
            phase,
            detail,
            creator: self.creator.load(std::sync::atomic::Ordering::Relaxed) || frozen_pending,
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
        s.endpoint != pop_storage::LIBRARY_SHARE_ENDPOINT && (s.endpoint != key.origin || s.room != key.room)
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
        && old.endpoint != pop_storage::LIBRARY_SHARE_ENDPOINT
        && (old.room != key.room || old.endpoint != key.origin)
    {
        if old.endpoint != pop_sharing_protocol::TRUSTED_ORIGIN {
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
    join_at(owner, existing, link, pop_sharing_protocol::TRUSTED_ORIGIN)
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
pub fn install(destination: &Path, link: &str, state_directory: &Path) -> Result<()> {
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
    let epoch = resolution::epoch(&key, &key.capability)?;
    let credential = joined_credential(link)?;
    let resolved = resolution::download(key.clone(), info, epoch, credential, &key.capability)?;
    resolved.install(destination, state_directory)
}
fn install_seed(destination: &Path, key: &RoomKey, info: &RoomInfo, bytes: &[u8]) -> Result<()> {
    if Some(hex(&Sha256::digest(bytes))) != info.seed_hash || Some(bytes.len() as u32) != info.seed_size {
        return Err("Seed checksum mismatch".into());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let stage = destination.with_file_name(format!(".pop-sharing-{}.pop", random()?));
    let result = (|| {
        private_file(&stage, bytes)?;
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
