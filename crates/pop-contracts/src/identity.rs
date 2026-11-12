//! Identity of the exact embedded app.

/// The exact embedded app row and assets, excluding document state and artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AppDigest([u8; 32]);
impl AppDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonical lowercase document UUID, identical across all replicas.
pub fn valid_document_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) { b == b'-' } else { b.is_ascii_digit() || (b'a'..=b'f').contains(&b) }
        })
}
