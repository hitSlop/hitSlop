//! Existing document acceptance bounds, shared without the document engine.
pub const ASSET_FILE_BYTES: usize = 26214400;
pub const ASSET_COUNT: usize = 256;
pub const ASSET_BYTES: usize = 52428800;
pub const ATTACHMENT_FILE_BYTES: usize = 10485760;
pub const ATTACHMENT_BYTES: usize = 104857600;
pub const ATTACHMENT_COUNT: usize = 256;
/// Host-owned Firebase ID tokens; these never enter a document or authored page.
pub const IDENTITY_TOKEN_BYTES: usize = 16 * 1024;
/// Account and relay admission policy, separate from document acceptance.
pub const ACCOUNT_ROOMS: usize = 10;
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
/// One socket's incoming push wire bytes per minute.
pub const SOCKET_PUSH_BYTES_PER_MINUTE: usize = 8 << 20;
/// One socket's incoming pushes per minute.
pub const SOCKET_PUSHES_PER_MINUTE: usize = 1_800;
/// Live rooms expire after this many days without activity or sockets.
pub const ROOM_IDLE_DAYS: u64 = 90;
/// Identifier-free ended tombstones are retained for this many days.
pub const ENDED_TOMBSTONE_DAYS: u64 = 30;
/// Covered records remain in private archives for this many days.
pub const COMPACTED_ARCHIVE_DAYS: u64 = 14;
/// Maximum sockets admitted from one client IP in a room.
pub const ROOM_IP_CONNECTIONS: usize = 8;
// Even a base with the maximum peer headers fitting in CONTROL_BYTES stays in one frame.
const _: () = assert!(BASE_BYTES + crate::CONTROL_BYTES + 16 < crate::frame::MAX_FRAME);
