//! Loro-free sharing contracts shared by the owner, native/web clients and relay.
pub mod document;
pub mod failure;
pub mod http;
pub mod library;
pub mod limits;
pub mod resources;
pub mod supervisor;
pub const TRUSTED_ORIGIN: &str = "https://share.dotpop.click";
pub const SOCKET_PROTOCOL: &str = "pop.sync.v2";
pub const SEED_BYTES: usize = 16 << 20;
pub const CONTROL_BYTES: usize = 1 << 20;
#[cfg(test)]
mod tests;
