//! Persistent cloud document synchronization under one Firebase identity.
use crate::library::receipts;
use crate::{cloud, transport};
use pop_contracts::hex;
use pop_contracts::relay::SyncPhase;
use pop_runtime::owner::{CallOptions, Completion, Owner};
use pop_sharing_protocol::http::{Invitation, RoomInfo};
use pop_storage::store::{Mode, Share, Store};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) use crate::error::{Error, Result};
pub(crate) type Notice = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
#[cfg(test)]
pub(crate) use crate::local::request;
pub(crate) async fn retry_busy<T: Send + 'static>(
    owner: &Owner,
    build: impl Fn(&Owner, CallOptions, Completion<T>),
) -> Result<T> {
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match transport::owner_request(
            owner,
            &transport::OwnerOutcome::default(),
            &build,
            transport::OwnerEffect::Other,
        )
        .await
        {
            Err(crate::Error::Sharing { kind: PauseKind::Busy, .. }) if Instant::now() < deadline => {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            result => return result,
        }
    }
}
pub(crate) fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file =
        std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(Error::from)?;
    file.write_all(bytes).map_err(Error::from)?;
    file.sync_all().map_err(Error::from)
}
#[derive(Clone)]
pub(crate) struct RoomKey {
    pub(crate) room: String,
    pub(crate) identity: String,
    pub(crate) identity_updates: Option<tokio::sync::watch::Receiver<Option<String>>>,
    pub(crate) origin: String,
}
impl RoomKey {
    pub(crate) fn bearer(&self) -> String {
        self.identity_updates
            .as_ref()
            .and_then(|identity| identity.borrow().clone())
            .unwrap_or_else(|| self.identity.clone())
    }
    pub(crate) fn identity(document: &str, token: &str) -> Result<Self> {
        if !pop_contracts::identity::valid_document_id(document)
            || token.is_empty()
            || token.len() > pop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES
        {
            return Err("Invalid cloud document access".into());
        }
        Ok(Self {
            room: document.into(),
            identity: token.into(),
            identity_updates: None,
            origin: pop_sharing_protocol::TRUSTED_ORIGIN.into(),
        })
    }
}
pub(crate) async fn room_info(key: &RoomKey) -> std::result::Result<RoomInfo, crate::Error> {
    let bytes = transport::http(
        &key.origin,
        &key.bearer(),
        transport::HttpOperation::new(reqwest::Method::GET, transport::Endpoint::Document(&key.room)),
    )
    .await?
    .unwrap_or_default();
    let info: RoomInfo = serde_json::from_slice(&bytes).map_err(Error::from)?;
    if !pop_contracts::identity::valid_document_id(&info.document)
        || info.document != key.room
        || info.seed_hash.as_deref().is_some_and(|hash| !pop_contracts::wire::valid_attachment_id(hash))
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
pub(crate) async fn bind_owner(owner: &Owner, key: &RoomKey, document: &str) -> Result<()> {
    if owner.app().document_uuid.as_deref() != Some(document) {
        return Err("Different cloud document".into());
    }
    retry_busy(owner, |owner: &Owner, options, callback| {
        owner.set_share(Share { endpoint: key.origin.clone() }, options, callback)
    })
    .await?;
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
        transport::discard_files(
            vec![self.path.clone(), self.snapshot.clone()],
            self.path.parent().map(Path::to_owned),
        );
    }
}

#[cfg(test)]
pub(crate) fn stage(owner: &Owner) -> Result<Seed> {
    transport::block_on(stage_async(owner))
}
pub(crate) async fn stage_async(owner: &Owner) -> Result<Seed> {
    let directory = std::env::temp_dir()
        .join(format!("pop-sharing-{}", pop_core::random_hex(32).map_err(|_| "Cannot generate document identity")?));
    use std::os::unix::fs::DirBuilderExt;
    let create = directory.clone();
    transport::blocking(move || Ok(std::fs::DirBuilder::new().mode(0o700).create(create)?)).await?;
    let mut seed = Seed {
        snapshot: directory.join("snapshot.pop"),
        path: directory.join("seed.pop"),
        version: vec![],
        owner_version: String::new(),
        app: String::new(),
        hash: String::new(),
    };
    retry_busy(owner, |owner: &Owner, options, callback| owner.stage_seed(seed.snapshot.clone(), options, callback))
        .await?;
    transport::blocking(move || {
        let source = Store::open(&seed.snapshot, Mode::Snapshot)?;
        source.seed_without_attachments(&seed.path)?;
        source.close()?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&seed.path, std::fs::Permissions::from_mode(0o600))?;
        if std::fs::metadata(&seed.path)?.len() > pop_sharing_protocol::SEED_BYTES as u64 {
            return Err("The embedded app and saved history exceed the 16 MiB initial sync limit. Your local document is retained.".into());
        }
        let store = Store::open(&seed.path, Mode::Snapshot)?;
        let document = store.document()?;
        seed.version = document.wire_history();
        seed.owner_version = document.version();
        seed.app = hex(store.app_digest()?.as_bytes());
        seed.hash = hex(&Sha256::digest(std::fs::read(&seed.path)?));
        store.close()?;
        Ok(seed)
    }).await
}

