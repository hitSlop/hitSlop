//! Loro-free sharing contracts shared by the owner, native/web clients and relay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Code {
    InvalidBytes,
    TooLarge,
}
#[derive(Clone, Debug)]
pub struct Error {
    pub code: Code,
    pub message: String,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub fn err(code: Code, message: impl Into<String>) -> Error {
    Error { code, message: message.into() }
}
pub mod document;
pub mod envelope;
pub mod failure;
pub mod frame;
pub mod http;
pub mod library;
pub mod limits;
pub mod resources;
pub mod routes;
pub const TRUSTED_ORIGIN: &str = "https://share.dotpop.click";
pub const SOCKET_PROTOCOL: &str = "pop.sync.v2";
pub const SEED_BYTES: usize = 16 << 20;
pub const QUEUED_BYTES: usize = 2 * frame::MAX_FRAME;
pub const QUEUED_FRAMES: usize = 256;
pub const CONTROL_BYTES: usize = 1 << 20;
pub use pop_contracts::hex;
/// Canonical lowercase document UUID, identical across all replicas.
pub fn valid_document_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) { b == b'-' } else { b.is_ascii_digit() || (b'a'..=b'f').contains(&b) }
        })
}

#[cfg(test)]
mod tests;

pub use limits::{
    BASE_BYTES, COMPACT_AFTER_BYTES, COMPACT_AFTER_RECORDS, ROOM_RECORD_BYTES, ROOM_RECORDS,
    SOCKET_PUSH_BYTES_PER_MINUTE, SOCKET_PUSHES_PER_MINUTE,
};
