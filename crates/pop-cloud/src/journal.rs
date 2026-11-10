//! The device-local receipt of the last version the relay and the local save both hold,
//! never document state or credentials.
use crate::client::*;
use crate::transport;
use pop_contracts::hex;
use pop_core::sync::PauseKind;
use pop_runtime::owner::{Owner, Reply, Request};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Local journal failures retain retryability and their underlying cause.
#[derive(Debug)]
pub(crate) enum Error {
    Busy,
    Storage(pop_storage::store::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str("relay journal is busy"),
            Self::Storage(error) => std::fmt::Display::fmt(error, f),
            Self::Io(error) => write!(f, "relay journal: {error}"),
            Self::Json(error) => write!(f, "invalid relay journal: {error}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}
impl From<pop_storage::store::Error> for Error {
    fn from(error: pop_storage::store::Error) -> Self {
        Self::Storage(error)
    }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}
impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Invalid(message.into())
    }
}
impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Receipt {
    endpoint: String,
    document: String,
    pub owner_version: String,
}
/// One receipt per relay endpoint and cloud document; the relay names a document by its UUID.
pub(crate) struct Journal {
    directory: PathBuf,
    name: String,
    endpoint: String,
    document: String,
}
pub(crate) struct Transaction<'a> {
    journal: &'a Journal,
    _lock: File,
}
/// The existing receipt held under the same OS lock as writers, without creating a
/// directory or lock file. This type deliberately exposes no write methods.
pub(crate) struct Inspection<'a> {
    journal: &'a Journal,
    _lock: File,
}
impl Journal {
    /// The journal for `document` at `origin`, creating its private directory.
    pub fn new(directory: &Path, origin: &str, document: &str) -> Result<Self> {
        let journal = Self::inspect(directory, origin, document)?;
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(directory).map_err(Error::from)?;
        Ok(journal)
    }
    pub fn inspect(directory: &Path, origin: &str, document: &str) -> Result<Self> {
        if !directory.is_absolute()
            || directory.components().any(|part| matches!(part, std::path::Component::ParentDir))
            || !pop_sharing_protocol::valid_document_id(document)
        {
            return Err("Invalid relay state location".into());
        }
        let name = hex(&Sha256::digest(serde_json::to_vec(&(origin, document)).map_err(Error::from)?));
        Ok(Self { directory: directory.into(), name, endpoint: origin.into(), document: document.into() })
    }
    fn path(&self, extension: &str) -> PathBuf {
        self.directory.join(format!("{}.{extension}", self.name))
    }
    pub fn inspection(&self) -> Result<Option<Inspection<'_>>> {
        let file = match File::open(self.path("lock")) {
            Ok(file) => file,
            // Without the writer's stable lock, no receipt can be trusted.
            // Missing state remains unconfirmed.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::Io(error)),
        };
        file.try_lock_shared().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => Error::Busy,
            std::fs::TryLockError::Error(error) => Error::Io(error),
        })?;
        Ok(Some(Inspection { journal: self, _lock: file }))
    }
    pub fn transaction(&self) -> Result<Transaction<'_>> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(self.path("lock"))
            .map_err(Error::from)?;
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => Error::Busy,
            std::fs::TryLockError::Error(error) => Error::Io(error),
        })?;
        Ok(Transaction { journal: self, _lock: file })
    }
    pub fn receipt(&self, owner_version: String) -> Receipt {
        Receipt { endpoint: self.endpoint.clone(), document: self.document.clone(), owner_version }
    }
    fn read(&self, extension: &str) -> Result<Option<Receipt>> {
        let file = match File::open(self.path(extension)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::from(error)),
        };
        let mut bytes = Vec::new();
        file.take(pop_sharing_protocol::CONTROL_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(Error::from)?;
        if bytes.len() > pop_sharing_protocol::CONTROL_BYTES {
            return Err("Relay authority receipt exceeds its limit".into());
        }
        let receipt: Receipt = serde_json::from_slice(&bytes).map_err(Error::from)?;
        if receipt.endpoint != self.endpoint || receipt.document != self.document {
            return Err("Relay authority receipt belongs to another connection".into());
        }
        Ok(Some(receipt))
    }
    fn write(&self, extension: &str, receipt: &Receipt) -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let bytes = serde_json::to_vec(receipt).map_err(Error::from)?;
        if bytes.len() > pop_sharing_protocol::CONTROL_BYTES {
            return Err("Relay authority receipt exceeds its limit".into());
        }
        // The OS lock is held: a fixed temporary cannot belong to another live writer.
        let temporary = self.path("tmp");
        match std::fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::from(error)),
        }
        let result = (|| {
            let mut file =
                OpenOptions::new().create_new(true).write(true).mode(0o600).open(&temporary).map_err(Error::from)?;
            file.write_all(&bytes).map_err(Error::from)?;
            file.sync_all().map_err(Error::from)?;
            std::fs::rename(&temporary, self.path(extension)).map_err(Error::from)?;
            File::open(&self.directory).and_then(|directory| directory.sync_all()).map_err(Error::from)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
}
impl Inspection<'_> {
    pub fn current(&self) -> Result<Option<Receipt>> {
        self.journal.read("json")
    }
}
impl Transaction<'_> {
    #[cfg(test)]
    pub fn current(&self) -> Result<Option<Receipt>> {
        self.journal.read("json")
    }
    /// Atomically replaces the receipt; a failed write keeps the previous one.
    pub fn confirm(&self, receipt: &Receipt) -> Result<()> {
        self.journal.write("json", receipt)
    }
}

