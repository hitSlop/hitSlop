//! Opaque byte relay. Document acceptance and Loro live exclusively on clients.
pub use hitslop_sharing_protocol::{envelope, frame, resources as resource_types};
#[cfg(any(target_arch = "wasm32", test))]
mod account;
#[cfg(any(target_arch = "wasm32", test))]
mod auth;
#[cfg(target_arch = "wasm32")]
mod room;

#[cfg(any(target_arch = "wasm32", test))]
mod abuse;
#[cfg(target_arch = "wasm32")]
mod metrics;
