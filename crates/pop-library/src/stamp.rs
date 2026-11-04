//! A bounded ordering fence for the latest CloudKit record, independent of room history.
use crate::{CatalogAuthority, Error, Issue, Library, Result, failed};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogStamp {
    pub revision: u64,
    pub change_tag: String,
}

impl CatalogStamp {
    pub fn validate(&self) -> Result<()> {
        if self.revision == 0
            || self.revision > i64::MAX as u64
            || self.change_tag.is_empty()
            || self.change_tag.len() > 4096
        {
            return Err(failed("Invalid catalog revision receipt"));
        }
        Ok(())
    }
}

pub(crate) fn read(db: &Connection, id: &str) -> Result<Option<CatalogStamp>> {
    Ok(db
        .query_row("SELECT revision,change_tag FROM catalog_stamps WHERE id=?1", [id], |r| {
            let revision: i64 = r.get(0)?;
            let revision = u64::try_from(revision)
                .ok()
                .filter(|revision| *revision > 0)
                .ok_or_else(|| rusqlite::Error::IntegralValueOutOfRange(0, revision))?;
            Ok(CatalogStamp { revision, change_tag: r.get(1)? })
        })
        .optional()?)
}

/// Called while holding the library lock, before touching canonical bytes or metadata.
pub(crate) fn check(db: &Connection, id: &str, incoming: Option<&CatalogStamp>) -> Result<()> {
    if let Some(incoming) = incoming {
        incoming.validate()?;
    }
    if let Some(current) = read(db, id)? {
        let Some(incoming) = incoming else {
            return Err(stale("A legacy record cannot replace a revisioned catalog record"));
        };
        if incoming.revision < current.revision
            || (incoming.revision == current.revision && incoming.change_tag != current.change_tag)
        {
            return Err(stale("The incoming catalog record is stale"));
        }
    }
    Ok(())
}

pub(crate) fn authority(
    current: &CatalogAuthority,
    next: &CatalogAuthority,
    incoming: Option<&CatalogStamp>,
) -> Result<()> {
    if incoming.is_some() {
        // A receiver may miss any number of complete sharing cycles. Ordering is
        // proved by the record CAS, not by retaining a chain of old room receipts.
        next.validate()
    } else {
        if *next != CatalogAuthority::default() {
            return Err(failed("A format 2 catalog record requires a revision receipt"));
        }
        crate::catalog::validate_remote(current, next)
    }
}

/// Persist the latest validated authority fence, not proof of completed installation.
/// Equal-tag retries may finish an interrupted file/metadata installation.
pub(crate) fn commit(db: &Connection, id: &str, incoming: Option<&CatalogStamp>) -> Result<()> {
    if let Some(incoming) = incoming {
        incoming.validate()?;
        let revision = i64::try_from(incoming.revision).map_err(failed)?;
        db.execute("INSERT INTO catalog_stamps(id,revision,change_tag) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,change_tag=excluded.change_tag", params![id,revision,incoming.change_tag])?;
    }
    Ok(())
}

impl Library {
    pub fn catalog_stamp(&self, id: &str) -> Result<Option<CatalogStamp>> {
        Self::valid_id(id)?;
        read(&self.inner.lock().unwrap().db, id)
    }
}

fn stale(message: &str) -> Error {
    Error { issue: Issue::StaleCatalog, message: message.into() }
}
