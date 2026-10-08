//! Why a storage call failed, shared by the file format (`file`), the writer lock
//! (`registry`) and the store, which hosts see it through (`store::Error`).

/// Why a storage call failed. Every save failure keeps ownership, the live state and all
/// edits.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Another process holds the writer lock.
    #[error("document has a live writer")]
    Locked,
    /// Another connection held the database (for example a backup); retrying can succeed.
    #[error("document database is busy")]
    Busy,
    /// The write would exceed the storage limits; saved state is intact.
    #[error("document is full")]
    Full,
    /// The document file was moved or replaced while open.
    #[error("document moved or replaced")]
    Moved,
    /// The store no longer owns the document.
    #[error("document is closed")]
    Closed,
    /// The request breaks a document rule (a theme value, for example); nothing changed.
    #[error("{0}")]
    Rejected(crate::Error),
    #[error("{0}")]
    Failed(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn failed(message: impl ToString) -> Error {
    Error::Failed(message.to_string())
}
/// A refusal under a core error code; nothing changed.
pub(crate) fn rejected(code: crate::Code, message: impl ToString) -> Error {
    Error::Rejected(crate::err(code, message))
}
pub(crate) fn invalid(message: impl ToString) -> Error {
    rejected(crate::Code::InvalidRequest, message)
}
pub(crate) fn requires_update(message: impl ToString) -> Error {
    rejected(crate::Code::RequiresUpdate, message)
}
/// A SQLite failure as a storage outcome: full, busy, moved, or failed doing `action`.
pub(crate) fn sqlite(action: &str) -> impl FnOnce(rusqlite::Error) -> Error + '_ {
    use rusqlite::ffi::ErrorCode;
    move |e| match e.sqlite_error_code() {
        Some(ErrorCode::DiskFull | ErrorCode::TooBig) => Error::Full,
        // BUSY is a definite failure: nothing was written.
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Error::Busy,
        // Not a SQLite database at all: a file this app refuses, not a storage failure.
        Some(ErrorCode::NotADatabase) => invalid("This is not a hitSlop document"),
        // Apple's SQLite: the file was renamed, or gained and lost a hard link, while open.
        _ if e.sqlite_error().is_some_and(|e| e.extended_code == rusqlite::ffi::SQLITE_IOERR_VNODE) => Error::Moved,
        _ => failed(format!("{action}: {e}")),
    }
}
