//! Durable, revisioned document-to-account delivery. Library metadata never grants access.
use crate::{
    bounded::bounded,
    document_state::DocumentState,
    limits::OBJECT_TIMEOUT,
    room::{CloudDocument, fail, hash},
};
use futures_util::{StreamExt, stream};
use pop_sharing_protocol::{document::DocumentRole, http::RoomPhase, library::AccountEntry};
use serde::{Deserialize, Serialize};
use worker::*;

/// Internal desired account state. Uploads are registered for deletion but hidden from the feed.
#[derive(Serialize, Deserialize)]
pub(crate) struct ApplyEntry {
    pub recipient: String,
    pub document: String,
    pub revision: u64,
    pub entry: Option<AccountEntry>,
    pub visible: bool,
}
#[derive(Deserialize)]
pub(crate) struct Recipient {
    pub uid: String,
}
#[derive(Deserialize)]
struct Pending {
    uid: String,
    revision: i64,
    payload: String,
    retry_ms: u64,
}
impl CloudDocument {
    /// The mutation, revision and latest desired entries commit together without yielding.
    /// The caller reserves a durable alarm before entering this transaction.
    pub(crate) fn library_transaction(
        &self,
        mut edit: impl FnMut(&SqlStorage, &mut DocumentState) -> Result<Vec<String>> + 'static,
    ) -> Result<()> {
        let sql = self.sql.clone();
        self.transaction(move || {
            let mut state = DocumentState::load(&sql)?.ok_or_else(|| fail("Missing document"))?;
            let removed = edit(&sql, &mut state)?;
            let revision = state.library_revision.unwrap_or(0).checked_add(1)
                .filter(|v| *v <= i64::MAX as u64).ok_or_else(|| fail("Library revision exhausted"))?;
            let mut entries = Vec::new();
            for uid in removed {
                entries.push((uid, None));
            }
            if state.phase == RoomPhase::Ended {
                entries.push((state.owner_uid.clone(), None));
            } else {
                entries.push((state.owner_uid.clone(), Some(DocumentRole::Owner)));
                entries.extend(sql.exec("SELECT uid FROM members", None)?.to_array::<Recipient>()?
                    .into_iter().map(|r| (r.uid, Some(DocumentRole::Editor))));
            }
            for (uid, role) in entries {
                let entry = role.map(|role| AccountEntry {
                    document: state.document.clone(), role,
                    title: state.title.clone().unwrap_or_default(),
                    template_id: state.template_id.clone().unwrap_or_default(),
                    sharing: state.capability.is_some(), revision,
                });
                let payload = serde_json::to_string(&ApplyEntry {
                    recipient: uid.clone(), document: state.document.clone(), revision, entry,
                    visible: state.phase == RoomPhase::Live,
                })?;
                sql.exec("INSERT INTO library_outbox VALUES(?,?,?,10000) ON CONFLICT(uid) DO UPDATE SET revision=excluded.revision,payload=excluded.payload,retry_ms=10000",
                    vec![uid.into(), (revision as i64).into(), payload.into()])?;
            }
            state.library_revision = (state.phase != RoomPhase::Ended).then_some(revision);
            state.save(&sql)
        })
    }
    pub(crate) async fn flush_library(&self) -> Result<()> {
        let rows =
            self.sql.exec("SELECT uid,revision,payload,retry_ms FROM library_outbox", None)?.to_array::<Pending>()?;
        if rows.is_empty() {
            return Ok(());
        }
        let retry = rows.iter().map(|r| r.retry_ms).min().unwrap_or(10_000).clamp(10_000, 300_000);
        self.schedule_cleanup(retry).await?;
        for row in &rows {
            self.sql.exec(
                "UPDATE library_outbox SET retry_ms=min(retry_ms*2,300000) WHERE uid=? AND revision=?",
                vec![row.uid.clone().into(), row.revision.into()],
            )?;
        }
        let outcomes = stream::iter(rows.into_iter().map(|row| async move {
            let delivered = async {
                let mut init = RequestInit::new();
                init.with_method(Method::Post).with_body(Some(row.payload.clone().into()));
                let req = Request::new_with_init("https://document.internal/internal/apply-entry", &init)?;
                let stub = self.env.durable_object("ACCOUNTS")?.get_by_name(&hash(&row.uid))?;
                let mut reply = bounded(stub.fetch_with_request(req), OBJECT_TIMEOUT).await?;
                if reply.status_code() != 200 {
                    return Ok::<_, worker::Error>(false);
                }
                let acknowledged: u64 = reply.json().await?;
                Ok(acknowledged >= row.revision as u64)
            }
            .await;
            (row, delivered)
        }))
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
        for (row, result) in outcomes {
            if matches!(result, Ok(true)) {
                // A newer local change may have replaced this row while delivery awaited.
                self.sql.exec(
                    "DELETE FROM library_outbox WHERE uid=? AND revision=?",
                    vec![row.uid.into(), row.revision.into()],
                )?;
            }
        }
        Ok(())
    }
}
