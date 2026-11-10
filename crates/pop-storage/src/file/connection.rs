//! Connection policy and writer-owned transactions.
use super::check::{markers, one};
use super::{STORAGE_VERSION, check};
use crate::error::{Result, failed, sqlite};
use rusqlite::config::DbConfig;
use rusqlite::limits::Limit;
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The file's path with its folder resolved: NOFOLLOW refuses a symbolic link anywhere in
/// a path, while the file itself must not be one.
pub fn resolve(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| failed("Invalid document path"))?;
    let folder = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Ok(fs::canonicalize(folder).map_err(|e| failed(format!("Cannot resolve the document's folder: {e}")))?.join(name))
}
/// An existing regular document leaf, resolving only its parent folders so linked
/// parents remain valid and a linked leaf is refused before SQLite opens it.
pub fn existing(path: &Path) -> Result<PathBuf> {
    let path = resolve(path)?;
    crate::registry::regular(&path)?;
    Ok(path)
}
/// Every connection: no symbolic links, defensive mode, no trusted schema, cell checks, no
/// memory mapping, and values no longer than the largest stored one.
pub(crate) fn connect(path: &Path, flags: OpenFlags, busy: Duration) -> Result<Connection> {
    let path = if flags.contains(OpenFlags::SQLITE_OPEN_CREATE) { resolve(path)? } else { existing(path)? };
    let conn =
        Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW)
            .map_err(sqlite("open"))?;
    conn.busy_timeout(busy).map_err(sqlite("open"))?;
    configure_connection(&conn)?;
    Ok(conn)
}
fn configure_connection(conn: &Connection) -> Result<()> {
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true).map_err(sqlite("open"))?;
    // Closing never checkpoints: a file in WAL mode, which only a newer build writes, is
    // refused, never rewritten.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true).map_err(sqlite("open"))?;
    conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA cell_size_check=ON; PRAGMA mmap_size=0;")
        .map_err(sqlite("open"))?;
    let version = one(conn, "PRAGMA user_version")?;
    if version <= STORAGE_VERSION {
        conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, pop_contracts::wire::STORAGE_BYTES as i32)
            .map_err(sqlite("open"))?;
    }
    Ok(())
}
/// The writer's durability: a rollback journal that exists only while a save commits, and
/// a full sync of every commit. Deleted content is zeroed where that costs no extra write
/// (FAST): Apple's SQLite does so by default, the bundled SQLite of the Linux engines
/// does not, and a shared file should not depend on which one wrote it.
pub(crate) fn configure_writer(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON; PRAGMA secure_delete=FAST;",
    )
    .map_err(sqlite("configure"))
}
/// A writer's connection: read-write, creating a new file when `create`. An existing file
/// is checked before `configure_writer`, so a file this build refuses is never written.
pub(crate) fn writer(path: &Path, create: bool) -> Result<Connection> {
    let create = if create { OpenFlags::SQLITE_OPEN_CREATE } else { OpenFlags::empty() };
    connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE | create, Duration::from_secs(2))
}
/// A reader's connection: read-write with `query_only`. A read-only connection beside this
/// process's writer makes the platform SQLite fail the writer's locks, and sometimes its
/// own, with EBADF (SQLITE_IOERR_LOCK). Like any opener, it completes the rollback of a
/// crashed write, which restores the saved state it reads. A file this process can't write
/// opens read-only; no writer can be in this process then.
pub(crate) fn reader(path: &Path) -> Result<Connection> {
    let busy = Duration::from_secs(5);
    match connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE, busy) {
        Ok(conn) => {
            conn.execute_batch("PRAGMA query_only=ON").map_err(sqlite("open"))?;
            Ok(conn)
        }
        Err(_) => connect(path, OpenFlags::SQLITE_OPEN_READ_ONLY, busy),
    }
}
/// All durable writes pass through this transaction. A future storage migration runs
/// inside `begin_write`, under the writer lock and never on reads, and the commit checks
/// the migrated file in full before it is durable.
pub(crate) struct WriteTransaction<'a> {
    tx: Transaction<'a>,
    validate: bool,
}
impl std::ops::Deref for WriteTransaction<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.tx
    }
}
impl WriteTransaction<'_> {
    pub(crate) fn commit(self) -> Result<()> {
        if self.validate {
            check(&self.tx, true)?;
        }
        self.tx.commit().map_err(sqlite("commit"))
    }
}
/// Begins a write after rechecking the file's markers inside the transaction.
pub(crate) fn begin_write<'a>(conn: &'a Connection, action: &str) -> Result<WriteTransaction<'a>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite(action))?;
    markers(&tx)?;
    Ok(WriteTransaction { tx, validate: false })
}
pub(crate) fn initialize(conn: &Connection) -> Result<WriteTransaction<'_>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("create"))?;
    Ok(WriteTransaction { tx, validate: true })
}
