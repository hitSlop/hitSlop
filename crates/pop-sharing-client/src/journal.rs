//! Device-local authority receipts, never document state or credentials. One current
//! receipt and one pending candidate survive uncertain installation acknowledgments.
use super::*;
use pop_sharing_protocol::lifecycle::EpochState;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    endpoint: String,
    room: String,
    document: String,
    pub epoch: EpochState,
    pub owner_version: String,
    pub recovery: Option<PathBuf>,
}
pub(crate) struct Journal {
    directory: PathBuf,
    name: String,
    endpoint: String,
    room: String,
    document: String,
    checked_recovery: Mutex<Option<(PathBuf, recovery::FileStamp)>>,
}
pub(crate) struct Transaction<'a> {
    journal: &'a Journal,
    _lock: File,
}
impl Journal {
    pub fn new(directory: &Path, key: &RoomKey, document: &str) -> Result<Self> {
        if !directory.is_absolute() || !pop_sharing_protocol::valid_copy(document) {
            return Err("Invalid relay state location".into());
        }
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(directory).map_err(fail)?;
        let name = hex(&Sha256::digest(
            serde_json::to_vec(&(key.origin.as_str(), key.room.as_str(), document)).map_err(fail)?,
        ));
        Ok(Self {
            directory: directory.into(),
            name,
            endpoint: key.origin.clone(),
            room: key.room.clone(),
            document: document.into(),
            checked_recovery: Mutex::new(None),
        })
    }
    pub fn recovery_destination(&self, source_version: &str) -> Result<PathBuf> {
        let directory = recovery::directory(&self.directory)?;
        let mut hash = Sha256::new();
        hash.update(b"pop.recovery-source.v1\0");
        hash.update(self.name.as_bytes());
        hash.update([0]);
        hash.update(source_version.as_bytes());
        Ok(directory.join(format!("{}-recovery-{}.pop", self.document, hex(&hash.finalize()))))
    }
    fn completed(&self, path: &Path) -> bool {
        let Some(stamp) = recovery::stamp(path) else { return false };
        if self
            .checked_recovery
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(checked_path, checked_stamp)| checked_path == path && checked_stamp == &stamp)
        {
            return true;
        }
        if !recovery::accepted(path, Some(&self.document)) {
            return false;
        }
        *self.checked_recovery.lock().unwrap() = Some((path.to_owned(), stamp));
        true
    }
    pub fn frozen_handoff(&self) -> Result<bool> {
        let transaction = self.transaction()?;
        Ok([transaction.pending()?, transaction.current()?]
            .into_iter()
            .flatten()
            .any(|receipt| receipt.epoch.handoff.is_some()))
    }
    pub fn retire_handoff(&self, freeze: &str) -> Result<()> {
        let frozen: pop_sharing_protocol::lifecycle::Handoff = serde_json::from_str(freeze).map_err(fail)?;
        frozen.validate()?;
        let transaction = self.transaction()?;
        if let Some(mut receipt) = transaction.current()?
            && receipt.epoch.handoff.as_ref() == Some(&frozen)
        {
            receipt.epoch.handoff = None;
            transaction.confirm(&receipt)?;
        }
        Ok(())
    }
    pub fn completed_recovery(&self) -> Option<PathBuf> {
        if let Ok(transaction) = self.transaction() {
            for receipt in
                [transaction.pending().ok().flatten(), transaction.current().ok().flatten()].into_iter().flatten()
            {
                if let Some(path) = receipt.recovery
                    && self.completed(&path)
                {
                    return Some(path);
                }
            }
        }
        // The receipt can already have rolled over, or still be locked by an
        // uncertain install. Completed files remain independently discoverable.
        let mut paths = recovery::paths(&self.directory).ok()?;
        paths.sort_by_key(|path| std::cmp::Reverse(std::fs::metadata(path).and_then(|meta| meta.modified()).ok()));
        paths.into_iter().find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&format!("{}-recovery-", self.document)))
                && self.completed(path)
        })
    }
    fn path(&self, extension: &str) -> PathBuf {
        self.directory.join(format!("{}.{extension}", self.name))
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
            .map_err(fail)?;
        file.try_lock().map_err(fail)?;
        Ok(Transaction { journal: self, _lock: file })
    }
    pub fn receipt(&self, epoch: EpochState, owner_version: String, recovery: Option<PathBuf>) -> Receipt {
        Receipt {
            endpoint: self.endpoint.clone(),
            room: self.room.clone(),
            document: self.document.clone(),
            epoch,
            owner_version,
            recovery,
        }
    }
    fn read(&self, extension: &str) -> Result<Option<Receipt>> {
        let file = match File::open(self.path(extension)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(fail(error)),
        };
        let mut bytes = Vec::new();
        file.take(pop_sharing_protocol::CONTROL_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(fail)?;
        if bytes.len() > pop_sharing_protocol::CONTROL_BYTES {
            return Err("Relay authority receipt exceeds its limit".into());
        }
        let receipt: Receipt = serde_json::from_slice(&bytes).map_err(fail)?;
        receipt.epoch.validate()?;
        if receipt.endpoint != self.endpoint || receipt.room != self.room || receipt.document != self.document {
            return Err("Relay authority receipt belongs to another connection".into());
        }
        Ok(Some(receipt))
    }
    fn write(&self, extension: &str, receipt: &Receipt) -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        receipt.epoch.validate()?;
        let bytes = serde_json::to_vec(receipt).map_err(fail)?;
        if bytes.len() > pop_sharing_protocol::CONTROL_BYTES {
            return Err("Relay authority receipt exceeds its limit".into());
        }
        // The OS lock is held: a fixed temporary cannot belong to another live writer.
        let temporary = self.path("tmp");
        match std::fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(fail(error)),
        }
        let result = (|| {
            let mut file =
                OpenOptions::new().create_new(true).write(true).mode(0o600).open(&temporary).map_err(fail)?;
            file.write_all(&bytes).map_err(fail)?;
            file.sync_all().map_err(fail)?;
            std::fs::rename(&temporary, self.path(extension)).map_err(fail)?;
            File::open(&self.directory).and_then(|directory| directory.sync_all()).map_err(fail)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
}
impl Transaction<'_> {
    pub fn current(&self) -> Result<Option<Receipt>> {
        self.journal.read("json")
    }
    pub fn pending(&self) -> Result<Option<Receipt>> {
        self.journal.read("pending")
    }
    pub fn prepare(&self, receipt: &Receipt) -> Result<()> {
        self.journal.write("pending", receipt)
    }
    pub fn promote(&self, expected: &Receipt) -> Result<()> {
        if self.pending()?.as_ref() != Some(expected) {
            return Err("The pending relay receipt changed".into());
        }
        std::fs::rename(self.journal.path("pending"), self.journal.path("json")).map_err(fail)?;
        File::open(&self.journal.directory).and_then(|directory| directory.sync_all()).map_err(fail)
    }
    pub fn confirm(&self, receipt: &Receipt) -> Result<()> {
        self.prepare(receipt)?;
        self.promote(receipt)
    }
}
