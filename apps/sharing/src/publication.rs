//! Retryable document-to-library publication. Library metadata never grants access.
use super::*;
use futures_util::{StreamExt, stream};
use pop_sharing_protocol::{document::DocumentRole, library::AccountEntry};

#[derive(Serialize, Deserialize)]
pub(crate) struct Description {
    pub entry: AccountEntry,
    pub generation: Option<u64>,
}

impl CloudDocument {
    pub(super) async fn publish_library(&self) -> Result<()> {
        let revision = self.text("library_revision")?.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let revision = revision
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or_else(|| fail("Library revision exhausted"))?;
        self.put("library_revision", &revision.to_string())?;
        self.put("library_pending", "1")?;
        self.put("library_retry_ms", "10000")?;
        self.schedule_cleanup(1).await?;
        self.state.storage().sync().await
    }

    pub(super) async fn flush_library(&self) -> Result<()> {
        if self.text("library_pending")?.as_deref() != Some("1") || self.phase()? != Some(RoomPhase::Live) {
            return Ok(());
        }
        // Leave a durable wake before issuing calls whose outcome can be lost.
        let retry =
            self.text("library_retry_ms")?.and_then(|s| s.parse::<u64>().ok()).unwrap_or(10_000).clamp(10_000, 300_000);
        self.put("library_retry_ms", &retry.saturating_mul(2).min(300_000).to_string())?;
        self.schedule_cleanup(retry).await?;
        let revision: u64 = self.text("library_revision")?.and_then(|s| s.parse().ok()).unwrap_or(1);
        let document = self.text("document")?.ok_or_else(|| fail("Missing document"))?;
        let owner = self.text("owner_uid")?.ok_or_else(|| fail("Missing owner"))?;
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
        let title = self.text("title")?.unwrap_or_else(|| "Untitled".into());
        let template_id = self.text("template_id")?.unwrap_or_default();
        let sharing = self.text("capability")?.is_some();
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
                let call = std::pin::pin!(stub.fetch_with_request(req));
                let timeout = std::pin::pin!(Delay::from(std::time::Duration::from_secs(5)));
                use futures_util::future::{Either, select};
                Ok::<_, worker::Error>(matches!(select(call, timeout).await,
                    Either::Left((Ok(response), _)) if matches!(response.status_code(), 200 | 410)))
            }
        }))
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
        if outcomes.into_iter().all(|r| matches!(r, Ok(true)))
            && self.text("library_revision")?.as_deref() == Some(&revision.to_string())
        {
            self.put("library_pending", "0")?;
        }
        Ok(())
    }
}
