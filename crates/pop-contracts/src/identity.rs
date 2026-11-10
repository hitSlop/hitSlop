//! Identity of the exact embedded app revision.

/// The exact embedded app row and assets, excluding document state and artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AppRevisionId([u8; 32]);
impl AppRevisionId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
