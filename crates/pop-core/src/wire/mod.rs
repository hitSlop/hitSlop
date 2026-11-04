//! Shared wire contracts; semantic acceptance replay remains in core.
pub use pop_contracts::wire::*;
#[cfg(feature = "acceptance")]
#[path = "limits.rs"]
mod accepted_limits;
