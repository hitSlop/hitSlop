//! Native clean copies, sharing seeds, capture sources, backups and artwork.
use super::*;

const RESOURCE_HASH_CHUNK_BYTES: u64 = 1 << 20;

impl Store {
    /// Files import reads one accepted snapshot without owning or modifying the source.
    /// The destination gets an independent identity, retaining the imported file's
    /// history and saved artwork. Import is not the sanitized Share a Copy operation.
    pub fn import_copy(source: &Path, dest: &Path) -> Result<()> {
        file::document_destination(dest)?;
        if let Some(folder) = dest.parent().filter(|folder| !folder.as_os_str().is_empty()) {
            std::fs::create_dir_all(folder).map_err(failed)?;
        }
        let conn = file::reader(source)?;
        let read = conn.unchecked_transaction().map_err(sqlite("read"))?;
        let accepted = file::opened(&read, source, true)?;
        super::validate_saved(&read, accepted.app.spec())?;
        let create = accepted.kind == file::Kind::Template;
        let imported = |staged: &Connection| {
            let tx = file::begin_write(staged, "import document")?;
            rows::renew_document(&tx)?;
            tx.commit()
        };
        file::copy(&read, dest, create, true, if create { None } else { Some(&imported) })
    }

    /// Writes the document's artwork through the writer's connection: a
    /// window renders it from the open document as it closes. Checked as `pack` checks it,
    /// and optimized at oxipng's fastest level, before the connection is taken: a close
    /// releases the writer lock only after this write.
    pub fn set_artwork(&self, artwork: &[(Artwork, &[u8])]) -> Result<()> {
        self.check(true)?;
        let optimized = optimized_artwork(artwork)?;
        self.connected(&mut lock(&self.backing).conn, |conn| {
            let tx = file::begin_write(conn, "write artwork")?;
            for (name, png) in &optimized {
                rows::put_artwork(&tx, *name, png)?;
            }
            tx.commit()
        })
    }

    /// Copies the open document to `dest` as a document of its own, from the writer's own
    /// connection so saves queue behind the copy: its current state without history, only
    /// the attachments that state references, and `artwork` (none when empty) in place of
    /// the original's, which can show what was since deleted. Duplicate and Share a Copy
    /// rebuild the accepted visible state from the actor's live snapshot, even when the
    /// source cannot save its full history. The original and its session are untouched.
    /// Blobs precede accepted references.
    pub fn copy_clean(&self, dest: &Path, state: &[u8], artwork: &[(Artwork, &[u8])]) -> Result<()> {
        file::document_destination(dest)?;
        self.copy_clean_unplaced(dest, state, artwork)
    }

    /// Stages an independent copy and checks its visible state before publishing it.
    /// The owner may release its original only after this succeeds.
    pub fn handoff_copy(
        &self,
        dest: &Path,
        state: &[u8],
        artwork: &[(Artwork, &[u8])],
        value: &serde_json::Value,
        theme: &std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        file::document_destination(dest)?;
        let dest = file::resolve(dest)?;
        if std::fs::symlink_metadata(&dest).is_ok() {
            return Err(rejected(crate::Code::Exists, "A file already exists there"));
        }
        let staged = file::Staged::beside(&dest)?;
        self.copy_clean_unplaced(staged.path(), state, artwork)?;
        let unusable = |why: String| failed(format!("The copy cannot replace this document: {why}"));
        let snapshot = Store::open(staged.path(), Mode::Snapshot).map_err(|e| unusable(e.to_string()))?;
        let reading = snapshot
            .document()
            .map_err(|e| e.to_string())
            .and_then(|document| document.reading().map_err(|e| e.message));
        let _ = snapshot.close();
        let reading = reading.map_err(unusable)?;
        if reading.value != *value || reading.theme != *theme {
            return Err(unusable("it does not match the document".into()));
        }
        staged.publish_new(&dest, true)?.confirmed()
    }

    /// `copy_clean` for a staged file beside a destination the caller has already checked
    /// with `file::document_destination`.
    pub(crate) fn copy_clean_unplaced(&self, dest: &Path, state: &[u8], artwork: &[(Artwork, &[u8])]) -> Result<()> {
        self.check(true)?;
        let artwork = optimized_artwork(artwork)?;
        let app = self.app.app.spec();
        self.read(|conn| {
            file::copy(conn, dest, false, true, Some(&|staged: &Connection| clean(staged, app, state, &artwork)))
        })
    }

