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

/// Other accounts admitted to a document; rejoining does not consume a second place.
pub const DOCUMENT_MEMBERS: i64 = 64;
/// Invitation exchange request budget.
pub const JOIN_BODY_BYTES: usize = 4096;
/// Metadata update request budget, including JSON escaping.
pub const METADATA_BODY_BYTES: usize = 8192;
/// Bound for one cross-object call; timeout does not establish whether it committed.
pub const OBJECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// Bound for account library preparation across multiple object calls.
pub const LIBRARY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
/// Firebase key identifiers and subjects are bounded before they address storage.
pub const IDENTITY_FIELD_BYTES: usize = 128;
/// Clock skew tolerated for Firebase token timestamps.
pub const IDENTITY_LEEWAY_SECONDS: u64 = 60;
/// Cache lifetime when Google's certificate response omits max-age.
pub const CERTIFICATE_CACHE_SECONDS: u64 = 3600;

/// Catch-up wire bytes per alarm across all sockets; one indivisible record may exceed it.
pub const CATCHUP_BYTES: usize = 1 << 20;
/// Bounds cursor scans even when every record is already known or empty.
pub const CATCHUP_ROWS: usize = 512;
/// Yield between catch-up slices because Workers expose no socket backpressure signal.
pub const CATCHUP_DELAY_MS: u64 = 25;
