#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RegistryRequest {
    pub document: String,
    pub generation: Option<u64>,
}
/// The account's request that a document DO record its owner, internal to the Worker.
#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct DocumentInit {
    pub owner_uid: String,
    pub document: String,
}
// Per-account registration/quota and deletion coordination; Document DO owns permissions.
#[cfg(target_arch = "wasm32")]
mod runtime {
    use crate::room::{hash, identity_response};
    use pop_sharing_protocol::library::{AccountEntry, LibraryFrame};
    use pop_sharing_protocol::{
        http::CreateRoom,
        routes::{self, Op},
    };
    use serde::{Deserialize, Serialize};
    use worker::*;
    #[derive(Deserialize)]
    struct Registration {
        document: String,
        owned: i64,
    }
    #[derive(Deserialize)]
    struct Number {
        n: i64,
    }
    #[derive(Serialize, Deserialize)]
    struct SocketIdentity {
        expires: u64,
    }
    #[derive(Deserialize)]
    struct Projection {
        payload: String,
    }
    #[durable_object]
    pub struct DocumentAccount {
        state: State,
        env: Env,
        sql: SqlStorage,
        operations: futures_util::lock::Mutex<()>,
    }
    impl DocumentAccount {
        async fn document(
            &self,
            id: &str,
            path: &str,
            method: Method,
            headers: Headers,
            body: Option<String>,
        ) -> Result<Response> {
            let mut init = RequestInit::new();
            init.with_method(method).with_headers(headers);
            if let Some(body) = body {
                init.with_body(Some(body.into()));
            }
            self.env
                .durable_object("DOCUMENTS")?
                .get_by_name(id)?
                .fetch_with_request(Request::new_with_init(&format!("https://account.internal{path}"), &init)?)
                .await
        }
        fn send_frame(&self, ws: &WebSocket, frame: &str) -> Result<()> {
            if ws
                .deserialize_attachment::<SocketIdentity>()?
                .is_none_or(|s| s.expires <= js_sys::Date::now() as u64 / 1000)
            {
                let _ = ws.close(Some(4401), Some("Authentication expired"));
                return Ok(());
            }
            if ws.send_with_str(frame).is_err() {
                let _ = ws.close(Some(1011), Some("Reconnect"));
            }
            Ok(())
        }
        fn broadcast(&self, frame: LibraryFrame) -> Result<()> {
            let text = frame.encode().map_err(|e| Error::RustError(e.to_string()))?;
            for ws in self.state.get_websockets() {
                self.send_frame(&ws, &text)?;
            }
            Ok(())
        }
        async fn describe(&self, input: crate::room::publication::Description) -> Result<Response> {
            if input.entry.validate().is_err() {
                return Response::error("Invalid entry", 400);
            }
            if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                return Response::error("Account deleted", 410);
            }
            let owned = i64::from(input.generation.is_none());
            let registered = self
                .sql
                .exec(
                    "SELECT count(*) AS n FROM documents WHERE document=? AND owned=? AND (owned=1 OR generation=?)",
                    vec![
                        input.entry.document.clone().into(),
                        owned.into(),
                        (input.generation.unwrap_or(0) as i64).into(),
                    ],
                )?
                .one::<Number>()?
                .n
                > 0;
            if !registered {
                return Response::ok("obsolete");
            }
            let prior = self
                .sql
                .exec("SELECT payload FROM projections WHERE document=?", vec![input.entry.document.clone().into()])?
                .to_array::<Projection>()?;
            if let Some(prior) = prior.first() {
                let old: AccountEntry = serde_json::from_str(&prior.payload)?;
                if old.revision >= input.entry.revision {
                    // A previous call may have committed before broadcasting or replying.
                    self.broadcast(LibraryFrame::Upsert { entry: old })?;
                    return Response::ok("current");
                }
            }
            self.sql.exec(
                "INSERT INTO projections VALUES(?,?) ON CONFLICT(document) DO UPDATE SET payload=excluded.payload",
                vec![input.entry.document.clone().into(), serde_json::to_string(&input.entry)?.into()],
            )?;
            self.state.storage().sync().await?;
            // A storage await may interleave release or a newer description.
            let current = self
                .sql
                .exec(
                    "SELECT payload FROM projections JOIN documents USING(document) WHERE document=?",
                    vec![input.entry.document.clone().into()],
                )?
                .to_array::<Projection>()?;
            if current.first().is_some_and(|row| {
                serde_json::from_str::<AccountEntry>(&row.payload).ok().as_ref() == Some(&input.entry)
            }) {
                self.broadcast(LibraryFrame::Upsert { entry: input.entry })?;
            }
            Response::ok("described")
        }
        async fn bootstrap_library(&self, uid: &str) -> Result<()> {
            use futures_util::{StreamExt, stream};
            let missing = self.sql.exec("SELECT document,owned FROM documents LEFT JOIN projections USING(document) WHERE projections.document IS NULL", None)?.to_array::<Registration>()?;
            let replies = stream::iter(missing.into_iter().map(|row| async move {
                self.document(
                    &row.document,
                    &format!("/internal/documents/{}/library", row.document),
                    Method::Post,
                    Headers::new(),
                    Some(serde_json::to_string(&serde_json::json!({"uid":uid}))?),
                )
                .await
            }))
            .buffer_unordered(4)
            .collect::<Vec<_>>()
            .await;
            for reply in replies {
                if let Ok(mut reply) = reply
                    && reply.status_code() == 200
                {
                    let input: crate::room::publication::Description = reply.json().await?;
                    self.describe(input).await?;
                }
            }
            Ok(())
        }
        async fn serve(&self, mut req: Request) -> Result<Response> {
            if req.path() == "/internal/describe" && req.method() == Method::Post {
                let input: crate::room::publication::Description =
                    match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                        Ok(v) => v,
                        Err(e) => return e.response(),
                    };
                return self.describe(input).await;
            }
            if req.path() == "/internal/release" && req.method() == Method::Post {
                let input: super::RegistryRequest =
                    match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                        Ok(input) => input,
                        Err(error) => return error.response(),
                    };
                if let Some(generation) = input.generation {
                    self.sql.exec(
                        "DELETE FROM documents WHERE document=? AND owned=0 AND generation<=?",
                        vec![input.document.clone().into(), (generation as i64).into()],
                    )?;
                } else {
                    self.sql.exec(
                        "DELETE FROM documents WHERE document=? AND owned=1",
                        vec![input.document.clone().into()],
                    )?;
                }
                let absent = self
                    .sql
                    .exec("SELECT count(*) AS n FROM documents WHERE document=?", vec![input.document.clone().into()])?
                    .one::<Number>()?
                    .n
                    == 0;
                if absent {
                    self.sql.exec("DELETE FROM projections WHERE document=?", vec![input.document.clone().into()])?;
                }
                self.state.storage().sync().await?;
                if absent
                    && self
                        .sql
                        .exec(
                            "SELECT count(*) AS n FROM documents WHERE document=?",
                            vec![input.document.clone().into()],
                        )?
                        .one::<Number>()?
                        .n
                        == 0
                {
                    self.broadcast(LibraryFrame::Remove { document: input.document })?;
                }
                return Response::ok("released");
            }
            if req.path() == "/internal/member" && req.method() == Method::Post {
                let input: super::RegistryRequest =
                    match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                        Ok(input) => input,
                        Err(error) => return error.response(),
                    };
                if !pop_sharing_protocol::valid_copy(&input.document) || input.generation.is_none() {
                    return Response::error("Invalid document", 400);
                }
                if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                    return Response::error("Account deletion pending", 409);
                }
                self.sql.exec("INSERT INTO documents VALUES(?,0,?) ON CONFLICT(document) DO UPDATE SET generation=max(documents.generation,excluded.generation) WHERE documents.owned=0",vec![input.document.into(),(input.generation.unwrap_or(0) as i64).into()])?;
                self.state.storage().sync().await?;
                return Response::ok("registered");
            }
            let path = req.path();
            let Some(matched) = routes::find(req.method().as_ref(), &path) else {
                return Response::error("Not found", 404);
            };
            let op = matched.route.op;
            let claims = match crate::auth::identity(&req, &self.env).await {
                Ok(c) => c,
                Err(e) => return identity_response(e),
            };
            if self.state.id().to_string()
                != self.env.durable_object("ACCOUNTS")?.id_from_name(&hash(&claims.sub))?.to_string()
            {
                return Response::error("Denied", 403);
            }
            // The `/internal/*` calls above stay outside this lock: a document holding its
            // membership lock awaits them while account deletion, holding this one, leaves it.
            let _guard = self.operations.lock().await;
            if op == Op::DeleteAccount {
                self.sql.exec("INSERT OR IGNORE INTO deleting VALUES(1)", None)?;
                for ws in self.state.get_websockets() {
                    let _ = ws.close(Some(1008), Some("Account deleted"));
                }
                self.state.storage().sync().await?;
                for row in self.sql.exec("SELECT document,owned FROM documents", None)?.to_array::<Registration>()? {
                    let path = if row.owned != 0 {
                        format!("/v1/documents/{}", row.document)
                    } else {
                        format!("/v1/documents/{}/members/me", row.document)
                    };
                    let response =
                        self.document(&row.document, &path, Method::Delete, req.headers().clone(), None).await?;
                    if !matches!(response.status_code(), 200 | 403 | 404 | 410) {
                        return Ok(response);
                    }
                    self.sql.exec("DELETE FROM projections WHERE document=?", vec![row.document.clone().into()])?;
                    self.sql.exec("DELETE FROM documents WHERE document=?", vec![row.document.into()])?;
                }
                // The fence stays durable. A new Firebase UID is a new account.
                self.state.storage().sync().await?;
                return Response::ok("deleted");
            }
            if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                return Response::error("Account deleted", 410);
            }
            if op == Op::AccountLibrary {
                if req.headers().get("Upgrade")?.as_deref() != Some("websocket") {
                    return Response::error("WebSocket required", 426);
                }
                let sockets = self.state.get_websockets();
                let active = sockets
                    .iter()
                    .filter(|ws| {
                        ws.deserialize_attachment::<SocketIdentity>()
                            .ok()
                            .flatten()
                            .is_some_and(|s| s.expires > js_sys::Date::now() as u64 / 1000)
                    })
                    .count();
                if active >= pop_sharing_protocol::limits::ACCOUNT_CONNECTIONS {
                    return Response::error("Account connection limit", 429);
                }
                for ws in sockets {
                    if ws
                        .deserialize_attachment::<SocketIdentity>()?
                        .is_none_or(|s| s.expires <= js_sys::Date::now() as u64 / 1000)
                    {
                        let _ = ws.close(Some(4401), Some("Authentication expired"));
                    }
                }
                self.bootstrap_library(&claims.sub).await?;
                let entries = self
                    .sql
                    .exec("SELECT payload FROM projections JOIN documents USING(document) ORDER BY document", None)?
                    .to_array::<Projection>()?
                    .into_iter()
                    .map(|row| serde_json::from_str::<AccountEntry>(&row.payload))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let pages = pop_sharing_protocol::library::snapshot_pages(entries)
                    .map_err(|e| Error::RustError(e.to_string()))?;
                let pair = WebSocketPair::new()?;
                pair.server.serialize_attachment(SocketIdentity { expires: claims.exp })?;
                self.state.accept_web_socket(&pair.server);
                for page in pages {
                    self.send_frame(&pair.server, &page)?;
                }
                return Response::from_websocket(pair.client);
            }
            if op != Op::CreateDocument {
                return Response::error("Not found", 404);
            }
            if !claims.may_create() {
                return Response::error("Sign in to create a cloud document", 401);
            }
            let input: CreateRoom = match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                Ok(input) => input,
                Err(error) => return error.response(),
            };
            if !pop_sharing_protocol::valid_copy(&input.document) {
                return Response::error("Invalid document", 400);
            }
            let known = self
                .sql
                .exec(
                    "SELECT count(*) AS n FROM documents WHERE document=? AND owned=1",
                    vec![input.document.clone().into()],
                )?
                .one::<Number>()?
                .n
                > 0;
            if !known {
                if self.env.var("CREATION_ENABLED").ok().is_none_or(|v| v.to_string() != "1") {
                    return Response::error("Creation unavailable", 503);
                }
                if !self.env.rate_limiter("ACCOUNT_CREATE_LIMIT")?.limit(hash(&claims.sub)).await?.success {
                    return Response::error("Limited", 429);
                }
            }
            let init = super::DocumentInit { owner_uid: claims.sub, document: input.document.clone() };
            let result = self
                .document(
                    &input.document,
                    &format!("/internal/documents/{}/init", input.document),
                    Method::Post,
                    Headers::new(),
                    Some(serde_json::to_string(&init)?),
                )
                .await?;
            if result.status_code() < 400 {
                self.sql.exec(
                    "INSERT INTO documents VALUES(?,1,0) ON CONFLICT(document) DO UPDATE SET owned=1",
                    vec![input.document.into()],
                )?;
                self.state.storage().sync().await?;
            }
            Ok(result)
        }
    }
    impl DurableObject for DocumentAccount {
        fn new(state: State, env: Env) -> Self {
            state.set_websocket_auto_response(
                &worker::worker_sys::WebSocketRequestResponsePair::new("ping", "pong").expect("valid heartbeat"),
            );
            let sql = state.storage().sql();
            sql.exec("CREATE TABLE IF NOT EXISTS projections(document TEXT PRIMARY KEY,payload TEXT NOT NULL)", None)
                .expect("library projections");
            sql.exec("CREATE TABLE IF NOT EXISTS documents(document TEXT PRIMARY KEY,owned INTEGER NOT NULL,generation INTEGER NOT NULL);CREATE TABLE IF NOT EXISTS deleting(id INTEGER PRIMARY KEY)",None).expect("account registry");
            Self { state, env, sql, operations: futures_util::lock::Mutex::new(()) }
        }
        async fn websocket_message(&self, ws: WebSocket, _: WebSocketIncomingMessage) -> Result<()> {
            let _ = ws.close(Some(1008), Some("Only heartbeat messages accepted"));
            Ok(())
        }
        async fn websocket_close(&self, ws: WebSocket, code: usize, _: String, _: bool) -> Result<()> {
            let _ = ws.close(Some(code as u16), Some("Closed"));
            Ok(())
        }
        async fn websocket_error(&self, ws: WebSocket, _: worker::Error) -> Result<()> {
            let _ = ws.close(Some(1011), Some("Socket failed"));
            Ok(())
        }
        async fn fetch(&self, req: Request) -> Result<Response> {
            let response = self.serve(req).await?;
            let headers = response.headers().clone();
            let mut response = response.with_headers(headers);
            response.headers_mut().set("Cache-Control", "no-store")?;
            Ok(response)
        }
    }
}
