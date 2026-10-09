//! Immutable attachment storage, verification and reclamation.
use super::*;

impl Store {
    /// Stores `bytes` and returns its reference; storing the same bytes again is a no-op.
    /// The blob is committed before any edit can save a reference to it.
    pub fn put_attachment(&self, bytes: &[u8]) -> Result<Attachment> {
        self.check(true)?;
        if bytes.len() > crate::ATTACHMENT_FILE_BYTES {
            return Err(rejected(
                crate::Code::TooLarge,
                format!("Attachment exceeds {} MiB", crate::ATTACHMENT_FILE_BYTES >> 20),
            ));
        }
        let id = attachment_id(bytes);
        self.connected(&mut lock(&self.backing).conn, |conn| self.store_attachment(conn, &id, bytes))?;
        lock(&self.account).changed = true;
        Ok(Attachment { id, bytes: bytes.len() as u64, media_type: crate::media::attachment_type(bytes).into() })
    }

    fn store_attachment(&self, conn: &Connection, id: &str, bytes: &[u8]) -> Result<()> {
        let tx = file::begin_write(conn, "store attachment")?;
        // The same bytes are already stored only if the stored copy is intact; damage is
        // refused, never repaired in passing, so a successful import is always readable.
        if stored_attachment(&tx, id)?.is_none() {
            let (count, _, total) = rows::attachment_sizes(&tx)?;
            if !file::attachments_fit(count + 1, bytes.len() as i64, total + bytes.len() as i64) {
                return Err(rejected(
                    crate::Code::TooLarge,
                    format!(
                        "Document attachment limit reached ({} MiB or {} files)",
                        crate::ATTACHMENT_BYTES >> 20,
                        crate::ATTACHMENT_COUNT
                    ),
                ));
            }
            rows::put_attachment(&tx, id, bytes)?;
        }
        tx.commit()
    }

    /// An attachment's bytes, verified against its identity.
    pub fn attachment(&self, id: &str) -> Result<Vec<u8>> {
        self.check(false)?;
        if !crate::wire::valid_attachment_id(id) {
            return Err(rejected(crate::Code::InvalidId, "Invalid attachment ID"));
        }
        self.read(|conn| stored_attachment(conn, id))?
            .ok_or_else(|| rejected(crate::Code::PathNotFound, "Attachment not found"))
    }

    /// Every stored attachment, by identity.
    pub fn attachments(&self) -> Result<Vec<Attachment>> {
        self.check(false)?;
        let stored = self.read(rows::attachment_list)?;
        Ok(stored.into_iter().map(|(id, bytes, media_type)| Attachment { id, bytes, media_type }).collect())
    }

    /// Deletes the attachments the saved state no longer references, in a transaction of
    /// their own, and returns how many. The owner calls it as it closes, after its final
    /// save, when no import can be waiting for its reference. A session that saved no edit
    /// and stored no attachment reclaims nothing, so reading never writes the file.
    pub fn reclaim_attachments(&self) -> Result<usize> {
        self.check(true)?;
        if !lock(&self.account).changed {
            return Ok(0);
        }
        self.connected(&mut lock(&self.backing).conn, |conn| {
            let tx = file::begin_write(conn, "reclaim attachments")?;
            let deleted = delete_unreferenced(&tx, None, self.app.app.spec())?;
            tx.commit()?;
            Ok(deleted)
        })
    }
}
/// Deletes the attachments `doc`, or the saved state when none is given, does not
/// reference (`Document::attachment_references`), inside the caller's transaction.
pub(super) fn delete_unreferenced(conn: &Connection, doc: Option<&Document>, app: &crate::AppSpec) -> Result<usize> {
    let stored: Vec<String> = rows::attachment_list(conn)?.into_iter().map(|(id, _, _)| id).collect();
    if stored.is_empty() {
        return Ok(0);
    }
    let loaded;
    let doc = match doc {
        Some(doc) => doc,
        None => {
            loaded = load(conn, app)?.0;
            &loaded
        }
    };
    let referenced = doc.attachment_references(&stored);
    let mut deleted = 0;
    for id in stored.iter().filter(|id| !referenced.contains(*id)) {
        deleted += rows::delete_attachment(conn, id)?;
    }
    Ok(deleted)
}

/// A stored attachment: its identity (the SHA-256 of its bytes) and size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub media_type: String,
    pub id: String,
    pub bytes: u64,
}
fn attachment_id(bytes: &[u8]) -> String {
    crate::hex(Sha256::digest(bytes).as_slice())
}
/// A stored attachment's bytes, verified against their identity: damage is an error, never
/// served.
fn stored_attachment(conn: &Connection, id: &str) -> Result<Option<Vec<u8>>> {
    let bytes = rows::read_attachment(conn, id)?;
    if bytes.as_deref().is_some_and(|bytes| attachment_id(bytes) != id) {
        return Err(failed("Attachment checksum mismatch; keep the file for recovery"));
    }
    Ok(bytes)
}
