//! Relay admission and retention policy. Only the Worker enforces these.

/// Sockets admitted to one room.
pub const ROOM_CONNECTIONS: usize = 32;
/// Retained update bytes, excluding the full-history base.
pub const ROOM_RECORD_BYTES: usize = 16 << 20;
/// Retained update rows, excluding the base.
pub const ROOM_RECORDS: usize = 32_768;
/// Retained updates at which a client may compact the room.
pub const COMPACT_AFTER_BYTES: usize = 4 << 20;
/// Retained rows at which a client may compact the room.
pub const COMPACT_AFTER_RECORDS: usize = 8_192;
/// Maximum sockets admitted from one client IP in a room.
pub const ROOM_IP_CONNECTIONS: usize = 8;
/// Maximum live library sockets per account.
pub const ACCOUNT_CONNECTIONS: usize = 16;
