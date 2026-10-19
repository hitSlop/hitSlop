//! Per-UID reservation registry. No document state or Firebase SDK types live here.
use hitslop_sharing_protocol::http::{Invitation, RoomIdentity, RoomPhase, RoomReservation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Serialize, Deserialize)]
struct Account {
    rooms: BTreeMap<String, RoomReservation>,
}
impl Account {
    fn reserve(
        &mut self,
        document: &str,
        generation: u32,
        room: String,
        capability: String,
    ) -> Result<RoomReservation, u16> {
        if !hitslop_sharing_protocol::valid_copy(document) || generation == 0 {
            return Err(400);
        }
        if let Some(old) = self.rooms.get(document) {
            if old.generation == generation {
                return Ok(old.clone());
            }
            if old.phase != RoomPhase::Ended || old.generation.checked_add(1) != Some(generation) {
                return Err(409);
            }
        } else if generation != 1 {
            return Err(409);
        }
        if self.rooms.values().filter(|r| r.phase != RoomPhase::Ended).count()
            >= hitslop_sharing_protocol::limits::ACCOUNT_ROOMS
        {
            return Err(429);
        }
        let previous_room = self.rooms.get(document).map(|r| r.room.clone());
        let value = RoomReservation {
            document: document.into(),
            generation,
            room,
            capability: Some(capability),
            previous_room,
            phase: RoomPhase::Pending,
            invitation: None,
        };
        self.rooms.insert(document.into(), value.clone());
        Ok(value)
    }
    fn matching(&mut self, id: &RoomIdentity) -> Result<&mut RoomReservation, u16> {
        self.rooms.get_mut(&id.document).filter(|r| r.generation == id.generation && r.room == id.room).ok_or(409)
    }
    fn publish(&mut self, id: &RoomIdentity, invitation: Invitation) -> Result<RoomReservation, u16> {
        let r = self.matching(id)?;
        if !matches!(r.phase, RoomPhase::Pending | RoomPhase::Published) {
            return Err(410);
        }
        if invitation.validate().is_err()
            || invitation.document != r.document
            || invitation.room != r.room
            || r.capability.as_ref() != Some(&invitation.capability)
        {
            return Err(409);
        }
        if let Some(old) = &r.invitation
            && serde_json::to_vec(old).ok() != serde_json::to_vec(&invitation).ok()
        {
            return Err(409);
        }
        r.invitation = Some(invitation);
        r.phase = RoomPhase::Published;
        Ok(r.clone())
    }
    fn stopping(&mut self, id: &RoomIdentity) -> Result<RoomReservation, u16> {
        let r = self.matching(id)?;
        if r.phase != RoomPhase::Ended {
            r.phase = RoomPhase::Stopping;
        }
        // Stop removes the server-held bearer secret before remote cleanup starts.
        r.capability = None;
        r.invitation = None;
        Ok(r.clone())
    }
    fn ended(&mut self, id: &RoomIdentity) -> Result<RoomReservation, u16> {
        let r = self.matching(id)?;
        if !matches!(r.phase, RoomPhase::Stopping | RoomPhase::Ended) {
            return Err(409);
        }
        r.phase = RoomPhase::Ended;
        Ok(r.clone())
    }
}

