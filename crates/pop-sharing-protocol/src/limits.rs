//! Existing document acceptance bounds, shared without the document engine.
pub use pop_contracts::wire::{
    ASSET_BYTES, ASSET_COUNT, ASSET_FILE_BYTES, ATTACHMENT_BYTES, ATTACHMENT_COUNT, ATTACHMENT_FILE_BYTES,
};
/// Host-owned Firebase ID tokens; these never enter a document or authored page.
pub const IDENTITY_TOKEN_BYTES: usize = 16 * 1024;
/// Relay admission policy, separate from document acceptance.
pub const ROOM_CONNECTIONS: usize = 32;

/// Retained update bytes, excluding the full-history base.
pub const ROOM_RECORD_BYTES: usize = 16 << 20;
/// Retained update rows, excluding the base.
pub const ROOM_RECORDS: usize = 32_768;
/// Largest full-history base accepted by the relay.
pub const BASE_BYTES: usize = 16 << 20;
/// Retained updates at which a client may compact the room.
pub const COMPACT_AFTER_BYTES: usize = 4 << 20;
/// Retained rows at which a client may compact the room.
pub const COMPACT_AFTER_RECORDS: usize = 8_192;
/// Target batch size; an indivisible accepted change may use the full socket allowance.
pub const PUSH_BATCH_BYTES: usize = 256 << 10;
/// One socket's incoming push wire bytes per minute.
pub const SOCKET_PUSH_BYTES_PER_MINUTE: usize = 8 << 20;
/// One socket's incoming pushes per minute.
pub const SOCKET_PUSHES_PER_MINUTE: usize = 1_800;
/// Maximum sockets admitted from one client IP in a room.
pub const ROOM_IP_CONNECTIONS: usize = 8;
/// Registrations per account-listing page; a full page is about 70 KB, far below
/// `CONTROL_BYTES`. Accounts have no document cap, so listings page instead.
pub const ACCOUNT_DOCUMENTS_PAGE: usize = 1_000;
// Even a base with the maximum peer headers fitting in CONTROL_BYTES stays in one frame.
const _: () = assert!(BASE_BYTES + crate::CONTROL_BYTES + 16 < crate::frame::MAX_FRAME);

/// Client timing, shared by the native supervisor and the browser worker.
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
