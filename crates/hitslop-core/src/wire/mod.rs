//! Rust-owned wire contracts and shared limits.
mod core;
mod limits;
pub use core::*;
pub use limits::*;
#[cfg(feature = "storage")]
pub mod page;
#[cfg(feature = "storage")]
pub(crate) use page::*;
#[cfg(feature = "storage")]
pub mod socket;
#[cfg(feature = "storage")]
pub(crate) use socket::*;
mod codes;
pub use codes::Code;
#[cfg(feature = "storage")]
pub use codes::OutcomeCode;
#[cfg(feature = "storage")]
pub mod build;
#[cfg(feature = "storage")]
pub mod engine;
#[cfg(feature = "storage")]
pub mod native;

/// Optional wire fields may be absent, but explicitly present null is not a value.
pub(crate) fn present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[cfg(feature = "storage")]
pub mod preview;
