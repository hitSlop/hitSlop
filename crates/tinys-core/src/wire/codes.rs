//! Classified core and owner outcomes shared by the process boundaries.
use serde::{Deserialize, Serialize};

#[cfg(feature = "storage")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum OutcomeCode {
    Rejected,
    OwnerReplaced,
    Closing,
    SaveFailed,
    OwnerInvalidated,
    UnknownOutcome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "engine.generated.ts"))]
pub enum Code {
    TypeMismatch,
    OutOfRange,
    PathNotFound,
    InvalidKey,
    Exists,
    DuplicateId,
    InvalidRequest,
    InvalidId,
    InvalidPath,
    InvalidSchema,
    TooLarge,
    StaleBase,
    InvalidBytes,
    MissingDependencies,
    EngineError,
    InvalidShape,
    RequiresUpdate,
    IsTemplate,
    /// A command refused with a message for the person (`refuse()`), not a fault.
    Refused,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TypeMismatch => "type_mismatch",
            Self::OutOfRange => "out_of_range",
            Self::PathNotFound => "path_not_found",
            Self::InvalidKey => "invalid_key",
            Self::Exists => "exists",
            Self::DuplicateId => "duplicate_id",
            Self::InvalidRequest => "invalid_request",
            Self::InvalidId => "invalid_id",
            Self::InvalidPath => "invalid_path",
            Self::InvalidSchema => "invalid_schema",
            Self::TooLarge => "too_large",
            Self::StaleBase => "stale_base",
            Self::InvalidBytes => "invalid_bytes",
            Self::MissingDependencies => "missing_dependencies",
            Self::EngineError => "engine_error",
            Self::InvalidShape => "invalid_shape",
            Self::RequiresUpdate => "requires_update",
            Self::IsTemplate => "is_template",
            Self::Refused => "refused",
        }
    }
}
impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(feature = "ts")]
impl Code {
    pub(crate) const ALL: &[Self] = &[
        Self::TypeMismatch,
        Self::OutOfRange,
        Self::PathNotFound,
        Self::InvalidKey,
        Self::Exists,
        Self::DuplicateId,
        Self::InvalidRequest,
        Self::InvalidId,
        Self::InvalidPath,
        Self::InvalidSchema,
        Self::TooLarge,
        Self::StaleBase,
        Self::InvalidBytes,
        Self::MissingDependencies,
        Self::EngineError,
        Self::InvalidShape,
        Self::RequiresUpdate,
        Self::IsTemplate,
        Self::Refused,
    ];
}

#[cfg(feature = "ts")]
impl OutcomeCode {
    pub(crate) const ALL: &[Self] = &[
        Self::Rejected,
        Self::OwnerReplaced,
        Self::Closing,
        Self::SaveFailed,
        Self::OwnerInvalidated,
        Self::UnknownOutcome,
    ];
}

impl Code {
    pub fn from_name(name: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(name.into())).ok()
    }
}
#[cfg(feature = "storage")]
impl OutcomeCode {
    pub fn from_name(name: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(name.into())).ok()
    }
    pub fn name(self) -> String {
        serde_json::to_value(self).expect("outcome name").as_str().expect("string code").into()
    }
}
