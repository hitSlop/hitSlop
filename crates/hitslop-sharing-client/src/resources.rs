use super::*;
use hitslop_sharing_protocol::resources::{self as resource_types, Manifest, Resource};

/// The incoming seed is transport-bounded; a valid local file may have grown beyond
/// CloudKit's preview checkpoint limit. Keep that policy out of relay reconnection.
pub(crate) fn merge_seed(owner: &Owner, source: &Path) -> std::result::Result<(), transport::HttpFailure> {
    let remote = Store::open(source, Mode::Snapshot).map_err(transport::HttpFailure::from)?;
    let local = Store::open(owner.path(), Mode::Snapshot).map_err(fail)?;
    if remote.app().document_uuid != local.app().document_uuid
        || remote.app_digest().map_err(fail)? != local.app_digest().map_err(fail)?
    {
        return Err(fail("Seed identity/app mismatch").into());
    }
    for item in remote.attachments().map_err(fail)? {
        let bytes = remote.attachment(&item.id).map_err(fail)?;
        request(owner, Request::PutAttachment { bytes })?;
    }
    let document = remote.document().map_err(transport::HttpFailure::from)?;
    let bytes = document.checkpoint().map_err(fail)?;
    snapshot::request_typed(owner, Request::MergeSnapshot { bytes }).map_err(transport::HttpFailure::from)?;
    request(owner, Request::Flush)?;
    Ok(())
}

pub(crate) fn upload_app(key: &RoomKey, store: &Store, path: &Path, token: &str) -> Result<()> {
    let reader = store.resource_reader().map_err(fail)?;
    let mut resources = vec![];
    for asset in hitslop_core::file::inspect(path).map_err(fail)?.assets {
        if !resource_types::asset_key(&asset.name) {
            continue;
        }
        let route = hitslop_core::file::ResourceRoute::App;
        if let Some(info) = reader.info(route, &asset.name).map_err(fail)? {
            let bytes = reader
                .read_range(route, &asset.name, 0, info.size)
                .map_err(fail)?
                .ok_or_else(|| fail("Missing app bytes"))?;
            resources.push(Resource {
                key: asset.name,
                hash: hex(&Sha256::digest(&bytes)),
                size: info.size,
                media_type: info.media_type,
            });
        }
    }
    resource_types::validate(&resources, false).map_err(fail)?;
    let manifest = Manifest { app: hex(&store.app_digest().map_err(fail)?), resources };
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
            .read_range(hitslop_core::file::ResourceRoute::App, &item.key, 0, item.size)
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
fn local_blobs(owner: &Owner) -> Result<Vec<Resource>> {
    let Reply::Attachments { items } = request(owner, Request::Attachments)? else {
        return Err(fail("Missing attachment inventory"));
    };
    Ok(items
        .into_iter()
        .map(|item| Resource { key: item.id.clone(), hash: item.id, size: item.bytes, media_type: item.media_type })
        .collect())
}
pub(crate) fn remote_blobs(key: &RoomKey) -> Result<Vec<Resource>> {
    let list = transport::http(key, "GET", "blobs", Some(&key.capability), &[], &[])?;
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
            Some(&key.capability),
            &bytes,
            &[("Content-Type", &item.media_type)],
        )?;
        uploaded.push(item);
    }
    Ok(())
}
pub(crate) fn download_blobs(owner: &Owner, key: &RoomKey, uploaded: &mut Vec<Resource>) -> Result<()> {
    let remote = remote_blobs(key)?;
    for item in resource_types::missing_attachments(&remote, &local_blobs(owner)?).map_err(fail)? {
        let bytes = transport::http(key, "GET", &format!("blob/{}", item.hash), Some(&key.capability), &[], &[])?;
        let accepted = hitslop_core::store::Attachment::from_bytes(&bytes).map_err(fail)?;
        if accepted.id != item.hash || accepted.bytes != item.size || accepted.media_type != item.media_type {
            return Err(fail("Downloaded attachment failed verification"));
        }
        let Reply::Attachment { item: stored } = request(owner, Request::PutAttachment { bytes })? else {
            return Err(fail("Attachment was not stored"));
        };
        if stored.id != item.hash {
            return Err(fail("Attachment identity changed"));
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
