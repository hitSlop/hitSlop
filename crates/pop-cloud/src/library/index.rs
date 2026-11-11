//! The device index: its schema and every statement run against it. Projection reads one
//! SQLite snapshot before inspecting files.
use super::*;
use pop_sharing_protocol::{document::JoinedDocument, library::AccountEntry};

/// These tables. Any other version is refused, and nothing in it is changed.
const INDEX_VERSION: u32 = 2;
/// The device index, beside the managed `documents` and per-account `accounts` folders.
const INDEX: &str = "index.sqlite";

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

/// The index's `role` column, read back by `Library::project`.
pub(super) fn stored_role(role: DocumentRole) -> &'static str {
    match role {
        DocumentRole::Owner => "owner",
        DocumentRole::Editor => "editor",
    }
}

pub(super) fn open(root: &Path) -> Result<Connection> {
    let db = Connection::open(root.join(INDEX))?;
    let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > INDEX_VERSION {
        return Err(Error {
            issue: Issue::RequiresUpdate,
            message: "This local index requires a newer pop; existing files are retained".into(),
        });
    }
    db.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS documents(account TEXT NOT NULL,id TEXT NOT NULL,title TEXT NOT NULL,template TEXT NOT NULL,path TEXT NOT NULL,version TEXT NOT NULL DEFAULT '',role TEXT,revision INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0,problem TEXT,pending_enable INTEGER NOT NULL DEFAULT 0,artwork_version TEXT NOT NULL DEFAULT '',artwork_digest TEXT NOT NULL DEFAULT '',file_stamp TEXT NOT NULL DEFAULT '',listed INTEGER NOT NULL DEFAULT 0,sharing_enabled INTEGER,opened INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(account,id));
            ",
    )?;
    db.pragma_update(None, "user_version", INDEX_VERSION)?;
    Ok(db)
}

