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
pub const AUTHORED_DOMAIN: &str = "dotslop.com";
pub const SOCKET_PROTOCOL: &str = "pop.sync.v2";
pub const SEED_BYTES: usize = 16 << 20;
pub const QUEUED_BYTES: usize = 2 * frame::MAX_FRAME;
pub const QUEUED_FRAMES: usize = 256;
pub const CONTROL_BYTES: usize = 1 << 20;
pub const RESOURCE_TTL_MS: u64 = 600_000;
pub const RESOURCE_RENEW_MS: u64 = RESOURCE_TTL_MS / 2;
pub use pop_contracts::hex;
/// Canonical lowercase document UUID.
pub use pop_contracts::wire::browser::valid_copy;
pub fn is_authored_origin(origin: &str) -> bool {
    origin.strip_prefix("https://").and_then(|s| s.strip_suffix(&format!(".{AUTHORED_DOMAIN}"))).is_some_and(valid_copy)
}
pub fn host_policy() -> String {
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; connect-src 'self' wss://share.dotpop.click https://*.dotslop.com https://identitytoolkit.googleapis.com https://securetoken.googleapis.com; worker-src 'self'; img-src 'self' blob:; frame-src https://*.{AUTHORED_DOMAIN}; base-uri 'none'; frame-ancestors 'none'"
    )
}
pub fn page_policy() -> String {
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' https: blob:; media-src 'self' https: blob:; frame-src https:; style-src 'self' 'unsafe-inline'; img-src 'self' data: https: blob:; font-src 'self' data:; worker-src 'self'; base-uri 'none'; frame-ancestors {TRUSTED_ORIGIN}"
    )
}

/// Canonical cloud document UUID, identical across all replicas.
pub fn valid_room(room: &str) -> bool {
    valid_copy(room)
}

#[cfg(test)]
mod tests;

pub use limits::{
    BASE_BYTES, COMPACT_AFTER_BYTES, COMPACT_AFTER_RECORDS, ROOM_RECORD_BYTES, ROOM_RECORDS,
    SOCKET_PUSH_BYTES_PER_MINUTE, SOCKET_PUSHES_PER_MINUTE,
};
