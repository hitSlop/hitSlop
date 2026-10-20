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
pub mod envelope;
pub mod frame;
pub mod http;
pub mod limits;
pub mod policy;
pub mod resources;
pub const TRUSTED_ORIGIN: &str = "https://share.hitslop.com";
pub const AUTHORED_DOMAIN: &str = "dotslop.com";
pub const SOCKET_PROTOCOL: &str = "hitslop.sync.v1";
pub const SEED_BYTES: usize = 16 << 20;
pub const QUEUED_BYTES: usize = 2 * frame::MAX_FRAME;
pub const QUEUED_FRAMES: usize = 256;
pub const CONTROL_BYTES: usize = 1 << 20;
pub const RESOURCE_TTL_MS: u64 = 600_000;
pub const RESOURCE_RENEW_MS: u64 = RESOURCE_TTL_MS / 2;
pub fn valid_copy(copy: &str) -> bool {
    copy.len() == 36
        && copy.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) { b == b'-' } else { b.is_ascii_digit() || (b'a'..=b'f').contains(&b) }
        })
}
pub fn authored_origin(copy: &str) -> Option<String> {
    valid_copy(copy).then(|| format!("https://{copy}.{AUTHORED_DOMAIN}"))
}
pub fn is_authored_origin(origin: &str) -> bool {
    origin.strip_prefix("https://").and_then(|s| s.strip_suffix(&format!(".{AUTHORED_DOMAIN}"))).is_some_and(valid_copy)
}
pub fn host_policy() -> String {
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; connect-src 'self' wss://share.hitslop.com https://*.dotslop.com; worker-src 'self'; img-src 'self' blob:; frame-src https://*.{AUTHORED_DOMAIN}; base-uri 'none'; frame-ancestors 'none'"
    )
}
pub fn page_policy() -> String {
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' https: blob:; media-src 'self' https: blob:; frame-src https:; style-src 'self' 'unsafe-inline'; img-src 'self' data: https: blob:; font-src 'self' data:; worker-src 'self'; base-uri 'none'; frame-ancestors {TRUSTED_ORIGIN}"
    )
}

/// Canonical random room identifier, independent of a document UUID.
pub fn valid_room(room: &str) -> bool {
    room.len() == 32 && room.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
