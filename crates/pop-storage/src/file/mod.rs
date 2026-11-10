//! A document is one SQLite file: the app its author built, the document's saved state, its
//! attachments and its artwork. This module owns that format: the tables and every statement
//! on them (every write, and the store's reads, in `rows`), the checks every open runs, packing a build into a template (`pack`),
//! creating and copying documents (`copy`), where documents may live (`places`), which
//! templates hosts list (`catalog`), and serving the app's assets (`assets`). `store` saves a document; `registry` holds its writer lock.

mod artwork;
mod assets;
pub mod build;
mod catalog;
mod category;
mod check;
mod connection;
mod copy;
mod open;
mod pack;
mod places;
pub(crate) mod rows;
mod stamp;

pub use artwork::Artwork;
pub(crate) use artwork::{check_artwork, optimize_png, pack_artwork};
pub use assets::{ResourceReader, content_type, valid_asset_path};
pub use catalog::{find_template, list_templates, open_template, template_source};
pub(crate) use copy::{Staged, copy};
pub use copy::{copy_replica, create_document, new_output, publish_output};
pub use pack::{APP_INPUT_BYTES, RenderArtwork, pack, validate_app};
pub(crate) use places::document_destination;
pub use places::{application, template_roots};
pub use stamp::FileStamp;

/// Refuses a document opened for writing where documents may not live.
pub fn document_location(path: &std::path::Path) -> crate::store::Result<()> {
    places::document_location(path)
}

/// Deletes a closed document while holding its writer lease. Templates stay immutable.
pub fn delete_document(path: &std::path::Path) -> crate::store::Result<()> {
    document_location(path)?;
    let lease = crate::registry::Lease::acquire(path)?;
    if kind(path)? != Kind::Document {
        return Err(crate::error::rejected(
            pop_contracts::wire::Code::IsTemplate,
            "A template cannot be deleted as a document",
        ));
    }
    lease.check()?;
    std::fs::remove_file(path).map_err(crate::error::failed)
}

// A product lineage is distinct from the schema version. Old files with the same
// schema must never be accepted as new-lineage documents or rewritten on open.
pub(crate) const APPLICATION_ID: i64 = 0x504F_5000 | pop_contracts::lineage::DOCUMENT_LINEAGE as i64;
/// These tables. A compatibility requirement, not a release number: a build refuses a
/// newer one with `requires_update`.
pub(crate) const STORAGE_VERSION: i64 = 1;
/// `app` is what the author built, written once by `pack` and identical in a template and
/// its documents, so saved state always belongs to its descriptor. `history` holds the saved
/// Loro state in order: its first row a snapshot (a template's is its initial state, and
/// only that row may start history late), the rest updates saved after it. A document
/// starts as a copy of a template and adds its `document` row, then history rows and
/// `attachments`. A `share` row names the relay endpoint a shared document syncs through;
/// the relay knows the document by its UUID. An asset's `size` is its length;
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
pub use connection::existing;
pub use connection::resolve;
pub(crate) use connection::{begin_write, configure_writer};
pub(crate) use connection::{initialize, reader, writer};
pub(crate) use open::checked;
pub(crate) use open::opened;
pub use open::{OpenedApp, Summary};
pub use open::{artwork, descriptor, export_artwork, inspect, kind, open, summary};
