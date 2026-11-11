//! Document lifecycle operations.
use crate::metrics::{Metric, record};
use crate::refusal::Refusal;
use crate::room::object_key;
use crate::room::{SharedDocument, fail};
use pop_contracts::relay::frame::ErrorCode;
use pop_sharing_protocol::CONTROL_BYTES;
use pop_sharing_protocol::http::RoomPhase;
use worker::*;

impl SharedDocument {
    pub(crate) async fn initialize_document(&self, req: &mut Request) -> Result<Response> {
        let input: crate::account::DocumentInit = match crate::input::body(req, CONTROL_BYTES).await {
            Ok(input) => input,
            Err(error) => return error.response(),
        };
        if !pop_contracts::identity::valid_document_id(&input.document) || input.owner_uid.is_empty() {
            return Refusal::new(400, "invalid_request", "Invalid metadata").response();
        }
        self.schedule_cleanup(1).await?;
        let existing = self.document_state()?;
        if existing.as_ref().is_some_and(|s| s.phase == RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        let creating = existing.is_none();
        if creating {
            crate::policy::transition(None, RoomPhase::Uploading).map_err(|_| fail("Invalid phase transition"))?;
            self.sql.exec("INSERT INTO document_state(id,owner_uid,document,phase,library_revision,update_bytes,update_rows,title,template_id,cleanup_pending) VALUES(1,?,?,'uploading',0,0,0,'Untitled','',0)",vec![input.owner_uid.clone().into(),input.document.clone().into()])?;
            record(&self.env, Metric::DocumentCreated);
        }
        let state = self.stored()?;
        if state.owner_uid != input.owner_uid || state.document != input.document {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        self.library_transaction(|_, _| Ok(vec![]))?;
        self.state.storage().sync().await?;
        Response::empty().map(|r| r.with_status(204))
    }
    pub(crate) async fn delete_document(&self, room: &str) -> Result<Response> {
        self.stop_room(room, Metric::DocumentDeleted).await
    }
    pub(crate) async fn stop_room(&self, room: &str, reason: Metric) -> Result<Response> {
        if self.phase()? != Some(RoomPhase::Ended) {
            self.schedule_cleanup(5_000).await?;
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::ok("deleted");
            }
            self.library_transaction(|sql, s| {
                let removed = sql
                    .exec("SELECT uid FROM members", None)?
                    .to_array::<crate::publication::Recipient>()?
                    .into_iter()
                    .map(|r| r.uid)
                    .collect();
                sql.exec("DELETE FROM members", None)?;
                s.phase = crate::policy::transition(Some(s.phase), RoomPhase::Ended)
                    .map_err(|_| fail("Invalid phase transition"))?;
                s.capability = None;
                s.title = None;
                s.template_id = None;
                s.cleanup_pending = Some(1);
                Ok(removed)
            })?;
            record(&self.env, reason);
        }
        // Identity/deletion tombstones persist: the UUID can never be reclaimed by
        // another account. Retry cleanup and discovery publication independently.
        if self.stored()?.cleanup_pending == Some(1) {
            self.schedule_cleanup(5_000).await?;
            self.state.storage().sync().await?;
            self.end_connections(ErrorCode::Ended, "Document deleted", 1008);
            let bucket = self.env.bucket("SEEDS")?;
            let prefix = object_key(room, "");
            loop {
                let objects = bucket.list().prefix(prefix.clone()).execute().await?.objects();
                if objects.is_empty() {
                    break;
                }
                for item in objects {
                    bucket.delete(item.key()).await?;
                }
            }
            self.sql.exec("DELETE FROM resource_blobs; DELETE FROM records; DELETE FROM chunks; DELETE FROM heads; DELETE FROM catchups; UPDATE document_state SET seed_hash=NULL,seed_size=NULL,root=NULL,update_bytes=NULL,update_rows=NULL,base_version=NULL,app=NULL,staged_upload_ms=NULL",None)?;
            self.state.storage().sync().await?;
            self.sql.exec("UPDATE document_state SET cleanup_pending=NULL", None)?;
            self.state.storage().sync().await?;
        }
        Response::ok("deleted")
    }
}