/// Every index row of this document identity with its resolved path, across accounts.
pub(super) fn indexed(db: &Connection, root: &Path, id: &str) -> Result<Vec<(String, PathBuf)>> {
    let rows = db
        .prepare("SELECT account,path FROM documents WHERE id=?1")?
        .query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter().map(|(account, path)| Ok((account, paths::resolve(root, &path)?))).collect()
}
pub(super) fn path_known(db: &Connection, stored: &str) -> Result<bool> {
    Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM documents WHERE path=?1)", [stored], |row| row.get(0))?)
}
pub(super) fn account_paths(db: &Connection) -> Result<Vec<String>> {
    Ok(db
        .prepare("SELECT DISTINCT path FROM documents WHERE account<>''")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}
/// Ids selected for synchronization whose first upload has not completed.
pub(super) fn pending_enable(db: &Connection, account: &str) -> Result<Vec<String>> {
    Ok(db
        .prepare("SELECT id FROM documents WHERE account=?1 AND pending_enable=1 AND deleted=0")?
        .query_map([account], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub(super) fn mark_opened_anywhere(db: &Connection, account: &str, id: &str) -> Result<()> {
    db.execute("UPDATE documents SET opened=1 WHERE id=?1 AND (account=?2 OR account='')", params![id, account])?;
    Ok(())
}
pub(super) fn mark_opened(db: &Connection, account: &str, id: &str) -> Result<()> {
    db.execute("UPDATE documents SET opened=1 WHERE account=?1 AND id=?2", params![account, id])?;
    Ok(())
}
pub(super) fn insert_local(
    db: &Connection,
    id: &str,
    title: &str,
    template: &str,
    path: &str,
    version: &str,
) -> Result<()> {
    db.execute(
        "INSERT INTO documents(account,id,title,template,path,version) VALUES('',?1,?2,?3,?4,?5)",
        params![id, title, template, path, version],
    )?;
    Ok(())
}
pub(super) fn delete_local(db: &Connection, account: &str, id: &str) -> Result<()> {
    db.execute("DELETE FROM documents WHERE account=?1 AND id=?2", params![account, id])?;
    Ok(())
}
pub(super) fn update_cached(
    db: &Connection,
    account: &str,
    id: &str,
    version: &str,
    artwork_digest: &str,
    stamp: &str,
    artwork_source: Option<&str>,
) -> Result<()> {
    db.execute("UPDATE documents SET version=?3,artwork_digest=?4,file_stamp=?5,artwork_version=coalesce(?6,artwork_version) WHERE account=?1 AND id=?2",params![account,id,version,artwork_digest,stamp,artwork_source])?;
    Ok(())
}
pub(super) fn insert_recovery(
    db: &Connection,
    account: &str,
    id: &str,
    title: &str,
    template: &str,
    path: &str,
    version: &str,
) -> Result<()> {
    db.execute(
        "INSERT OR IGNORE INTO documents(account,id,title,template,path,version) VALUES(?1,?2,?3,?4,?5,?6)",
        params![account, id, title, template, path, version],
    )?;
    Ok(())
}

/// Selecting synchronization durably binds this exact local file to the account.
pub(super) fn begin_enable(
    db: &mut Connection,
    account: &str,
    id: &str,
    title: &str,
    template: &str,
    path: &str,
    version: &str,
) -> Result<()> {
    let tx = db.transaction()?;
    tx.execute("INSERT INTO documents(account,id,title,template,path,version,role,pending_enable) VALUES(?1,?2,?3,?4,?5,?6,'owner',1) ON CONFLICT(account,id) DO UPDATE SET pending_enable=1,path=excluded.path,role='owner',deleted=0",params![account,id,title,template,path,version])?;
    tx.execute("DELETE FROM documents WHERE account='' AND id=?1", [id])?;
    tx.commit()?;
    Ok(())
}
pub(super) fn record_problem(db: &Connection, account: &str, id: &str, message: &str) -> Result<()> {
    db.execute("UPDATE documents SET problem=?3 WHERE account=?1 AND id=?2", params![account, id, message])?;
    Ok(())
}
pub(super) fn finish_enable(db: &mut Connection, account: &str, id: &str) -> Result<()> {
    let tx = db.transaction()?;
    tx.execute("UPDATE documents SET pending_enable=0,problem=NULL WHERE account=?1 AND id=?2", params![account, id])?;
    tx.execute("DELETE FROM documents WHERE account='' AND id=?1", [id])?;
    tx.commit()?;
    Ok(())
}
pub(super) fn remember_sharing(db: &Connection, account: &str, id: &str, sharing: bool) -> Result<()> {
    db.execute(
        "UPDATE documents SET sharing_enabled=?3 WHERE account=?1 AND id=?2 AND role IS NOT NULL AND deleted=0",
        params![account, id, sharing],
    )?;
    Ok(())
}
pub(super) fn upsert_joined(db: &Connection, account: &str, joined: &JoinedDocument, path: &str) -> Result<()> {
    let access = &joined.access;
    db.execute("INSERT INTO documents(account,id,title,template,path,role,opened,sharing_enabled) VALUES(?1,?2,?3,?4,?5,?6,coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0),?7) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role=excluded.role,deleted=0,sharing_enabled=excluded.sharing_enabled",params![account,access.document,joined.title,joined.template_id,path,stored_role(access.role),access.sharing])?;
    Ok(())
}
/// A departing document stays listed until the cloud confirms its removal.
pub(super) fn mark_departing(db: &Connection, account: &str, id: &str) -> Result<()> {
    db.execute("UPDATE documents SET deleted=1,listed=1 WHERE account=?1 AND id=?2", params![account, id])?;
    Ok(())
}
pub(super) fn restore_departing(db: &Connection, account: &str, id: &str) -> Result<()> {
    db.execute("UPDATE documents SET deleted=0 WHERE account=?1 AND id=?2", params![account, id])?;
    Ok(())
}

/// Ids the last complete snapshot listed that are still live.
pub(super) fn listed_ids(db: &Connection, account: &str) -> Result<Vec<String>> {
    Ok(db
        .prepare("SELECT id FROM documents WHERE account=?1 AND listed=1 AND deleted=0 AND pending_enable=0")?
        .query_map([account], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}
/// An upload can finish while an earlier snapshot is queued. Protect an unlisted local
/// selection for that first snapshot.
pub(super) fn list_selections(db: &Connection, account: &str) -> Result<()> {
    db.execute("UPDATE documents SET listed=1 WHERE account=?1 AND listed=0 AND pending_enable=0 AND role IS NOT NULL AND deleted=0", [account])?;
    Ok(())
}
pub(super) fn unlist_removed(db: &Connection, account: &str) -> Result<()> {
    db.execute("UPDATE documents SET listed=0 WHERE account=?1 AND deleted=1", [account])?;
    Ok(())
}
/// `(revision, deleted, pending_enable, listed)` of the account's row.
pub(super) fn prior(db: &Connection, account: &str, id: &str) -> Result<Option<(i64, bool, bool, bool)>> {
    Ok(db
        .query_row(
            "SELECT revision,deleted,pending_enable,listed FROM documents WHERE account=?1 AND id=?2",
            params![account, id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?, r.get::<_, bool>(2)?, r.get::<_, bool>(3)?)),
        )
        .optional()?)
}
/// The stored path of this document's device-local (account-less) row.
pub(super) fn local_path(db: &Connection, id: &str) -> Result<Option<String>> {
    Ok(db
        .query_row("SELECT path FROM documents WHERE account='' AND id=?1", [id], |r| r.get::<_, String>(0))
        .optional()?)
}
pub(super) fn upsert_listed(
    db: &Connection,
    account: &str,
    entry: &AccountEntry,
    path: &str,
    revision: i64,
) -> Result<()> {
    db.execute("INSERT INTO documents(account,id,title,template,path,role,revision,listed,opened,sharing_enabled) VALUES(?1,?2,?3,?4,?5,?6,?7,1,coalesce((SELECT opened FROM documents WHERE account='' AND id=?2),0),?8) ON CONFLICT(account,id) DO UPDATE SET title=excluded.title,template=excluded.template,role=excluded.role,revision=excluded.revision,listed=1,deleted=0,sharing_enabled=excluded.sharing_enabled",
        params![account, entry.document, entry.title, entry.template_id, path, stored_role(entry.role), revision, entry.sharing])?;
    Ok(())
}
pub(super) fn is_live(db: &Connection, account: &str, id: &str) -> Result<bool> {
    Ok(db.query_row(
        "SELECT count(*) FROM documents WHERE account=?1 AND id=?2 AND deleted=0",
        params![account, id],
        |r| r.get::<_, i64>(0),
    )? > 0)
}
pub(super) fn mark_removed(db: &Connection, account: &str, id: &str) -> Result<()> {
    db.execute("UPDATE documents SET deleted=1,listed=0 WHERE account=?1 AND id=?2", params![account, id])?;
    Ok(())
}
