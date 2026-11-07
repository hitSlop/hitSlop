//! Saved-source capture, independent of the document owner and its edit queue.
#[cfg(not(target_arch = "wasm32"))]
pub mod capture;
#[cfg(not(target_arch = "wasm32"))]
pub mod widgets;
#[cfg(not(target_arch = "wasm32"))]
use pop_contracts::wire;
#[cfg(not(target_arch = "wasm32"))]
pub use pop_core::{Code, Document, Error};
#[cfg(not(target_arch = "wasm32"))]
pub use pop_storage::{file, images, media, store};

#[cfg(not(target_arch = "wasm32"))]
fn err(code: Code, message: impl ToString) -> Error {
    Error { code, message: message.to_string(), op_index: None, path: vec![] }
}
#[cfg(not(target_arch = "wasm32"))]
fn encode(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("capture inputs encode as JSON")
}
#[cfg(not(target_arch = "wasm32"))]
mod error {
    pub(super) fn failed(message: impl ToString) -> pop_storage::store::Error {
        pop_storage::store::Error::Failed(message.to_string())
    }
    pub(super) fn rejected(code: pop_core::Code, message: impl ToString) -> pop_storage::store::Error {
        pop_storage::store::Error::Rejected(super::err(code, message))
    }
    pub(super) fn invalid(message: impl ToString) -> pop_storage::store::Error {
        rejected(pop_core::Code::InvalidRequest, message)
    }
}
