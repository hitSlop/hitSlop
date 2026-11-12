//! Rust-owned wire contracts and shared limits.
pub mod core;
pub mod limits;
pub use core::*;
pub use limits::*;
pub mod codes;
pub use codes::{Code, OutcomeCode};
pub mod page;
pub use page::*;
pub mod socket;
pub use socket::*;
pub mod build;
pub mod engine;
pub mod preview;

/// Optional fields may be absent, but explicit null is not a value.
pub fn present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
