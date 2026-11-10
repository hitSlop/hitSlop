//! Shared native/WASM document semantics.

pub mod app;
pub mod arguments;
#[cfg(feature = "ts")]
pub mod bindings;
mod check;
pub mod describe;
mod descriptor;
mod document;
mod execute;
mod identity;
mod layout;
mod project;
mod publication;
mod replace;
pub mod shape;
pub mod sync;
mod text;
pub mod theme;
use pop_contracts::wire;

pub use descriptor::validate;
use descriptor::{Node, is_scalar, loro_scalar, unwrap_optional, valid_key};
pub use document::{AppSpec, Applied, Document, HistoryVersion, LAYOUT, Origin, TextEdit};
use execute::{put, resolve};
use identity::application_id;
use loro::{
    Container, ContainerID, ContainerTrait, Index, LoroDoc, LoroMap, LoroMovableList, LoroText, LoroValue,
    ValueOrContainer,
};
use project::project;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use wire::{Anchor, Code, Hunk, Intent, PatchOp, Segment, valid_id};

use pop_contracts::{Error, MAX_SAFE, encode, err, parse};
type Result<T> = std::result::Result<T, Error>;
fn engine(e: impl ToString) -> Error {
    err(Code::EngineError, e)
}
/// A materialized Loro value as JSON.
fn json(value: loro::LoroValue) -> Value {
    serde_json::to_value(value).expect("Loro values encode as JSON")
}
/// `mutex`'s value, also after a thread panicked holding it: no value guarded here is left
/// half-changed by a panic.
pub fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
/// Fills `buffer` from the platform's randomness.
///
/// # Errors
/// Returns the platform error if its random source is unavailable.
pub fn random(buffer: &mut [u8]) -> std::result::Result<(), getrandom::Error> {
    getrandom::getrandom(buffer)
}
/// A random identity encoded as lowercase hexadecimal.
///
/// # Errors
/// Returns the platform error if its random source is unavailable.
pub fn random_hex(bytes: usize) -> std::result::Result<String, getrandom::Error> {
    let mut buffer = vec![0; bytes];
    random(&mut buffer)?;
    Ok(pop_contracts::hex(&buffer))
}
fn safe(n: i64) -> bool {
    (-MAX_SAFE..=MAX_SAFE).contains(&n)
}
/// JSON integers are integral numeric values, including `1.0`, `1e2` and `-0.0`.
/// Canonicalize only after proving the value fits JavaScript's exact integer range.
fn integer(value: &Value) -> Option<i64> {
    let n = value.as_f64()?;
    (n.is_finite() && n.fract() == 0.0 && n.abs() <= MAX_SAFE as f64).then_some(n as i64)
}
