//! Persistent cloud document synchronization and explicit anonymous invitations.
mod cloud;
mod epochs;
mod journal;
pub mod local;
mod recovery;
mod resolution;
mod resources;
#[cfg(test)]
mod tests;
mod transport;
pub use cloud::{
    access, authorized_access, bootstrap, delete_document, document_metadata, join_member, leave,
    ready_document_metadata, registered_documents, set_sharing, sharing_link, update_document,
};
use pop_runtime::owner::{Owner, Reply, Request};
use pop_sharing_protocol::http::{Invitation, RoomInfo, RoomPhase};
use pop_storage::store::{Mode, Share, Store};
pub use recovery::recovered_documents;
pub use resolution::{ResolvedRoom, resolve};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
pub type Result<T> = std::result::Result<T, String>;
type Notice = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
fn fail(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn request(owner: &Owner, request: Request) -> std::result::Result<Reply, transport::HttpFailure> {
    transport::owner_request(owner, request)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn random() -> Result<String> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "Cannot generate document identity")?;
    Ok(hex(&bytes))
}
fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(fail)?;
    file.write_all(bytes).map_err(fail)?;
    file.sync_all().map_err(fail)
}
#[derive(Clone)]
enum Authorization {
    Capability(String),
    Identity(String),
}
#[derive(Clone)]
struct RoomKey {
    room: String,
    authorization: Authorization,
    origin: String,
}
impl RoomKey {
    fn bearer(&self) -> &str {
        match &self.authorization {
            Authorization::Capability(value) | Authorization::Identity(value) => value,
        }
    }
    fn capability(&self) -> Option<&str> {
        match &self.authorization {
            Authorization::Capability(value) => Some(value),
            Authorization::Identity(_) => None,
        }
    }
    fn identity(document: &str, token: &str) -> Result<Self> {
        if !pop_sharing_protocol::valid_copy(document)
            || token.is_empty()
            || token.len() > pop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES
        {
            return Err("Invalid cloud document access".into());
        }
        Ok(Self {
            room: document.into(),
            authorization: Authorization::Identity(token.into()),
            origin: pop_sharing_protocol::TRUSTED_ORIGIN.into(),
        })
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GuestCredential {
    link: String,
    ended: bool,
}
#[derive(Clone)]
enum Credential {
    Guest(GuestCredential),
    Cloud { document: String, identity: String },
}
impl Credential {
    fn decode(value: &str) -> Result<Self> {
        if value.len() > 1024 {
            return Err("Invalid invitation credential".into());
        }
        let guest: GuestCredential = serde_json::from_str(value).map_err(fail)?;
        Invitation::parse(&guest.link)?;
        Ok(Self::Guest(guest))
    }
    fn runtime(&self) -> Result<RoomKey> {
        match self {
            Self::Cloud { document, identity } => RoomKey::identity(document, identity),
            Self::Guest(guest) => {
                let invitation = Invitation::parse(&guest.link)?;
                Ok(RoomKey {
                    room: invitation.room,
                    authorization: Authorization::Capability(invitation.capability),
                    origin: pop_sharing_protocol::TRUSTED_ORIGIN.into(),
                })
            }
        }
    }
    fn ended(&self) -> bool {
        matches!(self, Self::Guest(GuestCredential { ended: true, .. }))
    }
    fn link(&self) -> Option<String> {
        match self {
            Self::Guest(guest) => Some(guest.link.clone()),
            Self::Cloud { .. } => None,
        }
    }
}
fn room_info(key: &RoomKey, token: Option<&str>) -> std::result::Result<RoomInfo, transport::HttpFailure> {
    let bytes = transport::http_typed(key, "GET", "", Some(token.unwrap_or(key.bearer())), &[], &[])?;
    let info: RoomInfo = serde_json::from_slice(&bytes).map_err(fail)?;
    if !pop_sharing_protocol::valid_copy(&info.document)
        || info.document != key.room
        || info.seed_hash.as_deref().is_some_and(|hash| !pop_sharing_protocol::resources::hash_id(hash))
        || info.seed_size.is_some_and(|size| size as usize > pop_sharing_protocol::SEED_BYTES)
        || key.capability().is_some_and(|cap| info.capability.as_deref().is_some_and(|actual| actual != cap))
    {
        return Err("Invalid cloud document metadata".into());
    }
    Ok(info)
}
fn validate_membership(share: Option<Share>, key: &RoomKey) -> Result<()> {
    if share.is_some_and(|share| {
        share.endpoint != pop_storage::LIBRARY_SHARE_ENDPOINT
            && (share.endpoint != key.origin || share.room != key.room)
    }) {
        return Err("Conflicting cloud document membership".into());
    }
    Ok(())
}
fn bind_owner(owner: &Owner, key: &RoomKey, document: &str) -> Result<()> {
    if owner.app().document_uuid.as_deref() != Some(document) {
        return Err("Different cloud document".into());
    }
    validate_membership(owner.share().map_err(fail)?, key)?;
    epochs::retry_busy(owner, || Request::Share {
        share: Share { room: key.room.clone(), endpoint: key.origin.clone() },
    })?;
    Ok(())
}
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
pub struct Client {
    owner: Arc<Owner>,
    key: Mutex<Credential>,
    connection: Mutex<Option<transport::Connection>>,
    creator: std::sync::atomic::AtomicBool,
    notice: Notice,
    access: Arc<epochs::Access>,
}
impl Client {
    fn new(owner: Arc<Owner>, key: Credential, state_directory: &Path) -> Result<Self> {
        let runtime = key.runtime()?;
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can synchronize")?;
        if document != runtime.room {
            return Err("Different cloud document".into());
        }
        validate_membership(owner.share().map_err(fail)?, &runtime)?;
        let identity = match &key {
            Credential::Cloud { identity, .. } => Some(identity.clone()),
            Credential::Guest(_) => None,
        };
        let journal = Arc::new(journal::Journal::new(state_directory, &runtime, document)?);
        Ok(Self {
            owner,
            key: Mutex::new(key),
            connection: Mutex::new(None),
            creator: std::sync::atomic::AtomicBool::new(false),
            notice: Arc::new(Mutex::new(None)),
            access: Arc::new(epochs::Access { journal, identity }),
        })
    }
    pub fn cloud(owner: Arc<Owner>, document: &str, token: &str, state_directory: &Path) -> Result<Self> {
        Self::new(owner, Credential::Cloud { document: document.into(), identity: token.into() }, state_directory)
    }
    pub fn restore(owner: Arc<Owner>, value: &str, state_directory: &Path) -> Result<Self> {
        Self::new(owner, Credential::decode(value)?, state_directory)
    }
    pub fn connect(&self) -> Result<()> {
        self.pause();
        let key = self.key.lock().unwrap().clone();
        if key.ended() {
            return Ok(());
        }
        let key = key.runtime()?;
        bind_owner(&self.owner, &key, &key.room)?;
        *self.connection.lock().unwrap() =
            Some(transport::Connection::start(self.owner.clone(), key, self.notice.clone(), self.access.clone()));
        Ok(())
    }
    pub fn pause(&self) {
        let connection = self.connection.lock().unwrap().take();
        if let Some(connection) = connection {
            connection.stop();
        }
    }
    pub fn flush(&self) -> Result<()> {
        epochs::retry_busy(&self.owner, || Request::Flush)?;
        Ok(())
    }
    pub fn confirmed(&self) -> Result<bool> {
        self.flush()?;
        let Reply::State { reading, .. } = request(&self.owner, Request::State)? else {
            return Err("Owner state unavailable".into());
        };
        Ok(self
            .access
            .journal
            .transaction()?
            .current()?
            .is_some_and(|receipt| receipt.owner_version == reading.version))
    }
    pub fn set_creator(&self, creator: bool) {
        self.creator.store(creator, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn listen(&self, notice: Arc<dyn Fn() + Send + Sync>) {
        *self.notice.lock().unwrap() = Some(notice);
    }
    pub fn status(&self) -> ClientStatus {
        let key = self.key.lock().unwrap();
        let (phase, detail, pause_kind) = self
            .connection
            .lock()
            .unwrap()
            .as_ref()
            .map_or((Phase::Offline, "Offline".into(), None), |connection| connection.status());
        let (phase, detail) = if key.ended() {
            (Phase::Ended, "Sharing ended; your local copy is retained".into())
        } else {
            (phase, detail)
        };
        ClientStatus {
            phase,
            detail,
            pause_kind,
            link: key.link(),
            creator: self.creator.load(std::sync::atomic::Ordering::Relaxed),
            recovery_path: self.access.journal.completed_recovery().map(|path| path.to_string_lossy().into_owned()),
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.pause();
    }
}
pub fn joined_credential(link: &str) -> Result<String> {
    serde_json::to_string(&GuestCredential { link: Invitation::parse(link)?.link()?, ended: false }).map_err(fail)
}
pub fn invitation_document(link: &str) -> Result<String> {
    let key = Credential::decode(&joined_credential(link)?)?.runtime()?;
    Ok(room_info(&key, None)?.document)
}
pub fn local_document(path: &Path) -> Result<Option<String>> {
    Ok(Store::open(path, Mode::Snapshot).map_err(fail)?.app().document_uuid.clone())
}
pub fn install(destination: &Path, link: &str, state_directory: &Path) -> Result<()> {
    let key = Credential::decode(&joined_credential(link)?)?.runtime()?;
    let info = room_info(&key, None)?;
    if destination.exists() {
        let local = Store::open(destination, Mode::Snapshot).map_err(fail)?;
        if local.app().document_uuid.as_deref() != Some(info.document.as_str()) {
            return Err("Different local document".into());
        }
        return validate_membership(local.share().map_err(fail)?, &key);
    }
    let epoch = resolution::epoch(&key, key.bearer())?;
    resolution::download(key.clone(), info, epoch, key.bearer())?.install(destination, state_directory)
}
pub fn verify_account(token: &str) -> Result<()> {
    if token.is_empty() { Err("Sign in first".into()) } else { Ok(()) }
}
pub fn delete_account(token: &str) -> Result<()> {
    cloud::control("DELETE", "account", token, &[]).map(|_| ())
}
pub fn account_owns(document: &str, _credential: &str, token: &str) -> Result<bool> {
    Ok(access(document, token)?.role == pop_sharing_protocol::document::DocumentRole::Owner)
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
        local::install(&stage, destination, &info.document)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(stage);
    result
}

/// Bounded receipt metadata for catalog status; this does not open document state.
pub fn confirmed_saved_version(state_directory: &Path, document: &str) -> Result<Option<String>> {
    let journal = journal::Journal::at(state_directory, pop_sharing_protocol::TRUSTED_ORIGIN, document, document)?;
    let transaction = journal.transaction()?;
    if transaction.pending()?.is_some() {
        return Ok(None);
    }
    Ok(transaction.current()?.map(|receipt| receipt.owner_version))
}

/// Removing a cache requires a saved frontier acknowledged by this cloud document.
pub fn saved_confirmed(path: &Path, state_directory: &Path, document: &str) -> Result<bool> {
    let store = Store::open(path, Mode::Snapshot).map_err(fail)?;
    if store.app().document_uuid.as_deref() != Some(document) {
        return Err("Different cloud document".into());
    }
    let version = store.document().map_err(fail)?.version();
    store.close().map_err(fail)?;
    let journal = journal::Journal::at(state_directory, pop_sharing_protocol::TRUSTED_ORIGIN, document, document)?;
    let transaction = journal.transaction()?;
    Ok(transaction.pending()?.is_none()
        && transaction.current()?.is_some_and(|receipt| receipt.owner_version == version))
}
