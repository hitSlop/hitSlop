//! The Document DO is the sole permissions and lifecycle authority.
use crate::bounded::bounded;
use crate::limits::{DOCUMENT_MEMBERS, JOIN_BODY_BYTES, METADATA_BODY_BYTES, OBJECT_TIMEOUT};
use crate::refusal::Refusal;
use crate::room::{CloudDocument, NumberRow, SocketState, fail, hash, now, random_hex};
use pop_contracts::relay::frame::Frame;
use pop_sharing_protocol::http::RoomPhase;
use serde::Deserialize;
use worker::*;

use pop_sharing_protocol::document::{AccessState, DocumentMetadata, DocumentRole, JoinDocument, UpdateDocument};

#[derive(Deserialize)]
struct RegistryRow {
    uid: String,
    generation: i64,
}
impl CloudDocument {
    pub(crate) fn generation(&self) -> Result<u64> {
        self.document_state()?.and_then(|s| s.access_generation).ok_or_else(|| fail("Missing access generation"))
    }
    pub(crate) fn role_for_uid(&self, uid: &str) -> Result<Option<DocumentRole>> {
        let Some(state) = self.document_state()? else {
            return Ok(None);
        };
        self.role_in_state(uid, &state)
    }
    pub(crate) fn role_in_state(
        &self,
        uid: &str,
        state: &crate::document_state::DocumentState,
    ) -> Result<Option<DocumentRole>> {
        if state.owner_uid == uid {
            return Ok(Some(DocumentRole::Owner));
        }
        if state.phase == RoomPhase::Ended {
            return Ok(None);
        }
        let generation = state.access_generation.ok_or_else(|| fail("Missing access generation"))?;
        let n = self
            .sql
            .exec(
                "SELECT count(*) AS n FROM members WHERE uid=? AND generation=?",
                vec![uid.into(), (generation as i64).into()],
            )?
            .one::<NumberRow>()?
            .n;
        Ok((n > 0).then_some(DocumentRole::Editor))
    }
    /// Reserve a cleanup retry without postponing an earlier attachment-stage alarm.
    pub(crate) async fn schedule_cleanup(&self, delay_ms: u64) -> Result<()> {
        let requested = now().saturating_add(delay_ms);
        if self.state.storage().get_alarm().await?.is_none_or(|at| at as u64 > requested) {
            self.state.storage().set_alarm(std::time::Duration::from_millis(delay_ms.max(1))).await?;
        }
        Ok(())
    }
    pub(crate) fn queue_registry_releases(&self) -> Result<()> {
        self.sql.exec("INSERT INTO pending_registry SELECT uid,generation FROM members WHERE true ON CONFLICT(uid) DO UPDATE SET generation=max(pending_registry.generation,excluded.generation)",None)?;
        Ok(())
    }
    /// Queue `uid`'s release before an account call whose outcome may be lost: a join's
    /// registration or a leave's release. The row and its alarm are durable first, so a
    /// failure, timeout or reset after the call still ends in cleanup, which skips a member.
    /// The alarm falls due after the call's bound; a cleanup during the call waits for it.
    async fn queue_release(&self, uid: &str, generation: u64) -> Result<()> {
        self.sql.exec(
            "INSERT INTO pending_registry VALUES(?,?) ON CONFLICT(uid) DO UPDATE SET generation=max(pending_registry.generation,excluded.generation)",
            vec![uid.into(), (generation as i64).into()],
        )?;
        self.schedule_cleanup(OBJECT_TIMEOUT.as_millis() as u64 + 5_000).await?;
        self.state.storage().sync().await
    }
    /// The status of one call to `uid`'s account registry, or `None` when the outcome is
    /// unknown: the call failed or timed out, and the account may still have committed it.
    async fn account(&self, uid: &str, path: &str, generation: Option<u64>) -> Result<Option<u16>> {
        let body = serde_json::to_string(&crate::account::RegistryRequest {
            document: self.document_state()?.map(|s| s.document).ok_or_else(|| fail("Missing document"))?,
            generation,
        })?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_body(Some(body.into()));
        let request = Request::new_with_init(&format!("https://document.internal{path}"), &init)?;
        let stub = self.env.durable_object("ACCOUNTS")?.get_by_name(&hash(uid))?;
        Ok(bounded(stub.fetch_with_request(request), OBJECT_TIMEOUT).await.ok().map(|r| r.status_code()))
    }

