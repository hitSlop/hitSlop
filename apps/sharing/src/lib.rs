//! Opaque byte relay. Document acceptance and Loro live exclusively on clients.
pub use hitslop_sharing_protocol::{envelope, frame, resources as resource_types};
#[cfg(target_arch = "wasm32")]
mod room;
