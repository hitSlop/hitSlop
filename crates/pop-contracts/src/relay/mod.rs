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
