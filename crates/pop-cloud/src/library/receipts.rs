//! Durable acknowledgements share the device index transaction boundary.
use crate::error::Result;
use crate::transport;
use pop_runtime::owner::Owner;
use std::sync::Arc;

pub(crate) type Receipt = Arc<dyn Fn(String) -> Result<()> + Send + Sync>;

/// Record the exact local version that the relay acknowledged and the local save holds.
pub(crate) async fn confirm(
    owner: &Owner,
    outcome: &transport::OwnerOutcome,
    connection: &pop_runtime::owner::SyncConnection,
    receipt: Receipt,
) -> Result<bool> {
    let (send, receive) = tokio::sync::oneshot::channel();
    connection.durable_confirmed_version(move |confirmed| {
        let _ = send.send(confirmed);
    });
    let Some(confirmed) = transport::completion(outcome, receive, std::time::Duration::from_secs(20)).await? else {
        return Ok(false);
    };
    let reading = transport::owner_request(owner, outcome, Owner::version, transport::OwnerEffect::Other).await?;
    if reading.version != confirmed.version {
        return Ok(false);
    }
    transport::blocking(move || receipt(confirmed.version)).await?;
    Ok(true)
}
