//! Attachment inventory validation. No document state or semantic relay parsing.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub struct Resource {
    pub key: String,
    pub hash: String,
    pub size: u64,
    pub media_type: String,
}
pub fn validate(items: &[Resource]) -> Result<(), pop_contracts::Error> {
    validate_blobs(items)?;
    if items.iter().any(|item| item.size > pop_contracts::wire::ATTACHMENT_FILE_BYTES as u64) {
        return Err(invalid("Invalid resource inventory"));
    }
    let mut total = 0u64;
    for item in items {
        total = total.checked_add(item.size).ok_or_else(|| invalid("Resource inventory overflow"))?;
    }
    if items.len() > pop_contracts::wire::ATTACHMENT_COUNT || total > pop_contracts::wire::ATTACHMENT_BYTES as u64 {
        return Err(invalid("Resource inventory exceeds document limits"));
    }
    Ok(())
}
/// Staged blobs include the seed and abandoned attempts, outside attachment budgets.
pub fn validate_blobs(items: &[Resource]) -> Result<(), pop_contracts::Error> {
    let mut keys = std::collections::HashSet::new();
    for item in items {
        if !pop_contracts::wire::valid_attachment_id(&item.hash)
            || item.size > crate::SEED_BYTES as u64
            || !keys.insert(&item.key)
            || item.media_type.is_empty()
            || item.media_type.len() > 100
            || item.media_type.contains(['\r', '\n'])
            || item.key != item.hash
            || !pop_contracts::media::attachment_media_type(&item.media_type)
        {
            return Err(invalid("Invalid blob inventory"));
        }
    }
    Ok(())
}
/// Transfer only absent immutable blobs. Both inventories are untrusted at this
/// boundary; a matching hash with different metadata must never become a cache hit.
pub fn missing_attachments(required: &[Resource], present: &[Resource]) -> Result<Vec<Resource>, pop_contracts::Error> {
    validate(required)?;
    validate_blobs(present)?;
    let present: std::collections::HashMap<_, _> = present.iter().map(|item| (&item.hash, item)).collect();
    let mut missing = Vec::new();
    for item in required {
        if let Some(existing) = present.get(&item.hash) {
            if item.size != existing.size || item.media_type != existing.media_type {
                return Err(invalid("Attachment inventory metadata mismatch"));
            }
        } else {
            missing.push(item.clone());
        }
    }
    Ok(missing)
}

fn invalid(message: impl ToString) -> pop_contracts::Error {
    pop_contracts::Error::new(pop_contracts::wire::Code::InvalidRequest, message)
}

#[cfg(test)]
mod transfer_tests {
    use super::*;
    fn blob(hash: char) -> Resource {
        let hash = hash.to_string().repeat(64);
        Resource { key: hash.clone(), hash, size: 42, media_type: "image/png".into() }
    }
    #[test]
    fn inventory_accepts_flac_and_quicktime_attachments() {
        let mut items = vec![blob('a'), blob('b')];
        items[0].media_type = "audio/flac".into();
        items[1].media_type = "video/quicktime".into();
        assert!(validate(&items).is_ok());
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
