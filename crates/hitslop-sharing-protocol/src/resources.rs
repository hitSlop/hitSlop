//! Small experimental HTTP inventory. No document state or semantic relay parsing.
use crate::limits;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub key: String,
    pub hash: String,
    pub size: u64,
    pub media_type: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub app: String,
    pub resources: Vec<Resource>,
}
pub fn hash_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
pub fn asset_key(key: &str) -> bool {
    matches!(key, "ui.js" | "ui.css")
        || key.strip_prefix("media/").is_some_and(|name| {
            name.split_once('.').is_some_and(|(id, ext)| {
                hash_id(id) && !ext.is_empty() && ext.bytes().all(|c| c.is_ascii_alphanumeric())
            })
        })
}
pub fn validate(items: &[Resource], attachments: bool) -> Result<(), String> {
    let (count, file, bytes) = if attachments {
        (limits::ATTACHMENT_COUNT, limits::ATTACHMENT_FILE_BYTES, limits::ATTACHMENT_BYTES)
    } else {
        (limits::ASSET_COUNT, limits::ASSET_FILE_BYTES, limits::ASSET_BYTES)
    };
    let mut keys = std::collections::HashSet::new();
    let mut total = 0u64;
    for item in items {
        if !hash_id(&item.hash)
            || item.size > file as u64
            || !keys.insert(&item.key)
            || item.media_type.is_empty()
            || item.media_type.len() > 100
            || item.media_type.contains(['\r', '\n'])
            || (attachments && (item.key != item.hash || !passive(&item.media_type)))
            || (!attachments && !asset_key(&item.key))
        {
            return Err("Invalid resource inventory".into());
        }
        total = total.checked_add(item.size).ok_or("Resource inventory overflow")?;
    }
    if items.len() > count || total > bytes as u64 {
        return Err("Resource inventory exceeds document limits".into());
    }
    Ok(())
}
fn passive(kind: &str) -> bool {
    matches!(
        kind,
        "application/octet-stream"
            | "application/pdf"
            | "image/png"
            | "image/jpeg"
            | "image/webp"
            | "image/gif"
            | "image/avif"
            | "image/heic"
            | "image/heif"
            | "audio/mpeg"
            | "audio/mp4"
            | "audio/wav"
            | "audio/ogg"
            | "audio/webm"
            | "video/mp4"
            | "video/webm"
    )
}
