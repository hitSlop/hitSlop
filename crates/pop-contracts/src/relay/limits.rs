//! Bounds the relay enforces and every replica must respect.
use super::frame::MAX_FRAME;

/// Largest full-history base accepted by the relay.
pub const BASE_BYTES: usize = 16 << 20;
/// Target batch size; an indivisible accepted change may use the full socket allowance.
pub const PUSH_BATCH_BYTES: usize = 256 << 10;
/// One socket's incoming push wire bytes per minute.
pub const SOCKET_PUSH_BYTES_PER_MINUTE: usize = 8 << 20;
/// One socket's incoming pushes per minute.
pub const SOCKET_PUSHES_PER_MINUTE: usize = 1_800;
/// Bytes and frames a transport holds for its owner before it stops reading the socket.
pub const QUEUED_BYTES: usize = 2 * MAX_FRAME;
pub const QUEUED_FRAMES: usize = 256;
