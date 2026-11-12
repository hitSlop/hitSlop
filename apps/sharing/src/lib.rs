//! Opaque byte relay. Document acceptance and Loro live exclusively on clients.
#[cfg(any(target_arch = "wasm32", test))]
mod account;
#[cfg(any(target_arch = "wasm32", test))]
mod auth;
#[cfg(target_arch = "wasm32")]
mod input;
pub mod limits;
#[cfg(target_arch = "wasm32")]
mod room;
pub mod routes;

#[cfg(target_arch = "wasm32")]
mod metrics;

#[cfg(target_arch = "wasm32")]
mod access;
#[cfg(target_arch = "wasm32")]
mod blobs;
#[cfg(target_arch = "wasm32")]
mod bounded;
#[cfg(target_arch = "wasm32")]
mod compaction;
#[cfg(target_arch = "wasm32")]
mod document_state;
#[cfg(target_arch = "wasm32")]
mod lifecycle;
pub mod policy;
#[cfg(target_arch = "wasm32")]
pub(crate) mod publication;
pub mod refusal;
#[cfg(target_arch = "wasm32")]
mod seed;
#[cfg(target_arch = "wasm32")]
mod sync;
