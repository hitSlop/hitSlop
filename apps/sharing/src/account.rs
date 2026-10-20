//! Per-UID index and quota. Room phases and capabilities belong only to the Room DO.
#[cfg(target_arch = "wasm32")]
mod runtime {
    use crate::room::{bounded_body, hash, identity_response, random_hex};
    use hitslop_sharing_protocol::http::{AccountRoom, CreateRoom, RoomInit};
    use serde::Deserialize;
    use worker::*;

    #[derive(Deserialize)]
    struct RoomRow {
        room: String,
    }
    #[derive(Deserialize)]
    struct CountRow {
        n: usize,
    }
    #[durable_object]
    pub struct SharingAccount {
        state: State,
        env: Env,
        sql: SqlStorage,
    }
    impl SharingAccount {
        fn current(&self, document: &str) -> Result<Option<String>> {
            Ok(self
                .sql
                .exec("SELECT room FROM rooms WHERE document=?", vec![document.into()])?
                .to_array::<RoomRow>()?
                .first()
                .map(|r| r.room.clone()))
        }
        fn active(&self, room: &str) -> Result<bool> {
            Ok(self.sql.exec("SELECT count(*) AS n FROM live WHERE room=?", vec![room.into()])?.one::<CountRow>()?.n
                > 0)
        }
        async fn init_room(&self, room: &str, owner_uid: &str, document: &str) -> Result<Response> {
            let mut init = RequestInit::new();
            init.with_method(Method::Post).with_body(Some(
                serde_json::to_string(&RoomInit { owner_uid: owner_uid.into(), document: document.into() })?.into(),
            ));
            self.env
                .durable_object("ROOMS")?
                .get_by_name(room)?
                .fetch_with_request(Request::new_with_init(
                    &format!("https://account.internal/internal/rooms/{room}/init"),
                    &init,
                )?)
                .await
        }
        async fn serve(&self, mut req: Request) -> Result<Response> {
            // Only the room uses this path; the public router never forwards it.
            if req.path() == "/internal/release" && req.method() == Method::Post {
                let room = req.text().await?;
                self.sql.exec("DELETE FROM live WHERE room=?", vec![room.into()])?;
                self.state.storage().sync().await?;
                return Response::ok("released");
            }
            let claims = match crate::auth::identity(&req, &self.env).await {
                Ok(c) => c,
                Err(e) => return identity_response(e),
            };
            if self.state.id().to_string()
                != self.env.durable_object("ACCOUNTS")?.id_from_name(&hash(&claims.sub))?.to_string()
            {
                return Response::error("Denied", 403);
            }
            if let Some(document) = req.path().strip_prefix("/v1/account/rooms/") {
                if req.method() != Method::Get || !hitslop_sharing_protocol::valid_copy(document) {
                    return Response::error("Invalid request", 400);
                }
                return Response::from_json(&self.current(document)?.map(|room| AccountRoom { room }));
            }
            if req.path() != "/v1/rooms" || req.method() != Method::Post {
                return Response::error("Not found", 404);
            }
            let body = bounded_body(&mut req, hitslop_sharing_protocol::CONTROL_BYTES).await?;
            let input: CreateRoom = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(_) => return Response::error("Invalid request", 400),
            };
            if !hitslop_sharing_protocol::valid_copy(&input.document) {
                return Response::error("Invalid document", 400);
            }
            // Claim the index before awaiting Room init. Retried or concurrent requests
            // initialize the same room, and never consume a second quota slot.
            if let Some(room) = self.current(&input.document)?
                && self.active(&room)?
            {
                let response = self.init_room(&room, &claims.sub, &input.document).await?;
                if response.status_code() != 410 {
                    return Ok(response);
                }
                self.sql.exec("DELETE FROM live WHERE room=?", vec![room.into()])?;
            }
            if self.env.var("CREATION_ENABLED").ok().is_none_or(|v| v.to_string() != "1") {
                return Response::error("Creation unavailable", 503);
            }
            if !self.env.rate_limiter("ACCOUNT_CREATE_LIMIT")?.limit(hash(&claims.sub)).await?.success {
                return Response::error("Limited", 429);
            }
            // The limiter yielded: another create may now own this document's slot.
            let room = if let Some(room) = self.current(&input.document)?
                && self.active(&room)?
            {
                room
            } else {
                let n = self.sql.exec("SELECT count(*) AS n FROM live", None)?.one::<CountRow>()?.n;
                if n >= hitslop_sharing_protocol::limits::ACCOUNT_ROOMS {
                    return Response::error("Room quota reached", 429);
                }
                let room = random_hex(16)?;
                self.sql.exec(
                    "INSERT INTO rooms VALUES(?,?) ON CONFLICT(document) DO UPDATE SET room=excluded.room",
                    vec![input.document.clone().into(), room.clone().into()],
                )?;
                self.sql.exec("INSERT INTO live VALUES(?)", vec![room.clone().into()])?;
                room
            };
            self.init_room(&room, &claims.sub, &input.document).await
        }
    }
    impl DurableObject for SharingAccount {
        fn new(state: State, env: Env) -> Self {
            let sql = state.storage().sql();
            sql.exec("CREATE TABLE IF NOT EXISTS rooms(document TEXT PRIMARY KEY,room TEXT NOT NULL); CREATE TABLE IF NOT EXISTS live(room TEXT PRIMARY KEY)", None).expect("account index");
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
