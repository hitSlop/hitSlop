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
