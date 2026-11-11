use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    Busy,
    RequiresUpdate,
    AppMismatch,
    AccountChanged,
    SignInRequired,
    Failed,
}
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub issue: Issue,
    pub message: String,
}
pub type Result<T> = std::result::Result<T, Error>;
pub(super) fn failed(error: impl std::fmt::Display) -> Error {
    Error { issue: Issue::Failed, message: error.to_string() }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        failed(error)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        failed(error)
    }
}
impl From<Failure> for Error {
    fn from(error: Failure) -> Self {
        let issue = if error.reason == Some(pop_contracts::wire::Code::RequiresUpdate) {
            Issue::RequiresUpdate
        } else if matches!(error.kind, FailureKind::Busy | FailureKind::Locked | FailureKind::Closing) {
            Issue::Busy
        } else {
            Issue::Failed
        };
        Self { issue, message: error.message }
    }
}
impl From<pop_storage::store::Error> for Error {
    fn from(error: pop_storage::store::Error) -> Self {
        Failure::from(error).into()
    }
}