/// Journal failures as transport outcomes: contention retries, storage keeps its kind.
pub(crate) fn local(error: impl Into<Error>) -> transport::HttpFailure {
    let error = error.into();
    match error {
        Error::Storage(error) => transport::HttpFailure::from(error),
        Error::Busy => transport::HttpFailure::Busy(error.to_string()),
        Error::Io(_) => transport::HttpFailure::Other(error.to_string()),
        _ => transport::HttpFailure::Paused(pop_core::sync::Pause {
            kind: PauseKind::Refused,
            reason: error.to_string(),
        }),
    }
}
fn transaction(journal: &Journal) -> std::result::Result<Transaction<'_>, transport::HttpFailure> {
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match journal.transaction() {
            Err(Error::Busy) if Instant::now() < deadline => {
                transport::pause(std::time::Duration::from_millis(25))?;
            }
            result => return result.map_err(local),
        }
    }
}
/// Record the exact local version that the relay acknowledged and the local save holds.
pub(crate) fn confirm(
    owner: &Owner,
    connection: &pop_runtime::owner::SyncConnection,
    journal: &Journal,
) -> std::result::Result<bool, transport::HttpFailure> {
    let (send, receive) = std::sync::mpsc::channel();
    connection.durable_confirmed_version(move |confirmed| {
        let _ = send.send(confirmed);
    });
    let Some(confirmed) = transport::wait_reply(&receive, std::time::Duration::from_secs(20))? else {
        return Ok(false);
    };
    // The owner checked both relay acknowledgement and its exact durable history.
    // A normal save completion wakes the connection if Synced arrived first.
    let Reply::State { reading, .. } = request(owner, Request::State)? else {
        return Err(local("Owner state unavailable"));
    };
    if reading.version != confirmed.version {
        return Ok(false);
    }
    transaction(journal)?.confirm(&journal.receipt(confirmed.version)).map_err(local)?;
    Ok(true)
}
/// Record a version the relay already holds: an uploaded or downloaded seed.
pub(crate) fn confirm_seed(
    journal: &Journal,
    owner_version: String,
) -> std::result::Result<(), transport::HttpFailure> {
    transaction(journal)?.confirm(&journal.receipt(owner_version)).map_err(local)
}
