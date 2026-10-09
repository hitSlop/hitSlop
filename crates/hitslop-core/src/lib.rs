//! Shared native/WASM document semantics.

#[cfg(feature = "storage")]
pub mod app;
pub mod arguments;
#[cfg(feature = "ts")]
pub mod bindings;
mod check;
#[cfg(feature = "storage")]
pub mod command;
pub mod describe;
mod descriptor;
mod document;
#[cfg(feature = "storage")]
mod error;
mod execute;
#[cfg(feature = "storage")]
pub mod file;
mod identity;
#[cfg(feature = "storage")]
pub mod images;
#[cfg(feature = "storage")]
pub mod media;
#[cfg(feature = "storage")]
pub mod owner;
mod project;
mod publication;
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub mod registry;
mod replace;
pub mod shape;
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub mod socket;
#[cfg(feature = "storage")]
pub mod store;
#[cfg(all(test, feature = "storage"))]
mod testing;
mod text;
pub mod theme;
mod wire;

pub use descriptor::validate;
use descriptor::{Node, holds_collections, is_scalar, loro_scalar, unwrap_optional, valid_key};
#[cfg(feature = "storage")]
use document::version::hex;
use document::version::version_token;
pub use document::{AppSpec, Applied, Document, LAYOUT, Origin, TextEdit};
use execute::{Rows, put, resolve};
use identity::{application_id, stored_id};
use loro::{
    Container, ContainerID, ContainerTrait, ExportMode, Frontiers, ID, Index, LoroDoc, LoroMap, LoroMovableList,
    LoroText, ValueOrContainer, VersionVector,
};
use project::project;
use publication::ListState;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
#[cfg(feature = "storage")]
pub use wire::browser as browser_wire;
#[cfg(feature = "storage")]
pub use wire::build;
#[cfg(feature = "storage")]
pub use wire::engine;
#[cfg(feature = "storage")]
pub use wire::engine::{EngineReply, EngineRequest, EngineSuccess};
use wire::valid_id;
pub use wire::{
    ASSET_BYTES, ASSET_COUNT, ASSET_FILE_BYTES, ATTACHMENT_BYTES, ATTACHMENT_COUNT, ATTACHMENT_FILE_BYTES, Code,
    IMAGE_PIXELS, IMAGE_SIDE, PACKAGE_FORMAT, RUNTIME_ABI, STORAGE_BYTES, STORAGE_ROWS,
};
pub use wire::{Anchor, Batch, Hunk, Intent, OwnerState, PatchOp, Publication, Reading, Segment, Selection, ThemeFile};
#[cfg(feature = "storage")]
pub use wire::{HELPER_PROTOCOL, HostLimits, NATIVE_RESOURCE_POLICY, host_limits};
#[cfg(feature = "storage")]
pub use wire::{OutcomeCode, native, page as page_wire, preview, socket as socket_wire};

/// The largest JSON text the core parses: a page request, or an app's initial values.
const MAX_JSON: usize =
    if wire::APP_TEXT_BYTES > wire::PAGE_PAYLOAD { wire::APP_TEXT_BYTES } else { wire::PAGE_PAYLOAD };

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: Code,
    pub message: String,
    pub op_index: Option<usize>,
    /// Location within a checked value; independent of the batch's operation index.
    pub path: Vec<String>,
}
impl Error {
    fn at(mut self, segment: impl ToString) -> Self {
        self.path.insert(0, segment.to_string());
        self
    }
    /// RFC 6901 pointer; an empty string identifies the checked value itself.
    pub fn pointer(&self) -> String {
        self.path.iter().map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1"))).collect()
    }
}
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
/// `bytes` random bytes in lowercase hex.
#[cfg(feature = "storage")]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    random(&mut buffer);
    hex(&buffer)
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

/// Identifies the core build (a hash of its sources and the lockfile), so a release can
/// prove the app and its helper embed the same core.
pub const BUILD_ID: &str = env!("HITSLOP_CORE_BUILD_ID");