    /// The public app resource inventory from one accepted SQLite snapshot, hashed
    /// through the bounded resource reader.
    pub fn sharing_manifest(&self) -> Result<pop_sharing_protocol::resources::Manifest> {
        use pop_sharing_protocol::resources::{self, Manifest, Resource};
        self.check(false)?;
        self.read(|connection| {
            let read = connection.unchecked_transaction().map_err(sqlite("read sharing resources"))?;
            let connection: &Connection = &read;
            let reader = file::ResourceReader::with_cache(connection, Default::default());
            let mut inventory = Vec::new();
            for key in rows::asset_keys(&read)? {
                if !resources::asset_key(&key) {
                    continue;
                }
                let info = reader
                    .info(file::ResourceRoute::App, &key)?
                    .ok_or_else(|| failed("Missing public app resource"))?;
                let mut hash = Sha256::new();
                let mut offset = 0;
                while offset < info.size {
                    let length = (info.size - offset).min(RESOURCE_HASH_CHUNK_BYTES);
                    let bytes = reader
                        .read_range(file::ResourceRoute::App, &key, offset, length)?
                        .ok_or_else(|| failed("Missing app resource bytes"))?;
                    if bytes.len() as u64 != length {
                        return Err(failed("Incomplete app resource bytes"));
                    }
                    hash.update(&bytes);
                    offset += length;
                }
                inventory.push(Resource {
                    key,
                    hash: crate::hex(&hash.finalize()),
                    size: info.size,
                    media_type: info.media_type,
                });
            }
            resources::validate(&inventory, false).map_err(failed)?;
            Ok(Manifest { app: crate::hex(&rows::app_digest(&read)?), resources: inventory })
        })
    }

    /// Copies the saved document to `dest` as the seed every member of a shared document
    /// starts from: the same identity and history, as one checkpoint, with only the
    /// attachments that state references and no artwork, which can show what was since
    /// deleted. Every replica's later edits build on this history, so it is never rebuilt.
    /// The owner flushes first; the original and its session are untouched.
    pub fn stage_seed(&self, dest: &Path) -> Result<()> {
        file::document_destination(dest)?;
        self.check(true)?;
        let app = self.app.app.spec();
        self.read(|source| {
            let document = load(source, app)?.0;
            let checkpoint = document.checkpoint().map_err(load_failure)?;
            if !within(0, checkpoint_row(checkpoint.len())) {
                return Err(Error::Full);
            }
            file::copy(
                source,
                dest,
                false,
                true,
                Some(&|staged: &Connection| {
                    let tx = file::begin_write(staged, "stage sharing seed")?;
                    rows::replace_history(&tx, &checkpoint)?;
                    rows::clear_artwork(&tx)?;
                    delete_unreferenced(&tx, Some(&document), app)?;
                    for (id, _, _) in rows::attachment_list(&tx)? {
                        super::attachments::stored_attachment(&tx, &id)?
                            .ok_or_else(|| failed("Staged attachment disappeared"))?;
                    }
                    tx.commit()?;
                    // Compact the private copy after logical deletion, including sources
                    // whose accepted SQLite settings do not reclaim free pages themselves.
                    staged.execute_batch("VACUUM").map_err(sqlite("compact sharing seed"))
                }),
            )
        })
    }

    /// A transport seed keeps the complete app and saved history, while attachments
    /// travel separately. Call on the immutable sharing snapshot, never the live file.
    pub fn seed_without_attachments(&self, dest: &Path) -> Result<()> {
        file::document_destination(dest)?;
        self.check(false)?;
        self.read(|source| {
            file::copy(
                source,
                dest,
                false,
                true,
                Some(&|staged: &Connection| {
                    let tx = file::begin_write(staged, "separate sharing attachments")?;
                    tx.execute("DELETE FROM attachments", []).map_err(sqlite("separate sharing attachments"))?;
                    tx.commit()?;
                    staged.execute_batch("VACUUM").map_err(sqlite("compact sharing seed"))
                }),
            )
        })
    }

    /// Copies the open document to `dest` as it is stored, without syncing: a capture's
    /// source, rendered once and then deleted.
    pub fn capture_source(&self, dest: &Path) -> Result<()> {
        self.check(true)?;
        self.read(|conn| file::copy(conn, dest, false, false, None))
    }

    /// Copies the open document to `dest` as it is stored, durably: the same document
    /// (its identity, history and artwork), unlike Duplicate's copy of its own. The owner
    /// flushes before queuing it.
    pub fn backup(&self, dest: &Path) -> Result<()> {
        file::document_destination(dest)?;
        self.check(true)?;
        self.read(|conn| file::copy(conn, dest, false, true, None))
    }
}
/// Artwork as a write stores it: checked as `pack` checks it, and optimized at oxipng's
/// fastest level, before any connection is taken.
fn optimized_artwork(artwork: &[(Artwork, &[u8])]) -> Result<Vec<(Artwork, Vec<u8>)>> {
    artwork
        .iter()
        .map(|&(name, png)| {
            file::check_artwork(name, png)?;
            Ok((name, file::optimize_png(png.to_vec(), 0)))
        })
        .collect()
}
/// A copy's state made its own, in one transaction: the current state without history,
/// the attachments it references, and `artwork` in place of the original's.
fn clean(conn: &Connection, app: &crate::AppSpec, state: &[u8], artwork: &[(Artwork, Vec<u8>)]) -> Result<()> {
    let doc = Document::open(app, state, &[]).map_err(load_failure)?;
    let state = doc.clean_checkpoint().map_err(load_failure)?;
    if !within(0, checkpoint_row(state.len())) {
        return Err(Error::Full);
    }
    let tx = file::begin_write(conn, "clean copy")?;
    rows::renew_document(&tx)?;
    rows::replace_history(&tx, &state)?;
    rows::clear_artwork(&tx)?;
    delete_unreferenced(&tx, Some(&doc), app)?;
    for (name, png) in artwork {
        rows::put_artwork(&tx, *name, png)?;
    }
    tx.commit()
}
