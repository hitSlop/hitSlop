//! Existing document acceptance bounds, shared without the document engine.
pub const ASSET_FILE_BYTES: usize = 26214400;
pub const ASSET_COUNT: usize = 256;
pub const ASSET_BYTES: usize = 52428800;
pub const ATTACHMENT_FILE_BYTES: usize = 10485760;
pub const ATTACHMENT_BYTES: usize = 104857600;
pub const ATTACHMENT_COUNT: usize = 256;
/// Host-owned Firebase ID tokens; these never enter a document or authored page.
pub const IDENTITY_TOKEN_BYTES: usize = 16 * 1024;
