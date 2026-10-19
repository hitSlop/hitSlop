//! iCloud files are publication targets, never live SQLite databases. The normal Store
//! owns `working.slop`; this session owns the public lease and an immutable last-published
//! clone used only when an external stat change requires a byte comparison.
use super::{access, opened, reader};
use crate::{
    error::{Error, Result, failed},
    registry::Lease,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    dev: u64,
    ino: u64,
    size: u64,
    seconds: i64,
    nanos: i64,
}
impl Stamp {
    fn read(path: &Path) -> Result<Self> {
        let m = crate::registry::regular(path)?;
        if m.nlink() != 1 {
            return Err(failed("A cloud document must not have another hard link"));
        }
        Ok(Self { dev: m.dev(), ino: m.ino(), size: m.len(), seconds: m.mtime(), nanos: m.mtime_nsec() })
    }
    fn identity(&self) -> (u64, u64) {
        (self.dev, self.ino)
    }
}
#[derive(Serialize, Deserialize)]
struct Pending {
    staged: PathBuf,
    /// Recorded before replacement; a killed app can distinguish its own publication.
    inode: Option<(u64, u64)>,
    base_copy: String,
}
#[derive(Serialize, Deserialize)]
struct Record {
    target: PathBuf,
    token: String,
    base: Stamp,
    base_copy: String,
    saved: Stamp,
    pending: Option<Pending>,
}
pub(crate) struct Session {
    root: PathBuf,
    _lock: fs::File,
    record: Record,
    lease: Lease,
    discovery: Option<String>,
}
fn hash(path: &Path) -> Result<Vec<u8>> {
    let mut input = fs::File::open(path).map_err(failed)?;
    let mut hash = Sha256::new();
    let mut chunk = [0; 65536];
    loop {
        let size = input.read(&mut chunk).map_err(failed)?;
        if size == 0 {
            break;
        }
        hash.update(&chunk[..size]);
    }
    Ok(hash.finalize().to_vec())
}
fn same_bytes(left: &Path, right: &Path) -> Result<bool> {
    Ok(fs::metadata(left).map_err(failed)?.len() == fs::metadata(right).map_err(failed)?.len()
        && hash(left)? == hash(right)?)
}
fn private_folder(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(failed)?;
    if !fs::symlink_metadata(path).map_err(failed)?.is_dir() {
        return Err(failed("Invalid private session folder"));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(failed)
}
impl Session {
    pub(crate) fn working(&self) -> PathBuf {
        self.root.join("working.slop")
    }
    pub(crate) fn open(target: &Path) -> Result<Self> {
        let target = super::resolve(target)?;
        let target = target.as_path();
        let root = crate::registry::sessions_folder()?.join(crate::hex(&Sha256::digest(target.as_os_str().as_bytes())));
        Self::at(target, &root)
    }
    pub(crate) fn at(target: &Path, root: &Path) -> Result<Self> {
        let target = super::resolve(target)?;
        let target = target.as_path();
        private_folder(root)?;
        let held = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("session.lock"))
            .map_err(failed)?;
        held.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => Error::Locked,
            fs::TryLockError::Error(error) => failed(error),
        })?;
        access::claim(target, false, |resolved| {
            // A move during coordination is not an implicit relocation of the owner's URL.
            if super::resolve(resolved)? != target {
                return Err(Error::Moved);
            }
            let lease = Lease::acquire(target)?;
            let record_path = root.join("record.json");
            if record_path.exists() {
                let record: Record = serde_json::from_slice(&fs::read(record_path).map_err(failed)?).map_err(failed)?;
                if record.target != target {
                    return Err(failed("Recovery target changed; the private copy is retained"));
                }
                let mut session = Self { root: root.into(), _lock: held, record, lease, discovery: None };
                session.reconcile()?;
                session.check_base(target)?;
                // The normal Store recovers and validates this working database, including
                // any hot journal, before configuring its writer.
                return Ok(session);
            }
            let source = reader(target)?;
            let read = source.unchecked_transaction().map_err(failed)?;
            let app = opened(&read, target, true)?;
            if app.kind != super::Kind::Document {
                return Err(crate::error::rejected(crate::Code::IsTemplate, "Create a document from this template"));
            }
            crate::store::validate_saved(&read, app.app.spec())?;
            let token = crate::random_hex(16);
            let base_copy = format!("base-{token}.slop");
            let base_path = root.join(&base_copy);
            let working = root.join("working.slop");
            // A record is written only after both clones are durable. A crash before that
            // leaves no accepted edits; abandoned preparation files may be removed here.
            for file in fs::read_dir(root).map_err(failed)? {
                let path = file.map_err(failed)?.path();
                if path.file_name().is_some_and(|name| name != "session.lock")
                    && fs::symlink_metadata(&path).map_err(failed)?.is_file()
                {
                    fs::remove_file(path).map_err(failed)?;
                }
            }
            access::clone_file(target, &working)?;
            fs::set_permissions(&working, fs::Permissions::from_mode(0o600)).map_err(failed)?;
            access::clone_file(&working, &base_path)?;
            access::sync_file(&working)?;
            access::sync_file(&base_path)?;
            let record = Record {
                target: target.into(),
                token,
                base: Stamp::read(target)?,
                base_copy,
                saved: Stamp::read(&working)?,
                pending: None,
            };
            let session = Self { root: root.into(), _lock: held, record, lease, discovery: None };
            session.persist()?;
            Ok(session)
        })
    }
    fn persist(&self) -> Result<()> {
        let temporary = self.root.join("record.json.tmp");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)
            .map_err(failed)?;
        file.write_all(&serde_json::to_vec(&self.record).map_err(failed)?).map_err(failed)?;
        file.sync_all().map_err(failed)?;
        drop(file);
        access::sync_file(&temporary)?;
        fs::rename(temporary, self.root.join("record.json")).map_err(failed)?;
        access::sync_dir(&self.root)
    }
    fn conflict(&self) -> Error {
        failed(format!(
            "The iCloud copy changed outside hitSlop. Your edits are saved locally at {}. Keep this document open and use Duplicate or Share a Copy to save them to a new file.",
            self.working().display()
        ))
    }
    fn check_base(&self, target: &Path) -> Result<()> {
        let current = Stamp::read(target)?;
        if current == self.record.base {
            return Ok(());
        }
        // Same bytes (for example `touch`) are harmless. A replacement inode is still
        // refused because ownership/discovery belong to the original identity.
        if current.identity() == self.record.base.identity()
            && same_bytes(target, &self.root.join(&self.record.base_copy))?
        {
            return Ok(());
        }
        Err(self.conflict())
    }
    fn cleanup_candidate(&self, pending: &Pending) -> Result<()> {
        let expected = self.record.target.with_file_name(format!(".hitslop-publish-{}.tmp", self.record.token));
        if pending.staged != expected {
            return Err(failed("Invalid publication candidate path"));
        }
        if pending.staged.exists() {
            let stamp = Stamp::read(&pending.staged)?;
            if pending.inode.is_some_and(|inode| inode != stamp.identity()) {
                return Err(self.conflict());
            }
            fs::remove_file(&pending.staged).map_err(failed)?;
            access::sync_dir(pending.staged.parent().ok_or(Error::Moved)?)?;
        }
        Ok(())
    }
    fn reconcile(&mut self) -> Result<()> {
        let Some(pending) = self.record.pending.as_ref() else {
            return Ok(());
        };
        let current = Stamp::read(&self.record.target)?;
        if pending.inode == Some(current.identity()) {
            if !same_bytes(&self.record.target, &self.root.join(&pending.base_copy))? {
                return Err(self.conflict());
            }
            // Replacement may have completed just before termination or a directory-sync
            // error. Complete its durable receipt before another save can publish.
            access::sync_file(&self.record.target)?;
            access::sync_dir(self.record.target.parent().ok_or(Error::Moved)?)?;
            let previous = self.record.base_copy.clone();
            self.record.base = current;
            self.record.base_copy = pending.base_copy.clone();
            self.record.pending = None;
            self.persist()?;
            let _ = fs::remove_file(self.root.join(previous));
        } else {
            self.cleanup_candidate(pending)?;
            let abandoned = self.root.join(&pending.base_copy);
            self.record.pending = None;
            self.persist()?;
            let _ = fs::remove_file(abandoned);
        }
        Ok(())
    }
    /// Called only with the Store's backing mutex, between committed transactions.
    pub(crate) fn publish(&mut self, conn: &rusqlite::Connection) -> Result<()> {
        if !conn.is_autocommit() {
            return Err(failed("Cannot publish inside a SQLite transaction"));
        }
        let target = self.record.target.clone();
        access::claim(&target, true, |resolved| {
            if super::resolve(resolved)? != target {
                return Err(Error::Moved);
            }
            self.reconcile()?;
            self.check_base(&target)?;
            let working = Stamp::read(&self.working())?;
            if working == self.record.saved {
                return Ok(());
            }
            let staged = target.with_file_name(format!(".hitslop-publish-{}.tmp", self.record.token));
            let base_copy = format!("base-{}.slop", crate::random_hex(16));
            self.record.pending = Some(Pending { staged: staged.clone(), inode: None, base_copy: base_copy.clone() });
            self.persist()?;
            let base = self.root.join(&base_copy);
            access::clone_file(&self.working(), &base)?;
            access::sync_file(&base)?;
            access::sync_dir(&self.root)?;
            access::clone_file(&base, &staged)?;
            // No SQL transaction ever touches a public file, including this candidate.
            // Checking the header/schema is sufficient for a clone of the validated writer.
            let check = reader(&staged)?;
            super::check(&check, false)?;
            check.close().map_err(|(_, error)| failed(error))?;
            access::metadata(&target, &staged)?;
            let modified = fs::metadata(self.working()).and_then(|m| m.modified()).map_err(failed)?;
            // Metadata preservation must not reset the saved content's modification date.
            fs::File::open(&staged)
                .and_then(|file| file.set_times(fs::FileTimes::new().set_modified(modified)))
                .map_err(failed)?;
            access::sync_file(&staged)?;
            let mut new_lease = Lease::acquire(&staged)?;
            self.record.pending.as_mut().expect("pending candidate").inode = Some(new_lease.file());
            self.persist()?;
            if let Some(json) = &self.discovery {
                new_lease.publish(json)?;
            }
            let mode = fs::metadata(&target).map_err(failed)?.permissions();
            self.lease.check()?;
            self.check_base(&target)?;
            // After replace succeeds, transfer ownership BEFORE fallible durability work.
            // A subsequent failure keeps both the candidate receipt and the current lease.
            let replacement = access::replace(&staged, &target);
            if Stamp::read(&target)?.identity() == new_lease.file() {
                new_lease.retarget(&target)?;
                self.lease = new_lease;
                fs::set_permissions(&target, mode).map_err(failed)?;
                // Foundation's default safe-save preserves the old item's metadata.
                fs::File::open(&target)
                    .and_then(|file| file.set_times(fs::FileTimes::new().set_modified(modified)))
                    .map_err(failed)?;
                access::sync_file(&target)?;
            }
            replacement?;
            // saved advances with the receipt. A crash before it advances merely causes
            // one harmless repeat publication after recovery.
            self.record.saved = working;
            self.reconcile()
        })
    }
    pub(crate) fn publish_discovery(&mut self, json: &str) -> Result<()> {
        self.lease.publish(json)?;
        self.discovery = Some(json.into());
        Ok(())
    }
    pub(crate) fn withdraw(&mut self) {
        self.lease.withdraw();
        self.discovery = None;
    }
    pub(crate) fn finish(&self) -> Result<()> {
        match fs::remove_file(self.root.join("record.json")) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(failed(error)),
        }
        access::sync_dir(&self.root)?;
        // Keep the session lock file forever; a new opener must lock the same inode.
        for name in ["working.slop", self.record.base_copy.as_str()] {
            let _ = fs::remove_file(self.root.join(name));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Batch, Origin, file,
        store::{Mode, Store},
    };
    use serde_json::json;
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        static REGISTRY: std::sync::Once = std::sync::Once::new();
        REGISTRY.call_once(|| {
            crate::registry::use_folder(
                &std::env::temp_dir().join(format!("hitslop-publication-unit-{}", std::process::id())),
            )
            .unwrap()
        });
        let dir = tempfile::tempdir().unwrap();
        let stage = dir.path().join("build");
        fs::create_dir(&stage).unwrap();
        for name in ["ui.js", "preview.js", "icon.js"] {
            fs::write(stage.join(name), "export default {}").unwrap();
        }
        let input = json!({"packageFormat":crate::PACKAGE_FORMAT,"runtimeABI":crate::RUNTIME_ABI,
            "declaration":{"metadata":{"slug":"publication-fixture","title":"Fixture","description":"Storage fixture","author":{"name":"Test"},"categories":["utilities"]},
                "window":{"kind":"standard","width":320,"height":240},
                "document":{"kind":"object","properties":{"title":{"kind":"string"}}},"initial":{"title":"Initial"},
                "theme":[],"commands":[],"views":{"preview":true,"icon":true,"export":false}},
            "roles":{"ui":"ui.js","preview":"preview.js","icon":"icon.js"},
            "resources": (["ui.js","preview.js","icon.js"].map(|key|json!({"kind":"app","key":key,"mediaType":"text/javascript","path":key}))),"artwork":{}});
        let template = dir.path().join("template.slop");
        file::pack(&input.to_string(), &stage, &template, None).unwrap();
        let target = dir.path().join("document.slop");
        file::create_document(&template, &target).unwrap();
        (dir, target)
    }
    fn edit(store: &Store, title: &str) {
        let mut doc = store.document().unwrap();
        let batch: Batch =
            serde_json::from_value(json!({"intents":[{"type":"set","path":["title"],"value":title}]})).unwrap();
        doc.apply_batch(batch, Origin::Agent).unwrap();
        store.write(&store.job(&mut doc).unwrap().unwrap()).unwrap();
    }
    fn title(path: &Path) -> String {
        Store::open(path, Mode::Snapshot).unwrap().document().unwrap().reading().unwrap().value["title"]
            .as_str()
            .unwrap()
            .into()
    }
    #[test]
    fn private_saves_publish_complete_files_and_transfer_the_public_lease() {
        let (dir, target) = fixture();
        let root = dir.path().join("session");
        let session = Session::at(&target, &root).unwrap();
        let store = Store::open_backing(&target, Mode::Document, Some(session)).unwrap();
        store.publish_discovery("test owner").unwrap();
        let assets = store.resource_reader().unwrap();
        let first = Stamp::read(&target).unwrap();
        edit(&store, "Published");
        assert_eq!(title(&target), "Initial", "autosaves never modify the public SQLite file");
        assert_eq!(Stamp::read(&target).unwrap(), first);
        assert!(matches!(Lease::acquire(&target), Err(Error::Locked)));
        let working_modified = fs::metadata(root.join("working.slop")).unwrap().modified().unwrap();
        store.publish().unwrap();
        assert_eq!(fs::metadata(&target).unwrap().modified().unwrap(), working_modified);
        assert_eq!(title(&target), "Published");
        assert_ne!(Stamp::read(&target).unwrap().identity(), first.identity());
        assert_eq!(crate::registry::discovery(&target).unwrap().as_deref(), Some("test owner"));
        assert!(matches!(Lease::acquire(&target), Err(Error::Locked)));
        assert!(assets.read_range(file::ResourceRoute::App, "ui.js", 0, 512).unwrap().is_some());
        let published = Stamp::read(&target).unwrap();
        store.publish().unwrap();
        assert_eq!(Stamp::read(&target).unwrap(), published, "an unchanged flush does not replace the file");
        edit(&store, "Closed");
        store.close().unwrap();
        assert_eq!(title(&target), "Closed");
        assert!(Lease::acquire(&target).is_ok());
        assert!(!root.join("record.json").exists());
        assert!(!target.with_extension("slop-journal").exists());
        assert!(
            !fs::read_dir(dir.path()).unwrap().any(|f| f
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".hitslop-publish"))
        );
    }
    #[test]
    fn abandoned_session_recovers_local_edits_and_conflict_allows_a_rescue_copy() {
        let (dir, target) = fixture();
        let root = dir.path().join("session");
        let store = Store::open_backing(&target, Mode::Document, Some(Session::at(&target, &root).unwrap())).unwrap();
        edit(&store, "Recovered");
        drop(store); // Termination: normal close and publication did not run.
        assert_eq!(title(&target), "Initial");
        let store = Store::open_backing(&target, Mode::Document, Some(Session::at(&target, &root).unwrap())).unwrap();
        assert_eq!(store.document().unwrap().reading().unwrap().value["title"], "Recovered");
        store.publish().unwrap();
        edit(&store, "Keep my edits");
        let external = dir.path().join("external.slop");
        fs::copy(&target, &external).unwrap();
        let other = Store::open(&external, Mode::Document).unwrap();
        edit(&other, "Other device");
        other.close().unwrap();
        fs::rename(external, &target).unwrap();
        assert!(store.publish().unwrap_err().to_string().contains("iCloud copy changed"));
        assert!(store.close().is_err(), "failed publication must retain ownership");
        assert!(matches!(Session::at(&target, &root), Err(Error::Locked)));
        let rescue = dir.path().join("rescue.slop");
        store.copy_clean(&rescue, &[]).unwrap();
        assert_eq!(title(&rescue), "Keep my edits");
        assert_eq!(title(&target), "Other device");
    }
    #[test]
    fn page_barriers_and_rescue_copy_work_after_cloud_publication_fails() {
        use crate::{
            library::snapshot::request,
            owner::{Owner, Request},
        };
        use std::sync::Arc;
        let (dir, target) = fixture();
        let root = dir.path().join("session");
        let session = Session::at(&target, &root).unwrap();
        let store = Arc::new(Store::open_backing(&target, Mode::Document, Some(session)).unwrap());
        let owner = Owner::from_store(target.clone(), store, Mode::Document, Arc::new(|_| {}), None).unwrap();
        request(
            &owner,
            Request::Apply {
                batch: serde_json::from_value(
                    json!({"intents":[{"type":"set","path":["title"],"value":"Rescued live edits"}]}),
                )
                .unwrap(),
                origin: Origin::Window,
            },
        )
        .unwrap();
        let replacement = dir.path().join("replacement.slop");
        fs::copy(&target, &replacement).unwrap();
        fs::rename(replacement, &target).unwrap();
        owner.attach("recovery".into());
        let (send, receive) = std::sync::mpsc::channel();
        crate::command::page(&owner, "recovery".into(), r#"{"method":"flush"}"#, move |reply| {
            send.send(reply).unwrap();
        });
        let reply = receive.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert!(
            matches!(reply, crate::command::PageDispatch::Reply { failure: None, .. }),
            "a page flush must save locally even when publication conflicts"
        );
        assert!(request(&owner, Request::Flush).unwrap_err().contains("iCloud copy changed"));
        let rescued = dir.path().join("rescued.slop");
        request(&owner, Request::Copy { destination: rescued.clone(), preview: None, icon: None }).unwrap();
        assert_eq!(title(&rescued), "Rescued live edits");
        assert_eq!(title(&target), "Initial");
    }
    #[test]
    fn cloud_creation_publishes_a_new_complete_file_and_never_overwrites() {
        let (dir, _) = fixture();
        let source = reader(&dir.path().join("template.slop")).unwrap();
        let target = dir.path().join("created.slop");
        super::super::copy::copy_to_cloud(&source, &target, true, true, None).unwrap();
        assert_eq!(title(&target), "Initial");
        let before = fs::read(&target).unwrap();
        assert!(super::super::copy::copy_to_cloud(&source, &target, true, true, None).is_err());
        assert_eq!(fs::read(&target).unwrap(), before);
        assert!(!target.with_extension("slop-journal").exists());
    }
    #[test]
    fn recovery_reconciles_a_replacement_without_its_receipt() {
        let (dir, target) = fixture();
        let root = dir.path().join("session");
        let session = Session::at(&target, &root).unwrap();
        let working = session.working();
        let store = Store::open(&working, Mode::Document).unwrap();
        edit(&store, "Already replaced");
        store.close().unwrap();
        let mut session = session;
        let next = "base-pending.slop";
        access::clone_file(&working, &root.join(next)).unwrap();
        let staged = target.with_file_name(format!(".hitslop-publish-{}.tmp", session.record.token));
        access::clone_file(&working, &staged).unwrap();
        session.record.pending = Some(Pending {
            staged: staged.clone(),
            inode: Some(Stamp::read(&staged).unwrap().identity()),
            base_copy: next.into(),
        });
        session.persist().unwrap();
        access::replace(&staged, &target).unwrap();
        drop(session); // Killed after replace, before recording its result.
        let recovered = Session::at(&target, &root).unwrap();
        assert!(recovered.record.pending.is_none());
        assert_eq!(title(&recovered.working()), "Already replaced");
        assert_eq!(title(&target), "Already replaced");
    }
}
