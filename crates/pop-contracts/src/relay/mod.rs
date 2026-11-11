//! The relay wire shared by every replica and the relay: the frame codec, the limits both
//! sides enforce, and the phase a replica reports. Loro-free; the relay never reads updates.
use serde::{Deserialize, Serialize};

pub mod envelope;
pub mod frame;
pub mod limits;

/// Where a replica stands with its room, apart from the words shown for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum SyncPhase {
    #[default]
    Offline,
    Syncing,
    Synced,
    /// Stopped until a person acts; the reason travels with the pause.
    Paused,
    /// The room no longer exists; the local copy is all that remains.
    Ended,
}

#[cfg(test)]
mod tests;

/// Stable recovery choices shared by every transport and UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "sharing.generated.ts"))]
pub enum PauseKind {
    RoomFull,
    /// Folding cannot make the room fit, or one retained change cannot be relayed.
    /// Local edits keep saving; the person can save an unshared copy.
    Outgrown,
    NeedsUpdate,
    Refused,
    MissingHistory,
    FilesFull,
    Ended,
    Conflict,
    SignIn,
    Failed,
    Busy,
}

impl PauseKind {
    /// Whether synchronization waits for a person rather than retrying.
    pub fn pauses(self) -> bool {
        matches!(
            self,
            Self::RoomFull
                | Self::Outgrown
                | Self::NeedsUpdate
                | Self::Refused
                | Self::MissingHistory
                | Self::FilesFull
                | Self::Ended
        )
    }

    /// What to tell a person when the relay gave no message.
    pub fn message(self, status: u16) -> String {
        match self {
            Self::Ended => "Sharing ended".into(),
            Self::NeedsUpdate => "Update pop to keep sharing".into(),
            Self::Outgrown => "This document has outgrown sharing".into(),
            Self::FilesFull => "Attachments are full".into(),
            Self::SignIn => "Sign in to your sharing account first".into(),
            Self::Conflict => "Sharing was busy. Try again.".into(),
            Self::RoomFull => "Making room…".into(),
            Self::MissingHistory => "The shared document is missing history; this copy keeps its own".into(),
            Self::Busy => "Sharing is busy".into(),
            Self::Refused | Self::Failed => format!("Sharing service refused this request ({status})"),
        }
    }
}
