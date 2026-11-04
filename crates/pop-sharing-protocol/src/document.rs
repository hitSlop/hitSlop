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
    pub app_revision: Option<String>,
    pub preview_ref: Option<String>,
    pub modified_at: u64,
    pub role: DocumentRole,
    pub access_generation: u64,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RegisteredDocument {
    pub document: String,
    pub role: DocumentRole,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RegisteredDocuments {
    pub documents: Vec<RegisteredDocument>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DocumentMetadata {
    pub title: String,
    pub template_id: String,
}
