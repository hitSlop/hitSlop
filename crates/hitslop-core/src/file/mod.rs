//! A document is one SQLite file: the app its author built, the document's saved state, its
//! attachments and its artwork. This module owns that format: the tables and every statement
//! on them (every write, and the store's reads, in `rows`), the checks every open runs, packing a build into a template (`pack`),
//! creating and copying documents (`copy`), where documents may live (`places`), which
//! templates hosts list (`catalog`), and serving the app's assets (`assets`). `store` saves a document; `registry` holds its writer lock.

mod artwork;
mod assets;
#[cfg(not(target_arch = "wasm32"))]
pub mod build;
#[cfg(not(target_arch = "wasm32"))]
mod catalog;
mod check;
mod connection;
#[cfg(not(target_arch = "wasm32"))]
mod copy;
mod open;
#[cfg(not(target_arch = "wasm32"))]
mod pack;
#[cfg(not(target_arch = "wasm32"))]
mod places;
pub(crate) mod rows;

pub use artwork::Artwork;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use artwork::{check_artwork, optimize_png};
#[cfg(target_arch = "wasm32")]
pub(crate) use assets::ResourceCache;
pub use assets::{ResourceInfo, ResourceReader, ResourceRoute, content_type, valid_asset_path};
#[cfg(not(target_arch = "wasm32"))]
pub use catalog::{Catalog, Folder, Template, find_template, list_templates, open_template, template_source};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use copy::copy;
#[cfg(not(target_arch = "wasm32"))]
pub use copy::create_document;
#[cfg(not(target_arch = "wasm32"))]
pub use pack::{APP_INPUT_BYTES, pack, validate_app};
#[cfg(not(target_arch = "wasm32"))]
pub use places::{TemplateSource, template_roots};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use places::{document_destination, document_location};

pub(crate) const APPLICATION_ID: i64 = 0x4853_4C50; // HSLP
/// These tables. A compatibility requirement, not a release number: a build refuses a
/// newer one with `requires_update`.
pub(crate) const STORAGE_VERSION: i64 = 1;
/// `app` is what the author built, written once by `pack` and identical in a template and
/// its documents, so saved state always belongs to its descriptor. `history` holds the saved
/// Loro state in order: its first row a snapshot (a template's is its initial state, and
/// only that row may start history late), the rest updates saved after it. A document
/// starts as a copy of a template and adds its `document` row, then history rows and
/// `attachments`. A `share` row names the room a shared document syncs through. An asset's `size` is its length;
/// `encoding` is how `bytes` holds it (`encode`). The tables are STRICT: SQLite refuses a
/// mistyped write and the quick check finds a mistyped row, but a file is untrusted bytes,
/// so every open still checks what it reads.
pub(crate) const SCHEMA: &str = include_str!("storage-1.sql");
const _: () = assert!(STORAGE_VERSION == 1, "add the old reader and a transactional migration before raising storage");

/// A template holds only the app; a document also holds its saved state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Template,
    Document,
}

pub(crate) use check::{attachments_fit, check};
pub(crate) use connection::{begin_write, configure_writer};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use connection::{initialize, reader, resolve, writer};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use open::checked;
pub(crate) use open::opened;
pub use open::{OpenedApp, Summary};
#[cfg(not(target_arch = "wasm32"))]
pub use open::{artwork, descriptor, export_artwork, inspect, kind, open, summary};

#[cfg(target_arch = "wasm32")]
pub(crate) use connection::configure_connection;
