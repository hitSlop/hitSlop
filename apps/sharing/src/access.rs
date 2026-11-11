//! The Document DO is the sole permissions and lifecycle authority.
use crate::limits::{DOCUMENT_MEMBERS, JOIN_BODY_BYTES, METADATA_BODY_BYTES};
use crate::refusal::Refusal;
use crate::room::{CloudDocument, NumberRow, SocketState, fail, hash, now, random_hex};
use pop_contracts::relay::frame::Frame;
use pop_sharing_protocol::document::{AccessState, DocumentRole, JoinDocument, JoinedDocument, UpdateDocument};
use pop_sharing_protocol::http::RoomPhase;
use worker::*;

impl CloudDocument {
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
        let n = self.sql.exec("SELECT count(*) AS n FROM members WHERE uid=?", vec![uid.into()])?.one::<NumberRow>()?.n;
        Ok((n > 0).then_some(DocumentRole::Editor))
    }
    /// Reserve a retry without postponing an earlier alarm.
    pub(crate) async fn schedule_cleanup(&self, delay_ms: u64) -> Result<()> {
        let requested = now().saturating_add(delay_ms);
        if self.state.storage().get_alarm().await?.is_none_or(|at| at as u64 > requested) {
            self.state.storage().set_alarm(std::time::Duration::from_millis(delay_ms.max(1))).await?;
        }
        Ok(())
    }
    fn access_state(&self, role: DocumentRole) -> Result<AccessState> {
        let state = self.stored()?;
        Ok(AccessState {
            document: state.document,
            sharing: state.capability.is_some(),
            capability: if role == DocumentRole::Owner { state.capability } else { None },
            role,
        })
    }
    pub(crate) async fn join(&self, req: &mut Request, uid: &str, _: Option<DocumentRole>) -> Result<Response> {
        let input: JoinDocument = match crate::input::body(req, JOIN_BODY_BYTES).await {
            Ok(input) => input,
            Err(error) => return error.response(),
        };
        self.schedule_cleanup(1).await?;
        let state = self.stored()?;
        if state.phase != RoomPhase::Live
            || !state.capability.is_some_and(|cap| {
                pop_contracts::wire::valid_attachment_id(&input.capability) && hash(&cap) == hash(&input.capability)
            })
        {
            return Refusal::new(403, "denied", "Invitation revoked").response();
        }
        let role = self.role_for_uid(uid)?;
        if role != Some(DocumentRole::Owner) {
            let uid = uid.to_owned();
            if self
                .sql
                .exec("SELECT count(*) AS n FROM members WHERE uid<>?", vec![uid.clone().into()])?
                .one::<NumberRow>()?
                .n
                >= DOCUMENT_MEMBERS
            {
                return Refusal::new(429, "limited", "Membership limit").response();
            }
            let result = self.library_transaction(move |sql, _| {
                if sql
                    .exec("SELECT count(*) AS n FROM members WHERE uid<>?", vec![uid.clone().into()])?
                    .one::<NumberRow>()?
                    .n
                    >= DOCUMENT_MEMBERS
                {
                    return Err(fail("Membership limit"));
                }
                sql.exec("INSERT OR IGNORE INTO members VALUES(?)", vec![uid.clone().into()])?;
                Ok(vec![])
            });
            result?;
        }
        let joined = JoinedDocument {
            access: self.access_state(if role == Some(DocumentRole::Owner) {
                DocumentRole::Owner
            } else {
                DocumentRole::Editor
            })?,
            title: self.stored()?.title.ok_or_else(|| fail("Missing title"))?,
            template_id: self.stored()?.template_id.ok_or_else(|| fail("Missing template id"))?,
        };
        self.state.storage().sync().await?;
        Response::from_json(&joined)
    }
    pub(crate) async fn leave(&self, uid: &str, _: Option<DocumentRole>) -> Result<Response> {
        self.schedule_cleanup(1).await?;
        if self.role_for_uid(uid)? == Some(DocumentRole::Owner) {
            return Refusal::new(409, "conflict", "Owner must delete document explicitly").response();
        }
        if self.phase()? == Some(RoomPhase::Ended) || self.role_for_uid(uid)?.is_none() {
            return Response::ok("left");
        }
        let removed = uid.to_owned();
        self.library_transaction(move |sql, _| {
            sql.exec("DELETE FROM members WHERE uid=?", vec![removed.clone().into()])?;
            Ok(vec![removed.clone()])
        })?;
        for ws in self.state.get_websockets() {
            if ws.deserialize_attachment::<SocketState>()?.is_some_and(|state| state.uid == uid) {
                self.clear_catchup(&ws)?;
                let _ = ws.close(Some(1008), Some("Membership removed"));
            }
        }
        self.state.storage().sync().await?;
        Response::ok("left")
    }
    pub(crate) fn get_sharing(&self, role: DocumentRole) -> Result<Response> {
        Response::from_json(&self.access_state(role)?)
    }
    pub(crate) async fn enable_sharing(&self) -> Result<Response> {
        self.schedule_cleanup(1).await?;
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Document deleted").response();
        }
        if self.stored()?.capability.is_none() {
            let capability = random_hex(32)?;
            self.library_transaction(move |_, s| {
                s.capability = Some(capability.clone());
                Ok(vec![])
            })?;
        }
        self.state.storage().sync().await?;
        Response::from_json(&self.access_state(DocumentRole::Owner)?)
    }
    pub(crate) async fn disable_sharing(&self) -> Result<Response> {
        self.schedule_cleanup(1).await?;
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Document deleted").response();
        }
        if self.stored()?.capability.is_some() {
            self.library_transaction(|sql, s| {
                let removed = sql
                    .exec("SELECT uid FROM members", None)?
                    .to_array::<crate::publication::Recipient>()?
                    .into_iter()
                    .map(|r| r.uid)
                    .collect();
                sql.exec("DELETE FROM members", None)?;
                s.capability = None;
                Ok(removed)
            })?;
            for ws in self.state.get_websockets() {
                if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|state| state.owner) {
                    let _ = self.send(
                        &ws,
                        Frame::Error {
                            code: pop_contracts::relay::frame::ErrorCode::Ended.into(),
                            message: "Invitation revoked".into(),
                        },
                    );
                    self.clear_catchup(&ws)?;
                    let _ = ws.close(Some(1008), Some("Invitation revoked"));
                }
            }
        }
        self.state.storage().sync().await?;
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
        self.schedule_cleanup(1).await?;
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Document deleted").response();
        }
        self.library_transaction(move |_, s| {
            s.title = Some(input.title.clone());
            s.template_id = Some(input.template_id.clone());
            Ok(vec![])
        })?;
        self.state.storage().sync().await?;
        Response::from_json(&self.access_state(DocumentRole::Owner)?)
    }
}
