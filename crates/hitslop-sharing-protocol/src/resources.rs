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
/// Transfer only absent immutable blobs. Both inventories are untrusted at this
/// boundary; a matching hash with different metadata must never become a cache hit.
pub fn missing_attachments(required: &[Resource], present: &[Resource]) -> Result<Vec<Resource>, String> {
    validate(required, true)?;
    validate(present, true)?;
    let present: std::collections::HashMap<_, _> = present.iter().map(|item| (&item.hash, item)).collect();
    let mut missing = Vec::new();
    for item in required {
        if let Some(existing) = present.get(&item.hash) {
            if item.size != existing.size || item.media_type != existing.media_type {
                return Err("Attachment inventory metadata mismatch".into());
            }
        } else {
            missing.push(item.clone());
        }
    }
    Ok(missing)
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

#[cfg(test)]
mod transfer_tests {
    use super::*;
    fn blob(hash: char) -> Resource {
        let hash = hash.to_string().repeat(64);
        Resource { key: hash.clone(), hash, size: 42, media_type: "image/png".into() }
    }
    #[test]
    fn attachment_transfers_skip_remote_blobs_and_recover_lost_upload_replies() {
        let local = vec![blob('a'), blob('b')];
        let remote = vec![blob('a')];
        assert_eq!(missing_attachments(&local, &remote).unwrap()[0].hash, local[1].hash);
        assert!(missing_attachments(&remote, &local).unwrap().is_empty());
        // The upload succeeded but its reply was lost: the next inventory is enough
        // to skip it, without reading its bytes or retaining the previous connection.
        assert!(missing_attachments(&local, &local).unwrap().is_empty());
        assert_eq!(missing_attachments(&local, &[]).unwrap().len(), 2);
    }
    #[test]
    fn attachment_cache_hits_require_valid_matching_metadata() {
        let local = vec![blob('a')];
        let mut remote = local.clone();
        remote[0].size += 1;
        assert!(missing_attachments(&local, &remote).is_err());
        remote = local.clone();
        remote[0].media_type = "image/jpeg".into();
        assert!(missing_attachments(&remote, &local).is_err());
        remote = local.clone();
        remote.push(blob('a'));
        assert!(missing_attachments(&local, &remote).is_err());
        remote = local.clone();
        remote[0].hash = "invalid".into();
        assert!(missing_attachments(&local, &remote).is_err());
    }
}
