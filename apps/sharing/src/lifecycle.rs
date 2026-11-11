//! Document lifecycle operations.
use crate::metrics::{Metric, record};
use crate::refusal::Refusal;
use crate::room::object_key;
use crate::room::{CloudDocument, fail};
use pop_contracts::relay::frame::ErrorCode;
use pop_sharing_protocol::CONTROL_BYTES;
use pop_sharing_protocol::http::RoomPhase;
use worker::*;

impl CloudDocument {
    pub(crate) async fn initialize_document(&self, req: &mut Request) -> Result<Response> {
        let input: crate::account::DocumentInit = match crate::input::body(req, CONTROL_BYTES).await {
            Ok(input) => input,
            Err(error) => return error.response(),
        };
        if !pop_contracts::identity::valid_document_id(&input.document) || input.owner_uid.is_empty() {
            return Refusal::new(400, "invalid_request", "Invalid metadata").response();
        }
        let existing = self.document_state()?;
        if existing.as_ref().is_some_and(|s| s.phase == RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        let creating = existing.is_none();
        if creating {
            crate::policy::transition(None, RoomPhase::Uploading).map_err(|_| fail("Invalid phase transition"))?;
            self.sql.exec("INSERT INTO document_state(id,owner_uid,document,phase,access_generation,library_revision,library_pending,library_retry_ms,heads,title,template_id,cleanup_pending) VALUES(1,?,?,'uploading',1,1,0,10000,'[]','Untitled','',0)",vec![input.owner_uid.clone().into(),input.document.clone().into()])?;
            record(&self.env, Metric::DocumentCreated);
        }
        let state = self.stored()?;
        if state.owner_uid != input.owner_uid || state.document != input.document {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        self.state.storage().sync().await?;
        Response::empty().map(|r| r.with_status(204))
    }
    pub(crate) fn describe_document(&self, role: pop_sharing_protocol::document::DocumentRole) -> Result<Response> {
        let state = self.stored()?;
        let revision = state.library_revision.ok_or_else(|| fail("Missing library revision"))?;
        Response::from_json(&crate::publication::Description {
            entry: pop_sharing_protocol::library::AccountEntry {
                document: state.document,
                role,
                title: state.title.ok_or_else(|| fail("Missing title"))?,
                template_id: state.template_id.ok_or_else(|| fail("Missing template id"))?,
                sharing: state.capability.is_some(),
                revision,
            },
            generation: if role == pop_sharing_protocol::document::DocumentRole::Owner {
                None
            } else {
                Some(self.generation()?)
            },
        })
    }
    pub(crate) async fn delete_document(&self, room: &str) -> Result<Response> {
        let response = self.stop_room(room, Metric::DocumentDeleted).await?;
        self.flush_registry().await?;
        Ok(response)
    }
    pub(crate) async fn stop_room(&self, room: &str, reason: Metric) -> Result<Response> {
        if self.phase()? != Some(RoomPhase::Ended) {
            self.set_phase(RoomPhase::Ended)?;
            self.queue_registry_releases()?;
            self.sql.exec("DELETE FROM members; UPDATE document_state SET capability=NULL,title=NULL,template_id=NULL,library_revision=NULL,library_pending=NULL,library_retry_ms=NULL,cleanup_pending=1", None)?;
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
            self.sql.exec("DELETE FROM resource_blobs; DELETE FROM records; DELETE FROM chunks; UPDATE document_state SET seed_hash=NULL,seed_size=NULL,root=NULL,heads=NULL,base_version=NULL,app=NULL",None)?;
            self.state.storage().sync().await?;
            if let Some(uid) = self.document_state()?.map(|s| s.owner_uid)
                && !self.release_registration(&uid, None).await?
            {
                return Err(fail("Account release failed"));
            }
            self.sql.exec("UPDATE document_state SET access_generation=NULL,cleanup_pending=NULL", None)?;
            self.state.storage().sync().await?;
        }
        Response::ok("deleted")
    }
}
