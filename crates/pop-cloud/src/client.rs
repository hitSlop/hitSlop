//! Persistent cloud document synchronization under one Firebase identity.
use crate::{attachments, cloud, journal, local, transport};
use pop_runtime::owner::{Owner, Reply, Request};
use pop_sharing_protocol::{
    hex,
    http::{Invitation, RoomInfo},
};
use pop_storage::store::{Mode, Share, Store};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
#[cfg(test)]
pub(crate) mod tests;
pub(crate) type Result<T> = std::result::Result<T, String>;
pub(crate) type Notice = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
pub(crate) fn connection_notices(primary: &Notice, observer: &Notice) -> Notice {
    let primary = primary.clone();
    let observer = observer.clone();
    Arc::new(Mutex::new(Some(Arc::new(move || {
        let callbacks = [primary.lock().unwrap().clone(), observer.lock().unwrap().clone()];
        for callback in callbacks.into_iter().flatten() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
        }
    }))))
}
pub(crate) fn fail(error: impl std::fmt::Display) -> String {
    error.to_string()
}
pub(crate) fn request(owner: &Owner, request: Request) -> std::result::Result<Reply, transport::HttpFailure> {
    transport::owner_request(owner, request)
}
pub(crate) fn retry_busy(
    owner: &Owner,
    build: impl Fn() -> Request,
) -> std::result::Result<Reply, transport::HttpFailure> {
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match request(owner, build()) {
            Err(transport::HttpFailure::Busy(_)) if Instant::now() < deadline => {
                transport::pause(std::time::Duration::from_millis(25))?
            }
            result => return result,
        }
    }
}
pub(crate) fn random() -> Result<String> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "Cannot generate document identity")?;
    Ok(hex(&bytes))
}
pub(crate) fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(fail)?;
    file.write_all(bytes).map_err(fail)?;
    file.sync_all().map_err(fail)
}
#[derive(Clone)]
pub(crate) struct RoomKey {
    pub(crate) room: String,
    pub(crate) identity: String,
    pub(crate) origin: String,
}
impl RoomKey {
    pub(crate) fn bearer(&self) -> &str {
        &self.identity
    }
    pub(crate) fn identity(document: &str, token: &str) -> Result<Self> {
        if !pop_sharing_protocol::valid_document_id(document)
            || token.is_empty()
            || token.len() > pop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES
        {
            return Err("Invalid cloud document access".into());
        }
        Ok(Self { room: document.into(), identity: token.into(), origin: pop_sharing_protocol::TRUSTED_ORIGIN.into() })
    }
}
pub(crate) fn room_info(key: &RoomKey, token: Option<&str>) -> std::result::Result<RoomInfo, transport::HttpFailure> {
    let bytes = transport::http_typed(key, "GET", "", Some(token.unwrap_or(key.bearer())), &[], &[])?;
    let info: RoomInfo = serde_json::from_slice(&bytes).map_err(fail)?;
    if !pop_sharing_protocol::valid_document_id(&info.document)
        || info.document != key.room
        || info.seed_hash.as_deref().is_some_and(|hash| !pop_sharing_protocol::resources::hash_id(hash))
        || info.seed_size.is_some_and(|size| size as usize > pop_sharing_protocol::SEED_BYTES)
    {
        return Err("Invalid cloud document metadata".into());
    }
    Ok(info)
}
/// The owner's document UUID names the relay's document; the share names the relay.
pub(crate) fn validate_membership(share: Option<Share>, key: &RoomKey) -> Result<()> {
    if share.is_some_and(|share| share.endpoint != key.origin) {
        return Err("Conflicting cloud document membership".into());
    }
    Ok(())
}
pub(crate) fn bind_owner(owner: &Owner, key: &RoomKey, document: &str) -> Result<()> {
    if owner.app().document_uuid.as_deref() != Some(document) {
        return Err("Different cloud document".into());
    }
    validate_membership(owner.share().map_err(fail)?, key)?;
    retry_busy(owner, || Request::Share { share: Share { endpoint: key.origin.clone() } })?;
    Ok(())
}
/// The saved document as uploaded once, before the room goes live. Its history is the
/// room's root: the owner keeps editing on top of it and members start from it.
pub(crate) struct Seed {
    pub(crate) snapshot: std::path::PathBuf,
    pub(crate) path: std::path::PathBuf,
    pub(crate) version: Vec<(String, u32)>,
    pub(crate) owner_version: String,
    pub(crate) app: String,
    pub(crate) hash: String,
}
impl Drop for Seed {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(&self.snapshot);
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

pub(crate) fn stage(owner: &Owner) -> Result<Seed> {
    let directory = std::env::temp_dir().join(format!("pop-sharing-{}", random()?));
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(&directory).map_err(fail)?;
    let path = directory.join("seed.pop");
    let snapshot = directory.join("snapshot.pop");
    let result = (|| {
        retry_busy(owner, || Request::StageSeed { destination: snapshot.clone() })?;
        let source = Store::open(&snapshot, Mode::Snapshot).map_err(fail)?;
        source.seed_without_attachments(&path).map_err(fail)?;
        source.close().map_err(fail)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(fail)?;
        if std::fs::metadata(&path).map_err(fail)?.len() > pop_sharing_protocol::SEED_BYTES as u64 {
            return Err("The embedded app and saved history exceed the 16 MiB initial sync limit. Your local document is retained.".into());
        }
        let store = Store::open(&path, Mode::Snapshot).map_err(fail)?;
        let document = store.document().map_err(fail)?;
        let seed = Seed {
            snapshot: snapshot.clone(),
            path: path.clone(),
            version: document.wire_history(),
            owner_version: document.version(),
            app: hex(&store.app_digest().map_err(fail)?),
            hash: hex(&Sha256::digest(std::fs::read(&path).map_err(fail)?)),
        };
        store.close().map_err(fail)?;
        Ok(seed)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&snapshot);
        let _ = std::fs::remove_dir(&directory);
    }
    result
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
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
}
pub struct Client {
    owner: Arc<Owner>,
    key: Mutex<RoomKey>,
    connection: Mutex<Option<transport::Connection>>,
    notice: Notice,
    observer: Notice,
    journal: Arc<journal::Journal>,
}
impl Client {
    fn new(owner: Arc<Owner>, key: RoomKey, state_directory: &Path) -> Result<Self> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can synchronize")?;
        if document != key.room {
            return Err("Different cloud document".into());
        }
        validate_membership(owner.share().map_err(fail)?, &key)?;
        let journal = Arc::new(journal::Journal::new(state_directory, &key.origin, document)?);
        Ok(Self {
            owner,
            key: Mutex::new(key),
            connection: Mutex::new(None),
            notice: Arc::new(Mutex::new(None)),
            observer: Arc::new(Mutex::new(None)),
            journal,
        })
    }
    pub(crate) fn cloud(owner: Arc<Owner>, document: &str, token: &str, state_directory: &Path) -> Result<Self> {
        Self::new(owner, RoomKey::identity(document, token)?, state_directory)
    }
    /// Includes transient reconnects and compaction; terminal supervisors are stopped.
    pub(crate) fn is_running(&self) -> bool {
        self.connection.lock().unwrap().as_ref().is_some_and(transport::Connection::is_running)
    }
    /// Refresh credentials within the caller's existing account binding. Account changes
    /// detach the old library/client rather than transfer its live connection.
    pub(crate) fn set_identity(&self, token: &str) -> Result<()> {
        {
            let mut key = self.key.lock().unwrap();
            RoomKey::identity(&key.room, token)?;
            key.identity = token.to_owned();
        }
        if let Some(connection) = self.connection.lock().unwrap().as_ref() {
            connection.set_identity(token.to_owned());
        }
        Ok(())
    }
    pub(crate) fn connect(&self) -> Result<()> {
        self.pause();
        let key = self.key.lock().unwrap().clone();
        bind_owner(&self.owner, &key, &key.room)?;
        let connection = transport::Connection::start(
            self.owner.clone(),
            key,
            connection_notices(&self.notice, &self.observer),
            self.journal.clone(),
        );
        *self.connection.lock().unwrap() = Some(connection);
        Ok(())
    }
    pub(crate) fn pause(&self) {
        let connection = self.connection.lock().unwrap().take();
        if let Some(connection) = connection {
            connection.stop();
        }
    }
    pub(crate) fn flush(&self) -> Result<()> {
        retry_busy(&self.owner, || Request::Flush)?;
        Ok(())
    }
    /// Internal owner of catalog snapshots; independent from the document UI listener.
    pub(crate) fn observe(&self, observer: Arc<dyn Fn() + Send + Sync>) {
        *self.observer.lock().unwrap() = Some(observer);
    }
    pub fn listen(&self, notice: Arc<dyn Fn() + Send + Sync>) {
        *self.notice.lock().unwrap() = Some(notice);
    }
    pub fn status(&self) -> ClientStatus {
        let (phase, detail, pause_kind) = self
            .connection
            .lock()
            .unwrap()
            .as_ref()
            .map_or((Phase::Offline, "Offline".into(), None), |connection| connection.status());
        ClientStatus { phase, detail, pause_kind }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.pause();
    }
}
/// Parse an invitation without using its capability as an ongoing connection identity.
pub fn invitation_document(link: &str) -> Result<String> {
    Ok(Invitation::parse(link)?.room)
}
pub fn local_document(path: &Path) -> Result<Option<String>> {
    Ok(Store::open(path, Mode::Snapshot).map_err(fail)?.app().document_uuid.clone())
}
pub fn delete_account(token: &str) -> Result<()> {
    cloud::control("DELETE", "account", token, &[]).map(|_| ())
}
pub(crate) fn install_seed(
    destination: &Path,
    key: &RoomKey,
    info: &RoomInfo,
    bytes: &[u8],
    current: impl FnOnce() -> Result<()>,
) -> Result<()> {
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
        let binding: Result<()> = (|| {
            request(&owner, Request::Share { share: Share { endpoint: key.origin.clone() } })?;
            attachments::download_blobs(&owner, key, &mut Vec::new())?;
            Ok(())
        })();
        let closed = request(&owner, Request::Close { preview: None, icon: None });
        binding?;
        closed?;
        current()?;
        local::install(&stage, destination, &info.document)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(stage);
    result
}

/// Bounded receipt metadata for catalog status; this does not open document state.
pub(crate) fn confirmed_saved_version(state_directory: &Path, document: &str) -> Result<Option<String>> {
    let journal = journal::Journal::inspect(state_directory, pop_sharing_protocol::TRUSTED_ORIGIN, document)?;
    let Some(transaction) = journal.inspection()? else { return Ok(None) };
    Ok(transaction.current()?.map(|receipt| receipt.owner_version))
}

/// Removing a cache requires a saved frontier acknowledged by this cloud document.
pub(crate) fn saved_confirmed(path: &Path, state_directory: &Path, document: &str) -> Result<bool> {
    let store = Store::open(path, Mode::Snapshot).map_err(fail)?;
    if store.app().document_uuid.as_deref() != Some(document) {
        return Err("Different cloud document".into());
    }
    let version = store.document().map_err(fail)?.version();
    store.close().map_err(fail)?;
    let journal = journal::Journal::inspect(state_directory, pop_sharing_protocol::TRUSTED_ORIGIN, document)?;
    let Some(transaction) = journal.inspection()? else { return Ok(false) };
    Ok(transaction.current()?.is_some_and(|receipt| receipt.owner_version == version))
}
