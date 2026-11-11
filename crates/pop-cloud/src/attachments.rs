use crate::{client::*, transport};
use pop_runtime::owner::Owner;
use pop_sharing_protocol::resources::{self as resource_types, Resource};

pub(crate) fn inventory(items: Vec<pop_storage::store::Attachment>) -> Vec<Resource> {
    items
        .into_iter()
        .map(|item| Resource { key: item.id.clone(), hash: item.id, size: item.bytes, media_type: item.media_type })
        .collect()
}
fn local_blobs(owner: &Owner) -> std::result::Result<Vec<Resource>, transport::HttpFailure> {
    let items = request(owner, Owner::attachments)?;
    Ok(inventory(items))
}
pub(crate) fn remote_blobs(key: &RoomKey) -> std::result::Result<Vec<Resource>, transport::HttpFailure> {
    let list = transport::http_typed(key, "GET", "blobs", Some(key.bearer()), &[], &[])?;
    let items: Vec<Resource> = serde_json::from_slice(&list).map_err(fail)?;
    resource_types::validate_blobs(&items).map_err(fail)?;
    Ok(items)
}
/// The one attachment upload: every blob in `local` that `uploaded` (what the relay
/// holds) lacks, read through `read` only when it is sent. Blobs are immutable and named
/// by hash, so a retry after any failure sends only what is still missing.
pub(crate) fn upload(
    key: &RoomKey,
    local: &[Resource],
    uploaded: &mut Vec<Resource>,
    read: impl Fn(&str) -> std::result::Result<Vec<u8>, transport::HttpFailure>,
) -> std::result::Result<(), transport::HttpFailure> {
    for item in resource_types::missing_attachments(local, uploaded).map_err(fail)? {
        let bytes = read(&item.hash)?;
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
/// A live connection uploads what its owner holds before pushing changes that use it.
pub(crate) fn upload_blobs(
    owner: &Owner,
    key: &RoomKey,
    uploaded: &mut Vec<Resource>,
) -> std::result::Result<(), transport::HttpFailure> {
    upload(key, &local_blobs(owner)?, uploaded, |id| {
        let bytes =
            request(owner, |owner: &Owner, options, callback| owner.read_attachment(id.into(), options, callback))?;
        Ok(bytes)
    })
}
pub(crate) fn download_blobs(
    owner: &Owner,
    key: &RoomKey,
    uploaded: &mut Vec<Resource>,
) -> std::result::Result<(), transport::HttpFailure> {
    let remote = remote_blobs(key)?;
    resource_types::validate(&remote).map_err(fail)?;
    for item in resource_types::missing_attachments(&remote, &local_blobs(owner)?).map_err(fail)? {
        let bytes = transport::http_typed(key, "GET", &format!("blob/{}", item.hash), Some(key.bearer()), &[], &[])?;
        let accepted = pop_storage::store::Attachment::from_bytes(&bytes).map_err(transport::HttpFailure::from)?;
        if accepted.id != item.hash || accepted.bytes != item.size || accepted.media_type != item.media_type {
            return Err(fail("Downloaded attachment failed verification").into());
        }
        let stored = transport::owner_request(
            owner,
            |owner: &Owner, options, callback| owner.put_attachment(bytes, options, callback),
            transport::OwnerEffect::Attachment,
        )?;
        if stored.id != item.hash {
            return Err(fail("Attachment identity changed").into());
        }
    }
    *uploaded = remote;
    Ok(())
}
