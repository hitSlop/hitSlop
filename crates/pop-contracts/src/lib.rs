//! Passive contracts shared across pop runtime boundaries.
pub mod evaluator;
pub mod identity;
pub mod lineage;
pub mod resources;
pub use identity::AppRevisionId;

pub mod app;
pub mod capture;
pub mod catalog;
mod error;
pub mod media;
pub mod relay;
pub mod shape;
pub mod widgets;
pub mod wire;
pub use error::{Error, Result};
pub use wire::Code;
/// Largest integer exactly representable by JavaScript.
pub const MAX_SAFE: i64 = 9_007_199_254_740_991;
/// Lowercase hexadecimal, two digits per byte: the text form of digests and identities.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    out
}
/// Constructs a classified boundary error.
pub fn err(code: Code, message: impl ToString) -> Error {
    Error::new(code, message)
}
/// Parses bounded JSON, classifying malformed or oversized input.
///
/// # Errors
/// Returns `TooLarge` above the JSON bound, or `InvalidRequest` for malformed input.
pub fn parse<T: serde::de::DeserializeOwned>(input: &str) -> Result<T> {
    if input.len() > wire::JSON_BYTES {
        return Err(err(Code::TooLarge, "JSON exceeds size limit"));
    }
    serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))
}
/// Encodes a host-built value whose keys and numbers are JSON-compatible.
///
/// # Panics
/// Panics if the value cannot be represented as JSON.
pub fn encode(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("core values encode as JSON")
}

/// FNV-1a fingerprint for deterministic identities and acceptance rules.
pub fn fnv(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}
