//! The accepted pop file, durable storage and its writer lease.
mod error;
pub mod file;
pub mod images;
pub mod media;
#[cfg(not(target_arch = "wasm32"))]
pub mod registry;
pub mod store;
pub use pop_contracts::wire::{self, *};
pub use pop_core::app;
pub use pop_core::{AppSpec, Document, Error, HistoryVersion};
use serde::Serialize;
type Result<T> = std::result::Result<T, Error>;
fn err(code: Code, message: impl ToString) -> Error {
    Error { code, message: message.to_string(), op_index: None, path: vec![] }
}
fn encode(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("core values encode as JSON")
}
use pop_contracts::hex;
fn random(buffer: &mut [u8]) {
    getrandom::getrandom(buffer).expect("random bytes");
}
#[cfg(not(target_arch = "wasm32"))]
fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0; bytes];
    random(&mut buffer);
    hex(&buffer)
}
fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
