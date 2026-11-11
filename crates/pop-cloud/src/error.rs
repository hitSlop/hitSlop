//! Failures retain actionable kinds and causes across native sharing boundaries.
use pop_contracts::relay::PauseKind;
use pop_runtime::owner::{Failure, FailureKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    Busy,
    RequiresUpdate,
    AppMismatch,
    AccountChanged,
    SignInRequired,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Pause { reason: PauseKind },
    UnknownOutcome,
    Busy,
    RequiresUpdate,
    AppMismatch,
    AccountChanged,
    SignInRequired,
    Failed,
}
#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("{reason}")]
    Sharing {
        kind: PauseKind,
        reason: String,
        unknown: bool,
        status: Option<u16>,
        code: Option<String>,
        #[source]
        source: Option<std::sync::Arc<dyn std::error::Error + Send + Sync>>,
    },
    #[error("{message}")]
    Library { issue: Issue, message: String },
}
pub(crate) type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub(crate) fn new(kind: PauseKind, reason: String) -> Self {
        Self::Sharing { kind, reason, unknown: false, status: None, code: None, source: None }
    }
    pub(crate) fn caused<E: std::error::Error + Send + Sync + 'static>(
        kind: PauseKind,
        reason: String,
        source: E,
    ) -> Self {
        Self::Sharing {
            kind,
            reason,
            unknown: false,
            status: None,
            code: None,
            source: Some(std::sync::Arc::new(source)),
        }
    }
    pub fn pause_kind(&self) -> PauseKind {
        match self {
            Self::Sharing { kind, .. } => *kind,
            Self::Library { issue, .. } => match issue {
                Issue::Busy => PauseKind::Busy,
                Issue::RequiresUpdate => PauseKind::NeedsUpdate,
                Issue::SignInRequired => PauseKind::SignIn,
                _ => PauseKind::Failed,
            },
        }
    }
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Sharing { unknown: true, .. } => ErrorKind::UnknownOutcome,
            Self::Sharing { kind: PauseKind::SignIn, .. } => ErrorKind::SignInRequired,
            Self::Sharing { kind, .. } => ErrorKind::Pause { reason: *kind },
            Self::Library { issue, .. } => match issue {
                Issue::Busy => ErrorKind::Busy,
                Issue::RequiresUpdate => ErrorKind::RequiresUpdate,
                Issue::AppMismatch => ErrorKind::AppMismatch,
                Issue::AccountChanged => ErrorKind::AccountChanged,
                Issue::SignInRequired => ErrorKind::SignInRequired,
                Issue::Failed => ErrorKind::Failed,
            },
        }
    }
    pub fn unknown_outcome(&self) -> bool {
        matches!(self, Self::Sharing { unknown: true, .. })
    }
    pub fn http_status(&self) -> Option<u16> {
        match self {
            Self::Sharing { status, .. } => *status,
            _ => None,
        }
    }
    pub fn refusal_code(&self) -> Option<&str> {
        match self {
            Self::Sharing { code, .. } => code.as_deref(),
            _ => None,
        }
    }
    pub(crate) fn owner_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Sharing { source: Some(source), .. } => source.downcast_ref::<Failure>().map(|e| e.kind),
            _ => None,
        }
    }
}
impl From<&str> for Error {
    fn from(reason: &str) -> Self {
        Self::new(PauseKind::Failed, reason.into())
    }
}
impl From<String> for Error {
    fn from(reason: String) -> Self {
        Self::new(PauseKind::Failed, reason)
    }
}
impl From<pop_core::sync::Pause> for Error {
    fn from(p: pop_core::sync::Pause) -> Self {
        Self::new(p.kind, p.reason)
    }
}
impl From<Failure> for Error {
    fn from(e: Failure) -> Self {
        crate::transport::owner_failure(e, crate::transport::OwnerEffect::Other)
    }
}
impl From<pop_storage::store::Error> for Error {
    fn from(e: pop_storage::store::Error) -> Self {
        let mut error = Self::from(Failure::from(e.clone()));
        if let Self::Sharing { source, .. } = &mut error {
            *source = Some(std::sync::Arc::new(e));
        }
        error
    }
}
impl From<pop_contracts::Error> for Error {
    fn from(e: pop_contracts::Error) -> Self {
        let mut error = Self::from(Failure::from(e.clone()));
        if let Self::Sharing { source, .. } = &mut error {
            *source = Some(std::sync::Arc::new(e));
        }
        error
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::caused(PauseKind::Failed, e.to_string(), e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::caused(PauseKind::Failed, e.to_string(), e)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::caused(PauseKind::Failed, e.to_string(), e)
    }
}