#[cfg(target_arch = "wasm32")]
mod runtime {
    use super::*;
    use crate::room::{bounded_body, hash, identity_response, random_hex};
    use worker::*;
    #[derive(Deserialize)]
    struct Row {
        value: String,
    }
    #[durable_object]
    pub struct SharingAccount {
        state: State,
        env: Env,
        sql: SqlStorage,
    }
    impl SharingAccount {
        fn load(&self) -> Result<Account> {
            let rows = self.sql.exec("SELECT value FROM account WHERE key='rooms'", None)?.to_array::<Row>()?;
            Ok(rows.first().map(|r| serde_json::from_str(&r.value)).transpose()?.unwrap_or_default())
        }
        fn save(&self, value: &Account) -> Result<()> {
            self.sql.exec(
                "INSERT INTO account VALUES('rooms',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                vec![serde_json::to_string(value)?.into()],
            )?;
            Ok(())
        }
        async fn serve(&self, mut req: Request) -> Result<Response> {
            let claims = match crate::auth::identity(&req, &self.env).await {
                Ok(c) => c,
                Err(e) => return identity_response(e),
            };
            // Verify the UID in the DO itself, as well as in the public router.
            let owner = self.sql.exec("SELECT value FROM account WHERE key='uid'", None)?.to_array::<Row>()?;
            if owner.first().is_some_and(|r| r.value != claims.sub) {
                return Response::error("Denied", 403);
            }
            self.sql.exec("INSERT OR IGNORE INTO account VALUES('uid',?)", vec![claims.sub.into()])?;
            let path = req.path();
            let route = path.trim_start_matches("/v1/account/");
            if let Some(document) = route.strip_prefix("access/") {
                if req.method() != Method::Get || !hitslop_sharing_protocol::valid_copy(document) {
                    return Response::error("Invalid request", 400);
                }
                return Response::from_json(&self.load()?.rooms.get(document));
            }
            if req.method() != Method::Post {
                return Response::error("Method refused", 405);
            }
            let body = bounded_body(&mut req, hitslop_sharing_protocol::CONTROL_BYTES).await?;
            if route == "reserve" {
                let input: hitslop_sharing_protocol::http::ReserveRoom = match serde_json::from_slice(&body) {
                    Ok(v) => v,
                    Err(_) => return Response::error("Invalid request", 400),
                };
                let mut account = self.load()?;
                // Idempotent retries do not consume creation quota or the kill switch.
                let exists = account.rooms.get(&input.document).is_some_and(|r| r.generation == input.generation);
                if !exists {
                    if self.env.var("CREATION_ENABLED").ok().is_none_or(|v| v.to_string() != "1") {
                        return Response::error("Creation unavailable", 503);
                    }
                    if !self
                        .env
                        .rate_limiter("ACCOUNT_CREATE_LIMIT")?
                        .limit(hash(
                            &self.sql.exec("SELECT value FROM account WHERE key='uid'", None)?.one::<Row>()?.value,
                        ))
                        .await?
                        .success
                    {
                        return Response::error("Limited", 429);
                    }
                    // The rate-limiter await allows other requests; reload before mutation.
                    account = self.load()?;
                }
                let result = match account.reserve(&input.document, input.generation, random_hex(16)?, random_hex(32)?)
                {
                    Ok(r) => r,
                    Err(s) => return Response::error("Reservation refused", s),
                };
                self.save(&account)?;
                self.state.storage().sync().await?;
                // A concurrent Stop can have changed this generation while flushing;
                // never return a replacement room to a stale reserve request.
                return match self.load()?.matching(&RoomIdentity {
                    document: result.document,
                    generation: result.generation,
                    room: result.room,
                }) {
                    Ok(current) => Response::from_json(current),
                    Err(status) => Response::error("Stale reservation", status),
                };
            }
            let id: RoomIdentity = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(_) => return Response::error("Invalid request", 400),
            };
            let mut account = self.load()?;
            let current = match account.matching(&id) {
                Ok(r) => r.clone(),
                Err(s) => return Response::error("Stale room", s),
            };
            if !matches!(route, "publish" | "stop") {
                return Response::error("Not found", 404);
            }
            if route == "stop" {
                if current.phase == RoomPhase::Ended {
                    return Response::from_json(&current);
                }
                account.stopping(&id).map_err(|_| Error::RustError("Stale room".into()))?;
                self.save(&account)?;
                self.state.storage().sync().await?;
            } else if !matches!(current.phase, RoomPhase::Pending | RoomPhase::Published) {
                return Response::error("Sharing ended", 410);
            }
            // Forward the original identity, never an asserted UID header. The room
            // independently verifies it and its own immutable owner UID.
            let headers = Headers::new();
            headers.set("Authorization", &req.headers().get("Authorization")?.unwrap_or_default())?;
            headers.set("X-Sharing-Document", &id.document)?;
            let mut init = RequestInit::new();
            init.with_method(if route == "stop" { Method::Delete } else { Method::Post }).with_headers(headers);
            let request =
                Request::new_with_init(&format!("https://account.internal/v1/rooms/{}/{}", id.room, route), &init)?;
            let mut response =
                self.env.durable_object("ROOMS")?.get_by_name(&id.room)?.fetch_with_request(request).await?;
            if response.status_code() != 200 {
                return Ok(response);
            }
            let mut account = self.load()?;
            let result = if route == "stop" {
                account.ended(&id)
            } else {
                let bytes = response.bytes().await?;
                if bytes.len() > hitslop_sharing_protocol::CONTROL_BYTES {
                    return Response::error("Invalid publication", 502);
                }
                // Reload after reading the response too: Stop wins over late publish.
                account = self.load()?;
                account.publish(&id, serde_json::from_slice(&bytes)?)
            };
            if let Err(status) = result {
                return Response::error("Stale or ended room", status);
            }
            self.save(&account)?;
            self.state.storage().sync().await?;
            match self.load()?.matching(&id) {
                Ok(current) => Response::from_json(current),
                Err(status) => Response::error("Stale reservation", status),
            }
        }
    }
    impl DurableObject for SharingAccount {
        fn new(state: State, env: Env) -> Self {
            let sql = state.storage().sql();
            sql.exec("CREATE TABLE IF NOT EXISTS account(key TEXT PRIMARY KEY,value TEXT NOT NULL)", None)
                .expect("account table");
            Self { state, env, sql }
        }
        async fn fetch(&self, req: Request) -> Result<Response> {
            let mut response = self.serve(req).await?;
            let headers = response.headers().clone();
            response = response.with_headers(headers);
            response.headers_mut().set("Cache-Control", "no-store")?;
            Ok(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DOC: &str = "00000000-0000-4000-8000-000000000001";
    fn reserve(a: &mut Account, generation: u32) -> Result<RoomReservation, u16> {
        a.reserve(DOC, generation, "a".repeat(32), "b".repeat(64))
    }
    fn id(r: &RoomReservation) -> RoomIdentity {
        RoomIdentity { document: r.document.clone(), generation: r.generation, room: r.room.clone() }
    }
    fn invitation(r: &RoomReservation) -> Invitation {
        Invitation {
            document: r.document.clone(),
            room: r.room.clone(),
            capability: r.capability.clone().unwrap(),
            origin: hitslop_sharing_protocol::TRUSTED_ORIGIN.into(),
            seed_hash: "c".repeat(64),
            size: 4096,
        }
    }
    #[test]
    fn reservations_survive_retries_and_restarts_and_stop_wins_over_late_publish() {
        let mut a = Account::default();
        let r = reserve(&mut a, 1).unwrap();
        a = serde_json::from_slice(&serde_json::to_vec(&a).unwrap()).unwrap();
        let retry = a.reserve(DOC, 1, "d".repeat(32), "e".repeat(64)).unwrap();
        assert_eq!(retry.room, r.room);
        assert_eq!(retry.capability, r.capability);
        assert_eq!(a.publish(&id(&r), invitation(&r)).unwrap().phase, RoomPhase::Published);
        let stopping = a.stopping(&id(&r)).unwrap();
        assert!(stopping.capability.is_none() && stopping.invitation.is_none());
        assert!(a.publish(&id(&r), invitation(&r)).is_err());
        assert!(reserve(&mut a, 2).is_err());
        a.ended(&id(&r)).unwrap();
        assert_eq!(reserve(&mut a, 1).unwrap().phase, RoomPhase::Ended);
        let next = a.reserve(DOC, 2, "d".repeat(32), "e".repeat(64)).unwrap();
        assert_eq!(next.generation, 2);
        assert!(a.stopping(&id(&r)).is_err());
        assert!(a.ended(&id(&r)).is_err());
        assert!(a.publish(&id(&r), invitation(&r)).is_err());
    }
    #[test]
    fn pending_rooms_count_toward_quota_and_another_account_has_no_access() {
        let mut a = Account::default();
        let r = reserve(&mut a, 1).unwrap();
        for n in 2..=10 {
            a.reserve(&format!("00000000-0000-4000-8000-{n:012}"), 1, format!("{n:032x}"), "b".repeat(64)).unwrap();
        }
        assert!(a.reserve("00000000-0000-4000-8000-000000000011", 1, "f".repeat(32), "b".repeat(64)).is_err());
        assert!(reserve(&mut a, 1).is_ok());
        a.stopping(&id(&r)).unwrap();
        assert!(a.reserve("00000000-0000-4000-8000-000000000011", 1, "f".repeat(32), "b".repeat(64)).is_err());
        a.ended(&id(&r)).unwrap();
        assert!(a.reserve("00000000-0000-4000-8000-000000000011", 1, "f".repeat(32), "b".repeat(64)).is_ok());
        assert!(Account::default().matching(&id(&r)).is_err());
    }
}
