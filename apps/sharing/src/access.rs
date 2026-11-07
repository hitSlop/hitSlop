//! The Document DO is the sole permissions and lifecycle authority.
use super::*;
use pop_sharing_protocol::{
    document::{AccessState, DocumentMetadata, DocumentRole, JoinDocument, UpdateDocument},
    routes::Op,
};
#[derive(Deserialize)]
struct RegistryRow {
    uid: String,
    generation: i64,
}
impl CloudDocument {
    pub(super) fn generation(&self) -> Result<u64> {
        Ok(self.text("access_generation")?.and_then(|v| v.parse().ok()).unwrap_or(1))
    }
    pub(super) async fn identity_role(&self, req: &Request) -> Result<Option<DocumentRole>> {
        let claims = match crate::auth::identity(req, &self.env).await {
            Ok(claims) => claims,
            Err(crate::auth::IdentityError::Denied) => return Ok(None),
            Err(crate::auth::IdentityError::Unavailable) => return Err(fail("Identity unavailable")),
        };
        self.role_for_uid(&claims.sub)
    }
    fn role_for_uid(&self, uid: &str) -> Result<Option<DocumentRole>> {
        if self.text("owner_uid")?.as_deref() == Some(uid) {
            return Ok(Some(DocumentRole::Owner));
        }
        let n = self
            .sql
            .exec(
                "SELECT count(*) AS n FROM members WHERE uid=? AND generation=?",
                vec![uid.into(), (self.generation()? as i64).into()],
            )?
            .one::<NumberRow>()?
            .n;
        Ok((n > 0).then_some(DocumentRole::Editor))
    }
    /// Reserve a cleanup retry without postponing an earlier attachment-stage alarm.
    pub(super) async fn schedule_cleanup(&self, delay_ms: u64) -> Result<()> {
        let requested = now().saturating_add(delay_ms);
        if self.state.storage().get_alarm().await?.is_none_or(|at| at as u64 > requested) {
            self.state.storage().set_alarm(std::time::Duration::from_millis(delay_ms.max(1))).await?;
        }
        Ok(())
    }
    pub(super) fn queue_registry_releases(&self) -> Result<()> {
        self.sql.exec("INSERT INTO pending_registry SELECT uid,generation FROM members WHERE true ON CONFLICT(uid) DO UPDATE SET generation=max(pending_registry.generation,excluded.generation)",None)?;
        Ok(())
    }
    pub(super) async fn release_registration(&self, uid: &str, generation: Option<u64>) -> Result<bool> {
        let body = serde_json::to_string(&crate::account::RegistryRequest {
            document: self.text("document")?.ok_or_else(|| fail("Missing document"))?,
            generation,
        })?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_body(Some(body.into()));
        let response = self
            .env
            .durable_object("ACCOUNTS")?
            .get_by_name(&hash(uid))?
            .fetch_with_request(Request::new_with_init("https://document.internal/internal/release", &init)?)
            .await?;
        Ok(response.status_code() == 200)
    }
    pub(super) async fn flush_registry(&self) -> Result<()> {
        for row in self.sql.exec("SELECT uid,generation FROM pending_registry", None)?.to_array::<RegistryRow>()? {
            if self.release_registration(&row.uid, Some(row.generation as u64)).await.unwrap_or(false) {
                self.sql.exec(
                    "DELETE FROM pending_registry WHERE uid=? AND generation=?",
                    vec![row.uid.into(), row.generation.into()],
                )?;
            }
        }
        Ok(())
    }
    fn access_state(&self, role: DocumentRole) -> Result<AccessState> {
        Ok(AccessState {
            document: self.text("document")?.ok_or_else(|| fail("Missing document"))?,
            sharing: self.text("capability")?.is_some(),
            generation: self.generation()?,
            capability: if role == DocumentRole::Owner { self.text("capability")? } else { None },
            role,
        })
    }
    pub(super) async fn access_route(&self, req: &mut Request, op: Op) -> Result<Response> {
        if self.phase()? == Some(RoomPhase::Ended) {
            return Response::error("Document deleted", 410);
        }
        if op == Op::GetMetadata && self.phase()? != Some(RoomPhase::Live) {
            return Response::error("Document upload is not ready", 409);
        }
        if op == Op::Join {
            let claims = match crate::auth::identity(req, &self.env).await {
                Ok(c) => c,
                Err(e) => return identity_response(e),
            };
            let input: JoinDocument = match crate::input::body(req, 4096).await {
                Ok(input) => input,
                Err(error) => return error.response(),
            };
            if self.phase()? != Some(RoomPhase::Live)
                || self
                    .text("capability")?
                    .is_none_or(|cap| input.capability.len() != 64 || hash(&cap) != hash(&input.capability))
            {
                return Response::error("Invitation revoked", 403);
            }
            let owner = self.text("owner_uid")?.as_deref() == Some(&claims.sub);
            if !owner {
                if self
                    .sql
                    .exec("SELECT count(*) AS n FROM members WHERE uid<>?", vec![claims.sub.clone().into()])?
                    .one::<NumberRow>()?
                    .n
                    >= 64
                {
                    return Response::error("Membership limit", 429);
                }
                let generation = self.generation()?;
                let membership_revision = self.text("membership_revision")?;
                let mut init = RequestInit::new();
                init.with_method(Method::Post).with_body(Some(
                    serde_json::to_string(&crate::account::RegistryRequest {
                        document: self.text("document")?.ok_or_else(|| fail("Missing document"))?,
                        generation: Some(generation),
                    })?
                    .into(),
                ));
                let response = self
                    .env
                    .durable_object("ACCOUNTS")?
                    .get_by_name(&hash(&claims.sub))?
                    .fetch_with_request(Request::new_with_init("https://document.internal/internal/member", &init)?)
                    .await?;
                if response.status_code() != 200 {
                    return Response::error("Account unavailable", 409);
                }
                if self.phase()? != Some(RoomPhase::Live)
                    || self.generation()? != generation
                    || self.text("membership_revision")? != membership_revision
                    || self.text("capability")?.is_none_or(|cap| hash(&cap) != hash(&input.capability))
                {
                    return Response::error("Invitation revoked", 403);
                }
                self.sql.exec(
                    "INSERT INTO members VALUES(?,?) ON CONFLICT(uid) DO UPDATE SET generation=excluded.generation",
                    vec![claims.sub.clone().into(), (self.generation()? as i64).into()],
                )?;
                self.state.storage().sync().await?;
            }
            return Response::from_json(&self.access_state(if owner {
                DocumentRole::Owner
            } else {
                DocumentRole::Editor
            })?);
        }
        if op == Op::Leave {
            let claims = match crate::auth::identity(req, &self.env).await {
                Ok(c) => c,
                Err(e) => return identity_response(e),
            };
            if self.text("owner_uid")?.as_deref() == Some(&claims.sub) {
                return Response::error("Owner must delete document explicitly", 409);
            }
            if !self.release_registration(&claims.sub, Some(self.generation()?)).await? {
                return Response::error("Account unavailable", 503);
            }
            self.sql.exec("DELETE FROM members WHERE uid=?", vec![claims.sub.clone().into()])?;
            let revision = self
                .text("membership_revision")?
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| fail("Membership revision exhausted"))?;
            self.put("membership_revision", &revision.to_string())?;
            self.state.storage().sync().await?;
            for ws in self.state.get_websockets() {
                if ws.deserialize_attachment::<SocketState>()?.is_some_and(|state| state.uid == claims.sub) {
                    let _ = ws.close(Some(1008), Some("Membership removed"));
                }
            }
            return Response::ok("left");
        }
        if op == Op::GetMetadata {
            if !self.authorized_guest(req).await? {
                return Response::error("Denied", 403);
            }
            return Response::from_json(&DocumentMetadata {
                title: self.text("title")?.unwrap_or_else(|| "Untitled".into()),
                template_id: self.text("template_id")?.unwrap_or_default(),
            });
        }
        if op == Op::GetSharing {
            let claims = match crate::auth::identity(req, &self.env).await {
                Ok(claims) => claims,
                Err(error) => return identity_response(error),
            };
            if let Some(role) = self.role_for_uid(&claims.sub)? {
                return Response::from_json(&self.access_state(role)?);
            }
            return Response::error("Denied", 403);
        }
        match self.authorize_creator(req).await? {
            crate::auth::CreatorAuthorization::Allowed => {}
            decision => return creator_response(decision)?.ok_or_else(|| fail("Authorization")),
        }
        if self.phase()? == Some(RoomPhase::Ended) {
            return Response::error("Document deleted", 410);
        }
        if matches!(op, Op::EnableSharing | Op::DisableSharing) {
            match op {
                Op::EnableSharing => {
                    if self.text("capability")?.is_none() {
                        self.put("capability", &random_hex(32)?)?;
                        self.state.storage().sync().await?;
                    }
                }
                _ => {
                    if self.text("capability")?.is_some() {
                        let generation = self
                            .generation()?
                            .checked_add(1)
                            .filter(|v| *v <= i64::MAX as u64)
                            .ok_or_else(|| fail("Access generation exhausted"))?;
                        self.queue_registry_releases()?;
                        self.sql.exec("DELETE FROM members; DELETE FROM resource_sessions; DELETE FROM meta WHERE key='capability'",None)?;
                        self.put("access_generation", &generation.to_string())?;
                        self.schedule_cleanup(5_000).await?;
                        self.state.storage().sync().await?;
                        for ws in self.state.get_websockets() {
                            if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|state| state.owner) {
                                let _ = self.send(
                                    &ws,
                                    Frame::Error {
                                        code: crate::frame::ERROR_ENDED,
                                        message: "Invitation revoked".into(),
                                    },
                                );
                                let _ = ws.close(Some(1008), Some("Invitation revoked"));
                            }
                        }
                    }
                }
            }
            return Response::from_json(&self.access_state(DocumentRole::Owner)?);
        }
        if op == Op::PatchDocument {
            let input: UpdateDocument = match crate::input::body(req, 8192).await {
                Ok(input) => input,
                Err(error) => return error.response(),
            };
            if input.title.len() > 1024 || input.template_id.len() > 256 {
                return Response::error("Invalid metadata", 400);
            }
            // The body read yields; the document may have ended meanwhile. Nothing awaits
            // between this check and the writes below.
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Document deleted", 410);
            }
            self.put("title", &input.title)?;
            self.put("template_id", &input.template_id)?;
            self.state.storage().sync().await?;
            return Response::from_json(&self.access_state(DocumentRole::Owner)?);
        }
        Response::error("Not found", 404)
    }
}
