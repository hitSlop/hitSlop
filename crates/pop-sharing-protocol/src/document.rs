//! Persistent document discovery and access. Discovery never grants permission.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum DocumentRole {
    Owner,
    Editor,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DocumentEntry {
    pub document_id: String,
    pub title: String,
    pub template_id: String,
    pub modified_at: u64,
    pub role: DocumentRole,
    pub catalog_revision: u64,
    pub deleted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AccessState {
    pub document: String,
    pub sharing: bool,
    pub generation: u64,
    pub capability: Option<String>,
    pub role: DocumentRole,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct JoinDocument {
    pub capability: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UpdateDocument {
    pub title: String,
    pub template_id: String,
}

/// Registry hints for repairing a missing personal catalog reference. Authorization
/// is always checked by the document before any bytes are returned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RegisteredDocument {
    pub document: String,
    pub role: DocumentRole,
}
/// One page of the account's registrations in document order. An account has no
/// document cap, so the list is read with `?after=<next>` until `next` is absent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RegisteredDocuments {
    pub documents: Vec<RegisteredDocument>,
    /// The cursor for the following page: this page's last document. Absent on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub next: Option<String>,
}
impl RegisteredDocuments {
    /// The page for `rows`, read in document order after the cursor with a limit of one
    /// more than `ACCOUNT_DOCUMENTS_PAGE`: that extra row only says another page follows.
    pub fn page(mut rows: Vec<RegisteredDocument>) -> Self {
        let more = rows.len() > crate::limits::ACCOUNT_DOCUMENTS_PAGE;
        rows.truncate(crate::limits::ACCOUNT_DOCUMENTS_PAGE);
        let next = if more { rows.last().map(|row| row.document.clone()) } else { None };
        Self { documents: rows, next }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DocumentMetadata {
    pub title: String,
    pub template_id: String,
}