use pop_contracts::relay::PauseKind;
pub struct ClientStatus {
    pub pause_kind: Option<PauseKind>,
    pub phase: SyncPhase,
    pub detail: String,
}
pub struct Client {
    owner: Arc<Owner>,
    key: Mutex<RoomKey>,
    connection: Mutex<Option<transport::Connection>>,
    notice: Notice,
    receipt: receipts::Receipt,
}
impl Client {
    fn new(owner: Arc<Owner>, key: RoomKey, receipt: receipts::Receipt, notice: Notice) -> Result<Self> {
        let document = owner.app().document_uuid.as_deref().ok_or("Only documents can synchronize")?;
        if document != key.room {
            return Err("Different cloud document".into());
        }
        validate_membership(owner.share().map_err(Error::from)?, &key)?;
        Ok(Self { owner, key: Mutex::new(key), connection: Mutex::new(None), notice, receipt })
    }
    pub(crate) fn cloud(
        owner: Arc<Owner>,
        document: &str,
        token: &str,
        receipt: receipts::Receipt,
        notice: Notice,
    ) -> Result<Self> {
        Self::new(owner, RoomKey::identity(document, token)?, receipt, notice)
    }
    /// Includes transient reconnects and compaction; terminal supervisors are stopped.
    pub(crate) fn is_running(&self) -> bool {
        pop_core::lock(&self.connection).as_ref().is_some_and(transport::Connection::is_running)
    }
    /// Refresh credentials within the caller's existing account binding. Account changes
    /// stop existing clients before rebinding the long-lived library.
    pub(crate) fn set_identity(&self, token: &str) -> Result<()> {
        {
            let mut key = pop_core::lock(&self.key);
            RoomKey::identity(&key.room, token)?;
            key.identity = token.to_owned();
        }
        if let Some(connection) = pop_core::lock(&self.connection).as_ref() {
            connection.set_identity(token.to_owned());
        }
        Ok(())
    }
    pub(crate) fn connect(&self) -> Result<()> {
        self.pause();
        let key = pop_core::lock(&self.key).clone();
        validate_membership(self.owner.share()?, &key)?;
        transport::block_on(bind_owner(&self.owner, &key, &key.room))?;
        let connection =
            transport::Connection::start(self.owner.clone(), key, self.notice.clone(), self.receipt.clone());
        *pop_core::lock(&self.connection) = Some(connection);
        Ok(())
    }
    pub(crate) fn pause(&self) {
        let connection = pop_core::lock(&self.connection).take();
        if let Some(connection) = connection {
            connection.stop();
        }
    }
    pub(crate) fn flush(&self) -> Result<()> {
        transport::block_on(retry_busy(&self.owner, Owner::flush))?;
        Ok(())
    }
    pub fn status(&self) -> ClientStatus {
        let (phase, detail, pause_kind) = self
            .connection
            .lock()
            .unwrap()
            .as_ref()
            .map_or((SyncPhase::Offline, "Offline".into(), None), |connection| connection.status());
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
    Ok(Invitation::parse(link).map_err(Error::from)?.room)
}
pub fn local_document(path: &Path) -> Result<Option<String>> {
    Ok(Store::open(path, Mode::Snapshot).map_err(Error::from)?.app().document_uuid.clone())
}
pub fn delete_account(token: &str) -> Result<()> {
    cloud::control(reqwest::Method::DELETE, transport::Endpoint::Account, token, &[]).map(|_| ())
}
