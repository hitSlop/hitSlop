//! Failures classified for owner callers and native hosts.
use crate::{Code, store};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Rejected,
    Replaced,
    Closing,
    Closed,
    ReadOnly,
    Invalidated,
    Locked,
    Busy,
    Full,
    Moved,
    SaveFailed,
    Failed,
}
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
    pub reason: Option<Code>,
    pub op_index: Option<u32>,
}
impl Failure {
    pub(crate) fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), reason: None, op_index: None }
    }
    /// A refusal under a core error code: nothing changed.
    pub(crate) fn rejected(code: Code, message: impl Into<String>) -> Self {
        Self { reason: Some(code), ..Self::new(FailureKind::Rejected, message) }
    }
}
impl From<crate::Error> for Failure {
    fn from(e: crate::Error) -> Self {
        Self {
            kind: FailureKind::Rejected,
            reason: Some(e.code),
            message: e.message,
            op_index: e.op_index.map(|n| n as u32),
        }
    }
}
impl From<store::Error> for Failure {
    fn from(e: store::Error) -> Self {
        let kind = match e {
            store::Error::Rejected(e) => return e.into(),
            store::Error::Locked => FailureKind::Locked,
            store::Error::Busy => FailureKind::Busy,
            store::Error::Full => FailureKind::Full,
            store::Error::Moved => FailureKind::Moved,
            store::Error::Closed => FailureKind::Closed,
            store::Error::Failed(_) => FailureKind::Failed,
        };
        Self::new(kind, e.to_string())
    }
}
