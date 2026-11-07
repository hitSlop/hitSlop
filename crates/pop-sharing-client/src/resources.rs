use super::*;
use pop_sharing_protocol::resources::{self as resource_types, Resource};

pub(crate) fn upload_app(key: &RoomKey, store: &Store, _path: &Path, token: &str) -> Result<()> {
    let reader = store.resource_reader().map_err(fail)?;
    let manifest = store.sharing_manifest().map_err(fail)?;
    transport::http(
        key,
        "POST",
        "resources",
        Some(token),
        &serde_json::to_vec(&manifest).map_err(fail)?,
        &[("Content-Type", "application/json")],
    )?;
    for item in manifest.resources {
        let bytes = reader
            .read_range(pop_storage::file::ResourceRoute::App, &item.key, 0, item.size)
            .map_err(fail)?
            .ok_or_else(|| fail("Missing app bytes"))?;
        transport::http(
            key,
            "PUT",
            &format!("asset/{}", item.hash),
            Some(token),
            &bytes,
            &[("Content-Type", &item.media_type)],
        )?;
    }
    Ok(())
}
fn local_blobs(owner: &Owner) -> std::result::Result<Vec<Resource>, transport::HttpFailure> {
    let Reply::Attachments { items } = request(owner, Request::Attachments)? else {
        return Err(fail("Missing attachment inventory").into());
    };
    Ok(items
        .into_iter()
        .map(|item| Resource { key: item.id.clone(), hash: item.id, size: item.bytes, media_type: item.media_type })
        .collect())
}
pub(crate) fn remote_blobs(key: &RoomKey) -> std::result::Result<Vec<Resource>, transport::HttpFailure> {
    let list = transport::http_typed(key, "GET", "blobs", Some(key.bearer()), &[], &[])?;
    let items: Vec<Resource> = serde_json::from_slice(&list).map_err(fail)?;
    resource_types::validate(&items, true).map_err(fail)?;
    Ok(items)
}
pub(crate) fn upload_blobs(
    owner: &Owner,
    key: &RoomKey,
    uploaded: &mut Vec<Resource>,
) -> std::result::Result<(), transport::HttpFailure> {
    for item in resource_types::missing_attachments(&local_blobs(owner)?, uploaded).map_err(fail)? {
        let Reply::Bytes { bytes: Some(bytes) } = request(owner, Request::ReadAttachment { id: item.hash.clone() })?
        else {
            return Err(fail("Missing attachment bytes").into());
        };
        transport::http_typed(
            key,
            "PUT",
            &format!("blob/{}", item.hash),
            Some(key.bearer()),
            &bytes,
            &[("Content-Type", &item.media_type)],
        )?;
        uploaded.push(item);
    }
    Ok(())
}
pub(crate) fn download_blobs(
    owner: &Owner,
    key: &RoomKey,
    uploaded: &mut Vec<Resource>,
) -> std::result::Result<(), transport::HttpFailure> {
    download_blobs_authenticated(owner, key, uploaded, key.bearer())
}
pub(crate) fn download_blobs_authenticated(
    owner: &Owner,
    key: &RoomKey,
    uploaded: &mut Vec<Resource>,
    token: &str,
) -> std::result::Result<(), transport::HttpFailure> {
    let list = transport::http_typed(key, "GET", "blobs", Some(token), &[], &[])?;
    let remote: Vec<Resource> = serde_json::from_slice(&list).map_err(fail)?;
    resource_types::validate(&remote, true).map_err(fail)?;
    for item in resource_types::missing_attachments(&remote, &local_blobs(owner)?).map_err(fail)? {
        let bytes = transport::http_typed(key, "GET", &format!("blob/{}", item.hash), Some(token), &[], &[])?;
        let accepted = pop_storage::store::Attachment::from_bytes(&bytes).map_err(transport::HttpFailure::from)?;
        if accepted.id != item.hash || accepted.bytes != item.size || accepted.media_type != item.media_type {
            return Err(fail("Downloaded attachment failed verification").into());
        }
        let Reply::Attachment { item: stored } = request(owner, Request::PutAttachment { bytes })? else {
            return Err(fail("Attachment was not stored").into());
        };
        if stored.id != item.hash {
            return Err(fail("Attachment identity changed").into());
        }
    }
    *uploaded = remote;
    Ok(())
}

/// Upload the exact attachment inventory contained in this attempt's seed.
pub(crate) fn upload_initial_blobs(key: &RoomKey, store: &Store, token: &str) -> Result<()> {
    for item in store.attachments().map_err(fail)? {
        let bytes = store.attachment(&item.id).map_err(fail)?;
        transport::http(
            key,
            "PUT",
            &format!("blob/{}", item.id),
            Some(token),
            &bytes,
            &[("Content-Type", &item.media_type)],
        )?;
    }
    Ok(())
}
