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
pub use pop_contracts::hex;
/// Canonical lowercase document UUID, identical across all replicas.
pub fn valid_document_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) { b == b'-' } else { b.is_ascii_digit() || (b'a'..=b'f').contains(&b) }
        })
}

#[cfg(test)]
mod tests;
