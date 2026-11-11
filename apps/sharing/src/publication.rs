//! Retryable document-to-library publication. Library metadata never grants access.
use crate::bounded::bounded;
use crate::room::{CloudDocument, fail, hash};

use crate::limits::OBJECT_TIMEOUT;
use pop_sharing_protocol::http::RoomPhase;

use serde::{Deserialize, Serialize};
use worker::*;

use futures_util::{StreamExt, stream};
use pop_sharing_protocol::{document::DocumentRole, library::AccountEntry};

#[derive(Serialize, Deserialize)]
pub(crate) struct Description {
    pub entry: AccountEntry,
    pub generation: Option<u64>,
}

impl CloudDocument {
    pub(crate) fn library_revision(&self) -> Result<u64> {
        self.document_state()?.and_then(|s| s.library_revision).ok_or_else(|| fail("Missing library revision"))
    }
    pub(crate) async fn publish_library(&self) -> Result<()> {
        let revision = self.library_revision()?;
        let revision = revision
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or_else(|| fail("Library revision exhausted"))?;
        self.update_state(|s| s.library_revision = Some(revision))?;
        self.update_state(|s| s.library_pending = Some(1))?;
        self.update_state(|s| s.library_retry_ms = Some(10000))?;
        self.schedule_cleanup(1).await?;
        self.state.storage().sync().await
    }

    pub(crate) async fn flush_library(&self) -> Result<()> {
        if self.phase()? != Some(RoomPhase::Live) || self.stored()?.library_pending != Some(1) {
            return Ok(());
        }
        // Leave a durable wake before issuing calls whose outcome can be lost.
        let retry =
            self.stored()?.library_retry_ms.ok_or_else(|| fail("Missing library retry"))?.clamp(10_000, 300_000);
        self.update_state(|s| s.library_retry_ms = Some(retry.saturating_mul(2).min(300_000)))?;
        self.schedule_cleanup(retry).await?;
        let revision = self.library_revision()?;
        let document = self.document_state()?.map(|s| s.document).ok_or_else(|| fail("Missing document"))?;
        let owner = self.document_state()?.map(|s| s.owner_uid).ok_or_else(|| fail("Missing owner"))?;
        #[derive(Deserialize)]
        struct Member {
            uid: String,
            generation: i64,
        }
        let mut recipients = vec![(owner, None)];
        recipients.extend(
            self.sql
                .exec("SELECT uid,generation FROM members", None)?
                .to_array::<Member>()?
                .into_iter()
                .map(|m| (m.uid, Some(m.generation as u64))),
        );
        let title = self.stored()?.title.ok_or_else(|| fail("Missing title"))?;
        let template_id = self.stored()?.template_id.ok_or_else(|| fail("Missing template id"))?;
        let sharing = self.document_state()?.and_then(|s| s.capability).is_some();
        let outcomes = stream::iter(recipients.into_iter().map(|(uid, generation)| {
            let description = Description {
                entry: AccountEntry {
                    document: document.clone(),
                    role: if generation.is_none() { DocumentRole::Owner } else { DocumentRole::Editor },
                    title: title.clone(),
                    template_id: template_id.clone(),
                    sharing,
                    revision,
                },
                generation,
            };
            async move {
                let mut init = RequestInit::new();
                init.with_method(Method::Post).with_body(Some(serde_json::to_string(&description)?.into()));
                let req = Request::new_with_init("https://document.internal/internal/describe", &init)?;
                let stub = self.env.durable_object("ACCOUNTS")?.get_by_name(&hash(&uid))?;
                Ok::<_, worker::Error>(matches!(bounded(stub.fetch_with_request(req), OBJECT_TIMEOUT).await,
                    Ok(response) if matches!(response.status_code(), 200 | 410)))
            }
        }))
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
        if outcomes.into_iter().all(|r| matches!(r, Ok(true))) && self.stored()?.library_revision == Some(revision) {
            self.update_state(|s| s.library_pending = Some(0))?;
        }
        Ok(())
    }
}
