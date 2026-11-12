//! Classified core and owner outcomes shared by the process boundaries.
use serde::{Deserialize, Serialize};

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
        static NAMES: std::sync::LazyLock<Vec<String>> =
            std::sync::LazyLock::new(|| Code::ALL.iter().map(crate::enum_name).collect());
        let index = Self::ALL.iter().position(|code| *code == self).expect("all codes listed");
        &NAMES[index]
    }
}
impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Code {
    pub const ALL: &[Self] = &[
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
    pub const ALL: &[Self] = &[
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
impl OutcomeCode {
    pub fn from_name(name: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(name.into())).ok()
    }
    pub fn name(self) -> String {
        crate::enum_name(self)
    }
}
