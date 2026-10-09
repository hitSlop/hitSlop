//! Connection policy and writer-owned transactions.
use super::check::{markers, one};
use super::{STORAGE_VERSION, check};
#[cfg(not(target_arch = "wasm32"))]
use crate::error::failed;
use crate::error::{Result, invalid, sqlite};
use rusqlite::{Connection, Transaction, TransactionBehavior, config::DbConfig, limits::Limit};
#[cfg(not(target_arch = "wasm32"))]
use {
    rusqlite::OpenFlags,
    std::{
        fs,
        path::{Path, PathBuf},
        time::Duration,
    },
};

/// The file's path with its folder resolved: NOFOLLOW refuses a symbolic link anywhere in
/// a path, while the file itself must not be one.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn resolve(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| failed("Invalid document path"))?;
    let folder = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Ok(fs::canonicalize(folder).map_err(|e| failed(format!("Cannot resolve the document's folder: {e}")))?.join(name))
}
/// Every connection: no symbolic links, defensive mode, no trusted schema, cell checks, no
/// memory mapping, and values no longer than the largest stored one.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn connect(path: &Path, flags: OpenFlags, busy: Duration) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        resolve(path)?,
        flags | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(sqlite("open"))?;
    conn.busy_timeout(busy).map_err(sqlite("open"))?;
    configure_connection(&conn)?;
    Ok(conn)
}
/// Connection settings shared by native SQLite and the browser VFS.
pub(crate) fn configure_connection(conn: &Connection) -> Result<()> {
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true).map_err(sqlite("open"))?;
    // Closing never checkpoints: a file in WAL mode, which only a newer build writes, is
    // refused, never rewritten.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true).map_err(sqlite("open"))?;
    conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA cell_size_check=ON; PRAGMA mmap_size=0;")
        .map_err(sqlite("open"))?;
    let version = one(conn, "PRAGMA user_version")?;
    if version <= STORAGE_VERSION {
        conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, crate::STORAGE_BYTES as i32).map_err(sqlite("open"))?;
    }
    Ok(())
}
/// The writer's durability: a rollback journal that exists only while a save commits, and
/// a full sync of every commit. Deleted content is zeroed where that costs no extra write
/// (FAST): Apple's SQLite does so by default, the bundled SQLite of the Linux engines and
/// the browser does not, and a shared file should not depend on which one wrote it.
pub(crate) fn configure_writer(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON; PRAGMA secure_delete=FAST;",
    )
    .map_err(sqlite("configure"))
}
/// A writer's connection: read-write, creating a new file when `create`. An existing file
/// is checked before `configure_writer`, so a file this build refuses is never written.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn writer(path: &Path, create: bool) -> Result<Connection> {
    let create = if create { OpenFlags::SQLITE_OPEN_CREATE } else { OpenFlags::empty() };
    connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE | create, Duration::from_secs(2))
}
/// A reader's connection: read-write with `query_only`. A read-only connection beside this
/// process's writer makes the platform SQLite fail the writer's locks, and sometimes its
/// own, with EBADF (SQLITE_IOERR_LOCK). Like any opener, it completes the rollback of a
/// crashed write, which restores the saved state it reads. A file this process can't write
/// opens read-only; no writer can be in this process then.
#[cfg(not(target_arch = "wasm32"))]
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
/// All durable writes pass through this transaction. Future forward migrations run
/// here, never on reads, and validate before the same transaction commits.
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
pub(crate) fn begin_write<'a>(conn: &'a Connection, action: &str) -> Result<WriteTransaction<'a>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite(action))?;
    let version = markers(&tx)?;
    let validate = migrate(&tx, version)?;
    Ok(WriteTransaction { tx, validate })
}
/// Brings a file from `from` to `STORAGE_VERSION` inside the write's transaction; the commit
/// then checks the result in full. Version 1 is the first. A later version adds its arm
/// here, admits `from` in `markers`, and creates new files by replaying the same chain
/// (`storage-1.sql`, then each migration), so a migrated file and a new one share one
/// exact layout. Display reads never call this.
fn migrate(_tx: &Connection, from: i64) -> Result<bool> {
    match from {
        STORAGE_VERSION => Ok(false),
        _ => Err(invalid("Unsupported document storage")),
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn initialize(conn: &Connection) -> Result<WriteTransaction<'_>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sqlite("create"))?;
    Ok(WriteTransaction { tx, validate: true })
}
