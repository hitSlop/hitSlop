//! Cloud protocol bounds and client timing.
/// Host-owned Firebase ID tokens; these never enter a document or authored page.
pub const IDENTITY_TOKEN_BYTES: usize = 16 * 1024;
/// Secondary bound on entries in a live-library snapshot frame.
pub const ACCOUNT_SNAPSHOT_PAGE: usize = 256;
/// Maximum UTF-8 bytes in a cloud library title.
pub const TITLE_BYTES: usize = 1024;
/// Maximum UTF-8 bytes in a cloud library template identifier.
pub const TEMPLATE_ID_BYTES: usize = 256;
// Even a base with the maximum peer headers fitting in CONTROL_BYTES stays in one frame.
const _: () = assert!(
    pop_contracts::relay::limits::BASE_BYTES + crate::CONTROL_BYTES + 16 < pop_contracts::relay::frame::MAX_FRAME
);

/// Client timing, used by native supervisors.
/// A client rereads the room this often to learn whether it should compact.
pub const ROOM_RECHECK_MS: u64 = 600_000;
/// A compacting client waits up to this long first, so rivals usually find it done.
pub const COMPACT_JITTER_MS: u64 = 30_000;
/// A client with no eligible base retries after this long.
pub const COMPACT_RETRY_MS: u64 = 15_000;
/// Idle sockets send a ping this often.
pub const PING_MS: u64 = 20_000;
/// Reconnect delays start here and double up to `BACKOFF_MAX_MS`.
pub const BACKOFF_MIN_MS: u64 = 250;
pub const BACKOFF_MAX_MS: u64 = 8_000;
