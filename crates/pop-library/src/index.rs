//! Typed local-index rows. Projection reads one SQLite snapshot before inspecting files.
use super::*;

const COLUMNS: &str = "account,id,title,template,path,version,role,revision,deleted,problem,artwork_version,artwork_digest,file_stamp,pending_enable,sharing_enabled,opened";

pub(super) struct IndexedDocument {
    pub account: String,
    pub id: String,
    pub title: String,
    pub template: String,
    pub path: String,
    pub version: String,
    pub role: Option<String>,
    pub revision: i64,
    pub deleted: bool,
    pub problem: Option<String>,
    pub artwork_version: String,
    pub artwork_digest: String,
    pub file_stamp: String,
    pub pending: bool,
    pub sharing_enabled: Option<bool>,
    pub opened: bool,
}
impl IndexedDocument {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            account: row.get(0)?,
            id: row.get(1)?,
            title: row.get(2)?,
            template: row.get(3)?,
            path: row.get(4)?,
            version: row.get(5)?,
            role: row.get(6)?,
            revision: row.get(7)?,
            deleted: row.get(8)?,
            problem: row.get(9)?,
            artwork_version: row.get(10)?,
            artwork_digest: row.get(11)?,
            file_stamp: row.get(12)?,
            pending: row.get(13)?,
            sharing_enabled: row.get(14)?,
            opened: row.get(15)?,
        })
    }
    pub fn one(db: &Connection, account: &str, id: &str) -> Result<Self> {
        Ok(db.query_row(
            &format!("SELECT {COLUMNS} FROM documents WHERE id=?1 AND (account=?2 OR account='') ORDER BY (account=?2) DESC LIMIT 1"),
            params![id, account], Self::read,
        )?)
    }
    pub fn all(db: &Connection, account: &str) -> Result<Vec<Self>> {
        Ok(db.prepare(&format!("SELECT {COLUMNS} FROM documents AS d WHERE account=?1 OR (account='' AND NOT EXISTS(SELECT 1 FROM documents AS preferred WHERE preferred.id=d.id AND preferred.account=?1)) ORDER BY title"))?
            .query_map([account], Self::read)?.collect::<rusqlite::Result<_>>()?)
    }
}
