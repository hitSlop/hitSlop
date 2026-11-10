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
mod wire;

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
pub use wire::browser as browser_wire;
pub use wire::build;
pub use wire::engine;
pub use wire::engine::{EngineReply, EngineRequest, EngineSuccess};
use wire::valid_id;
pub use wire::{
    ASSET_BYTES, ASSET_COUNT, ASSET_FILE_BYTES, ATTACHMENT_BYTES, ATTACHMENT_COUNT, ATTACHMENT_FILE_BYTES,
    ATTACHMENT_NAME_BYTES, Code, IMAGE_PIXELS, IMAGE_SIDE, PACKAGE_FORMAT, RUNTIME_ABI, STORAGE_BYTES, STORAGE_ROWS,
};
pub use wire::{Anchor, Batch, Hunk, Intent, OwnerState, PatchOp, Publication, Reading, Segment, Selection, ThemeFile};
pub use wire::{ENGINE_PROTOCOL, HostLimits, NATIVE_RESOURCE_POLICY, host_limits};
pub use wire::{OutcomeCode, page as page_wire, preview, socket as socket_wire};

/// The largest JSON text the core parses: a page request, or an app's initial values.
use wire::JSON_BYTES as MAX_JSON;

pub use pop_contracts::Error;
type Result<T> = std::result::Result<T, Error>;
fn err(code: Code, message: impl ToString) -> Error {
    Error { code, message: message.to_string(), op_index: None, path: vec![] }
}
fn engine(e: impl ToString) -> Error {
    err(Code::EngineError, e)
}
fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    if s.len() > MAX_JSON {
        return Err(err(Code::TooLarge, "JSON exceeds size limit"));
    }
    serde_json::from_str(s).map_err(|e| err(Code::InvalidRequest, e))
}
/// JSON text of a value the core built: string keys and finite or null numbers only.
fn encode(v: &impl Serialize) -> String {
    serde_json::to_string(v).expect("core values encode as JSON")
}
/// A materialized Loro value as JSON.
fn json(value: loro::LoroValue) -> Value {
    serde_json::to_value(value).expect("Loro values encode as JSON")
}
/// `mutex`'s value, also after a thread panicked holding it: no value guarded here is left
/// half-changed by a panic.
pub(crate) fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
/// Fills `buffer` from the platform's randomness, without which no identity can be minted.
fn random(buffer: &mut [u8]) {
    getrandom::getrandom(buffer).expect("random bytes");
}
const MAX_SAFE: i64 = 9_007_199_254_740_991;
fn safe(n: i64) -> bool {
    (-MAX_SAFE..=MAX_SAFE).contains(&n)
}
/// JSON integers are integral numeric values, including `1.0`, `1e2` and `-0.0`.
/// Canonicalize only after proving the value fits JavaScript's exact integer range.
fn integer(value: &Value) -> Option<i64> {
    let n = value.as_f64()?;
    (n.is_finite() && n.fract() == 0.0 && n.abs() <= MAX_SAFE as f64).then_some(n as i64)
}
