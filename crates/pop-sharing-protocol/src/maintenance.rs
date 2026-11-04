//! Compaction decisions shared by transports; clocks, randomness and I/O stay with callers.
use serde::{Deserialize, Serialize};

/// Whether a room-full connection has already retried after compaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum FullState {
    Available,
    Waiting,
    Retried,
    FilesWaiting,
    FilesRetried,
}

/// The observation made by a transport at one compaction boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "step", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum Step {
    Probe {
        compact: bool,
    },
    Recheck {
        compact: bool,
    },
    BaseUnavailable,
    /// Includes a conflict: another writer completed compaction first.
    Compacted,
}

/// The next operation, without performing it or sampling a clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum MaintenanceAction {
    Idle,
    Jitter { milliseconds: u64 },
    ReadBase,
    RetryBase { milliseconds: u64 },
    Reconnect,
    Outgrown,
}

/// Decide the next operation; `random` is a uniformly sampled transport-owned value.
pub fn decide(full: FullState, step: Step, random: u16) -> MaintenanceAction {
    use MaintenanceAction::*;
    match step {
        Step::Probe { .. } if full == FullState::FilesRetried => Outgrown,
        Step::Probe { compact: false } if full == FullState::Retried => Outgrown,
        Step::Probe { compact } if compact || full != FullState::Available => {
            Jitter { milliseconds: u64::from(random) * crate::limits::COMPACT_JITTER_MS / u64::from(u16::MAX) }
        }
        Step::Probe { .. } => Idle,
        Step::Recheck { .. } if full == FullState::FilesWaiting => ReadBase,
        Step::Recheck { compact: true } => ReadBase,
        Step::BaseUnavailable => RetryBase { milliseconds: crate::limits::COMPACT_RETRY_MS },
        Step::Recheck { compact: false } | Step::Compacted if full != FullState::Available => Reconnect,
        Step::Recheck { .. } | Step::Compacted => Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_rooms_retry_once_then_pause_and_unsettled_bases_wait() {
        use FullState::*;
        use MaintenanceAction::*;
        for (full, step, expected) in [
            (Available, Step::Probe { compact: false }, Idle),
            (FilesWaiting, Step::Probe { compact: false }, Jitter { milliseconds: 0 }),
            (FilesWaiting, Step::Recheck { compact: false }, ReadBase),
            (FilesRetried, Step::Probe { compact: true }, Outgrown),
            (FilesRetried, Step::Probe { compact: false }, Outgrown),
            (FilesWaiting, Step::Compacted, Reconnect),
            (Waiting, Step::Probe { compact: false }, Jitter { milliseconds: 0 }),
            (Retried, Step::Probe { compact: false }, Outgrown),
            (Retried, Step::Probe { compact: true }, Jitter { milliseconds: 0 }),
            (Waiting, Step::Recheck { compact: false }, Reconnect),
            (Available, Step::Recheck { compact: false }, Idle),
            (Waiting, Step::Recheck { compact: true }, ReadBase),
            (Retried, Step::BaseUnavailable, RetryBase { milliseconds: 15_000 }),
            (Waiting, Step::Compacted, Reconnect),
            (Available, Step::Compacted, Idle),
        ] {
            assert_eq!(decide(full, step, 0), expected, "{full:?} {step:?}");
        }
        assert_eq!(decide(Available, Step::Probe { compact: true }, u16::MAX), Jitter { milliseconds: 30_000 },);
    }
}