    pub(crate) async fn release_registration(&self, uid: &str, generation: Option<u64>) -> Result<bool> {
        Ok(self.account(uid, "/internal/release", generation).await? == Some(200))
    }
    /// Release queued registrations at their generation or older. A queued UID that is a
    /// member holds a valid registration: only the row goes.
    pub(crate) async fn flush_registry(&self) -> Result<()> {
        for row in self.sql.exec("SELECT uid,generation FROM pending_registry", None)?.to_array::<RegistryRow>()? {
            // A join registers before inserting its member; a release between the two
            // would strip a live member's hint, so each release waits for the join.
            let _membership = self.membership.lock().await;
            if self.role_for_uid(&row.uid)?.is_some()
                || self.release_registration(&row.uid, Some(row.generation as u64)).await?
            {
                self.sql.exec(
                    "DELETE FROM pending_registry WHERE uid=? AND generation=?",
                    vec![row.uid.into(), row.generation.into()],
                )?;
            }
        }
        Ok(())
    }
    fn access_state(&self, role: DocumentRole) -> Result<AccessState> {
        let state = self.stored()?;
        Ok(AccessState {
            document: state.document,
            sharing: state.capability.is_some(),
            generation: state.access_generation.ok_or_else(|| fail("Missing access generation"))?,
            capability: if role == DocumentRole::Owner { state.capability } else { None },
            role,
        })
    }
    pub(crate) async fn join(&self, req: &mut Request, uid: &str, role: Option<DocumentRole>) -> Result<Response> {
        let input: JoinDocument = match crate::input::body(req, JOIN_BODY_BYTES).await {
            Ok(input) => input,
            Err(error) => return error.response(),
        };
        let invited = || -> Result<bool> {
            let state = self.stored()?;
            Ok(state.phase == RoomPhase::Live
                && state.capability.is_some_and(|cap| {
                    pop_contracts::wire::valid_attachment_id(&input.capability) && hash(&cap) == hash(&input.capability)
                }))
        };
        if !invited()? {
            return Refusal::new(403, "denied", "Invitation revoked").response();
        }
        if role == Some(DocumentRole::Owner) {
            return Response::from_json(&self.access_state(DocumentRole::Owner)?);
        }
        // Joins, leaves and registry cleanup on this document run one at a time. A holder
        // awaits only bounded `/internal/*` account calls, which never take an account's
        // `operations` lock, so account deletion (which holds that lock while it leaves
        // documents) waits here only for those calls: the lock graph has no cycle. Stop and
        // revocation never take this lock; the recheck after the account call catches them.
        let _membership = self.membership.lock().await;
        if !invited()? {
            return Refusal::new(403, "denied", "Invitation revoked").response();
        }
        // Other members only: a member joining again keeps its place. Nothing is
        // registered yet, so a refusal leaves no account hint.
        if self
            .sql
            .exec("SELECT count(*) AS n FROM members WHERE uid<>?", vec![uid.to_owned().into()])?
            .one::<NumberRow>()?
            .n
            >= DOCUMENT_MEMBERS
        {
            return Refusal::new(429, "limited", "Membership limit").response();
        }
        // Register before inserting: a member row never exists without its account hint,
        // and a hint without a member is a stale discovery entry that cleanup releases.
        // Its release is queued first, so a join that ends short of a member (refused, lost
        // reply, timed out, revoked or reset) leaves no hint behind; a member joining again
        // keeps the one it had.
        let generation = self.generation()?;
        self.queue_release(uid, generation).await?;
        let registered = self.account(uid, "/internal/member", Some(generation)).await?;
        if registered != Some(200) {
            return if registered.is_some() {
                Refusal::new(409, "conflict", "Account unavailable").response()
            } else {
                Refusal::new(503, "unavailable", "The sharing service refused this request").response()
            };
        }
        if !invited()? || self.generation()? != generation {
            return Refusal::new(403, "denied", "Invitation revoked").response();
        }
        self.sql.exec(
            "INSERT INTO members VALUES(?,?) ON CONFLICT(uid) DO UPDATE SET generation=excluded.generation",
            vec![uid.to_owned().into(), (generation as i64).into()],
        )?;
        // This registration now belongs to a member; no queued release may remove it.
        self.sql.exec("DELETE FROM pending_registry WHERE uid=?", vec![uid.into()])?;
        self.publish_library().await?;
        Response::from_json(&self.access_state(DocumentRole::Editor)?)
    }
    pub(crate) async fn leave(&self, uid: &str, role: Option<DocumentRole>) -> Result<Response> {
        if role == Some(DocumentRole::Owner) {
            return Refusal::new(409, "conflict", "Owner must delete document explicitly").response();
        }
        // Serialized with joins (see Join): a same-UID join completes, then this removes it.
        let _membership = self.membership.lock().await;
        // A hint left by a failed join is already queued for cleanup.
        if self.role_for_uid(uid)?.is_none() {
            return Response::ok("left");
        }
        // Revoke first, written with the queued release before any await: access never
        // depends on the account's reply.
        let generation = self.generation()?;
        self.sql.exec("DELETE FROM members WHERE uid=?", vec![uid.to_owned().into()])?;
        self.queue_release(uid, generation).await?;
        for ws in self.state.get_websockets() {
            if ws.deserialize_attachment::<SocketState>()?.is_some_and(|state| state.uid == uid) {
                let _ = ws.close(Some(1008), Some("Membership removed"));
            }
        }
        // A refused, lost or late reply leaves the release to cleanup.
        if self.release_registration(uid, Some(generation)).await? {
            self.sql.exec(
                "DELETE FROM pending_registry WHERE uid=? AND generation=?",
                vec![uid.into(), (generation as i64).into()],
            )?;
        }
        Response::ok("left")
    }
    pub(crate) fn get_metadata(&self) -> Result<Response> {
        let state = self.stored()?;
        Response::from_json(&DocumentMetadata {
            title: state.title.ok_or_else(|| fail("Missing title"))?,
            template_id: state.template_id.ok_or_else(|| fail("Missing template id"))?,
        })
    }
    pub(crate) fn get_sharing(&self, role: DocumentRole) -> Result<Response> {
        Response::from_json(&self.access_state(role)?)
    }
    pub(crate) async fn enable_sharing(&self) -> Result<Response> {
        if self.stored()?.capability.is_none() {
            let capability = random_hex(32)?;
            self.update_state(|s| s.capability = Some(capability))?;
            self.state.storage().sync().await?;
        }
        self.publish_library().await?;
        Response::from_json(&self.access_state(DocumentRole::Owner)?)
    }
    pub(crate) async fn disable_sharing(&self) -> Result<Response> {
        if self.document_state()?.and_then(|s| s.capability).is_some() {
            let generation = self
                .generation()?
                .checked_add(1)
                .filter(|v| *v <= i64::MAX as u64)
                .ok_or_else(|| fail("Access generation exhausted"))?;
            self.queue_registry_releases()?;
            self.sql.exec("DELETE FROM members; UPDATE document_state SET capability=NULL", None)?;
            self.update_state(|s| s.access_generation = Some(generation))?;
            self.schedule_cleanup(5_000).await?;
            self.state.storage().sync().await?;
            for ws in self.state.get_websockets() {
                if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|state| state.owner) {
                    let _ = self.send(
                        &ws,
                        Frame::Error {
                            code: pop_contracts::relay::frame::ErrorCode::Ended.into(),
                            message: "Invitation revoked".into(),
                        },
                    );
                    let _ = ws.close(Some(1008), Some("Invitation revoked"));
                }
            }
        }

        self.publish_library().await?;
        Response::from_json(&self.access_state(DocumentRole::Owner)?)
    }
    pub(crate) async fn patch_document(&self, req: &mut Request) -> Result<Response> {
        let input: UpdateDocument = match crate::input::body(req, METADATA_BODY_BYTES).await {
            Ok(input) => input,
            Err(error) => return error.response(),
        };
        if input.title.len() > pop_sharing_protocol::limits::TITLE_BYTES
            || input.template_id.len() > pop_sharing_protocol::limits::TEMPLATE_ID_BYTES
        {
            return Refusal::new(400, "invalid_request", "Invalid metadata").response();
        }
        // The body read yields; the document may have ended meanwhile. Nothing awaits
        // between this check and the writes below.
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Document deleted").response();
        }
        self.update_state(|s| {
            s.title = Some(input.title);
            s.template_id = Some(input.template_id);
        })?;
        self.publish_library().await?;
        Response::from_json(&self.access_state(DocumentRole::Owner)?)
    }
}
