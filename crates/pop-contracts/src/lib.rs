//! Passive contracts shared across pop runtime boundaries.
pub mod evaluator;
pub mod identity;
pub use identity::{AppRevisionId, StateContractId};

pub mod app;
pub mod capture;
pub mod catalog;
mod error;
pub mod media;
pub mod shape;
pub mod widgets;
pub mod wire;
pub use error::{Error, Result};
pub use wire::Code;
const MAX_SAFE: i64 = 9_007_199_254_740_991;
fn err(code: Code, message: impl ToString) -> Error {
    Error { code, message: message.to_string(), op_index: None, path: vec![] }
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
