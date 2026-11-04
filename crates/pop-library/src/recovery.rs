//! Findable private recovery copies for confirmed catalog epoch cutoffs.
use crate::{Inner, Library, Result, failed};
use pop_storage::store::{Mode, Store};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

impl Library {
    pub(crate) fn recovery_slot(
        &self,
        inner: &mut Inner,
        id: &str,
        transition: &str,
        source_version: &str,
    ) -> Result<(PathBuf, String)> {
        let digest = Sha256::digest(source_version.as_bytes());
        let suffix: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        let transition = format!("{transition}-{suffix}");
        let filename = format!("{id}-{transition}.pop");
        inner.db.execute(
            "INSERT OR IGNORE INTO catalog_recoveries(document_id,transition,filename) VALUES(?1,?2,?3)",
            params![id, &transition, filename],
        )?;
        let saved: String = inner.db.query_row(
            "SELECT filename FROM catalog_recoveries WHERE document_id=?1 AND transition=?2",
            params![id, &transition],
            |row| row.get(0),
        )?;
        if saved != filename {
            return Err(failed("Invalid recovery catalog location"));
        }
        Ok((self.root.join("recoveries").join(saved), transition))
    }
    pub(crate) fn register_recovery_slot(&self, inner: &mut Inner, id: &str, transition: &str) -> Result<()> {
        let record: Option<(String, bool)> = inner
            .db
            .query_row(
                "SELECT filename,ready FROM catalog_recoveries WHERE document_id=?1 AND transition=?2",
                params![id, transition],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((filename, ready)) = record else {
            return Ok(());
        };
        if filename != format!("{id}-{transition}.pop") {
            return Err(failed("Invalid recovery catalog location"));
        }
        let path = self.root.join("recoveries").join(&filename);
        if !path.exists() {
            return Ok(());
        }
        let recovery = Store::open(&path, Mode::Snapshot)?;
        recovery.document()?;
        let recovered_id = recovery.app().document_uuid.clone().ok_or_else(|| failed("Recovery is not a document"))?;
        Self::valid_id(&recovered_id)?;
        let share = recovery.share()?;
        if recovered_id == id
            || share.is_some_and(|share| share.endpoint != pop_storage::LIBRARY_SHARE_ENDPOINT || !ready)
        {
            return Err(failed("Recovery identity is not independent"));
        }
        let (title, template): (String, String) =
            inner.db.query_row("SELECT title,template FROM documents WHERE id=?1", [id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        let old_id: Option<String> = inner.db.query_row(
            "SELECT recovery_id FROM catalog_recoveries WHERE document_id=?1 AND transition=?2",
            params![id, transition],
            |row| row.get(0),
        )?;
        if old_id.as_ref().is_some_and(|old| old != &recovered_id) {
            return Err(failed("Recovery catalog identity changed"));
        }
        let tx = inner.db.transaction()?;
        tx.execute("INSERT INTO documents(id,title,template,local_only) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET local_only=CASE WHEN ?4=0 THEN 0 ELSE documents.local_only END", params![recovered_id,format!("Recovered edits — {title}"),template,!ready])?;
        tx.execute(
            "UPDATE catalog_recoveries SET recovery_id=?3 WHERE document_id=?1 AND transition=?2",
            params![id, transition, recovered_id],
        )?;
        tx.commit()?;
        self.recovery_paths.lock().unwrap().insert(recovered_id, filename);
        Ok(())
    }
    pub(crate) fn recover_catalog_slots(&self) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let slots = inner
            .db
            .prepare("SELECT document_id,transition FROM catalog_recoveries")?
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, transition) in slots {
            self.register_recovery_slot(&mut inner, &id, &transition)?;
        }
        Ok(())
    }
}
