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
const MAX_SAFE: i64 = 9_007_199_254_740_991;
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
fn err(code: Code, message: impl ToString) -> Error {
    Error::new(code, message)
}
fn parse<T: serde::de::DeserializeOwned>(input: &str) -> Result<T> {
    if input.len() > wire::JSON_BYTES {
        return Err(err(Code::TooLarge, "JSON exceeds size limit"));
    }
    serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))
}
fn encode(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("core values encode as JSON")
}
