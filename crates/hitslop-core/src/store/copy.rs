//! Native clean copies, capture sources, backups and artwork.
use super::*;

impl Store {
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
    /// use it after flushing; the original and its session are untouched.
    pub fn copy_clean(&self, dest: &Path, artwork: &[(Artwork, &[u8])]) -> Result<()> {
        file::document_destination(dest)?;
        self.check(true)?;
        let artwork = optimized_artwork(artwork)?;
        let app = self.app.app.spec();
        self.read(|conn| file::copy(conn, dest, false, true, Some(&|staged: &Connection| clean(staged, app, &artwork))))
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
fn clean(conn: &Connection, app: &crate::AppSpec, artwork: &[(Artwork, Vec<u8>)]) -> Result<()> {
    let (doc, _) = load(conn, app)?;
    let state = doc.doc.export(ExportMode::shallow_snapshot(&doc.doc.oplog_frontiers())).map_err(failed)?;
    if !within(0, checkpoint_row(state.len())) {
        return Err(Error::Full);
    }
    let tx = file::begin_write(conn, "clean copy")?;
    rows::renew_document(&tx)?;
    rows::put_checkpoint(&tx, &state)?;
    rows::clear_updates(&tx)?;
    rows::clear_artwork(&tx)?;
    delete_unreferenced(&tx, Some(&doc), app)?;
    for (name, png) in artwork {
        rows::put_artwork(&tx, *name, png)?;
    }
    tx.commit()
}
