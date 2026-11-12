use crate::{client::*, transport};
use pop_runtime::owner::Owner;
use pop_sharing_protocol::resources::{self as resource_types, Resource};
use reqwest::Method;
use transport::{Endpoint, HttpOperation, OwnerEffect};

pub(crate) fn inventory(items: Vec<pop_storage::store::Attachment>) -> Vec<Resource> {
    items
        .into_iter()
        .map(|item| Resource { key: item.id.clone(), hash: item.id, size: item.bytes, media_type: item.media_type })
        .collect()
}
async fn local_blobs(
    owner: &Owner,
    outcome: &transport::OwnerOutcome,
) -> std::result::Result<Vec<Resource>, crate::Error> {
    let items = transport::owner_request(owner, outcome, Owner::attachments, OwnerEffect::Other).await?;
    Ok(inventory(items))
}
pub(crate) async fn remote_blobs(key: &RoomKey) -> std::result::Result<Vec<Resource>, crate::Error> {
    let list = transport::http(&key.origin, &key.bearer(), HttpOperation::new(Method::GET, Endpoint::Blobs(&key.room)))
        .await?
        .unwrap_or_default();
    let items: Vec<Resource> = serde_json::from_slice(&list).map_err(Error::from)?;
    resource_types::validate_blobs(&items).map_err(Error::from)?;
    Ok(items)
}
/// The one attachment upload: every blob in `local` that `uploaded` (what the relay
/// holds) lacks, read through `read` only when it is sent. Blobs are immutable and named
/// by hash, so a retry after any failure sends only what is still missing.
pub(crate) async fn upload<F, Fut>(
    key: &RoomKey,
    local: &[Resource],
    uploaded: &mut Vec<Resource>,
    read: F,
) -> Result<()>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>>>,
{
    for item in resource_types::missing_attachments(local, uploaded)? {
        let bytes = read(item.hash.clone()).await?;
        transport::http(
            &key.origin,
            &key.bearer(),
            HttpOperation::new(Method::PUT, Endpoint::Blob(&key.room, &item.hash))
                .body(&bytes)
                .headers(&[("Content-Type", &item.media_type)]),
        )
        .await?;
        uploaded.push(item);
    }
    Ok(())
}
pub(crate) async fn upload_blobs(
    owner: &Owner,
    outcome: &transport::OwnerOutcome,
    key: &RoomKey,
    uploaded: &mut Vec<Resource>,
) -> Result<()> {
    upload(key, &local_blobs(owner, outcome).await?, uploaded, |id| async move {
        transport::owner_request(
            owner,
            outcome,
            |owner, options, callback| owner.read_attachment(id, options, callback),
            OwnerEffect::Other,
        )
        .await
    })
    .await
}
pub(crate) async fn download_blobs(
    owner: &Owner,
    outcome: &transport::OwnerOutcome,
    key: &RoomKey,
    uploaded: &mut Vec<Resource>,
) -> Result<()> {
    let remote = remote_blobs(key).await?;
    resource_types::validate(&remote)?;
    for item in resource_types::missing_attachments(&remote, &local_blobs(owner, outcome).await?)? {
        let bytes = transport::http(
            &key.origin,
            &key.bearer(),
            HttpOperation::new(Method::GET, Endpoint::Blob(&key.room, &item.hash)),
        )
        .await?
        .unwrap_or_default();
        let (accepted, bytes) =
            transport::blocking(move || Ok((pop_storage::store::Attachment::from_bytes(&bytes)?, bytes))).await?;
        if accepted.id != item.hash || accepted.bytes != item.size || accepted.media_type != item.media_type {
            return Err("Downloaded attachment failed verification".into());
        }
        let stored = transport::owner_request(
            owner,
            outcome,
            |owner, options, callback| owner.put_attachment(bytes, options, callback),
            OwnerEffect::Attachment,
        )
        .await?;
        if stored.id != item.hash {
            return Err("Attachment identity changed".into());
        }
    }
    *uploaded = remote;
    Ok(())
}
