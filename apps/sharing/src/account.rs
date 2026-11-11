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
    use crate::room::hash;
    use crate::routes::{self, Op};
    use crate::{
        auth::VerifiedIdentity,
        bounded::bounded,
        limits::{LIBRARY_TIMEOUT, OBJECT_TIMEOUT},
        refusal::Refusal,
    };
    use pop_sharing_protocol::http::CreateRoom;
    use pop_sharing_protocol::library::{AccountEntry, LibraryFrame};
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
            let stub = self.env.durable_object("DOCUMENTS")?.get_by_name(id)?;
            bounded(
                stub.fetch_with_request(Request::new_with_init(&format!("https://account.internal{path}"), &init)?),
                OBJECT_TIMEOUT,
            )
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
        async fn describe(&self, input: crate::publication::Description) -> Result<Response> {
            if input.entry.validate().is_err() {
                return Refusal::new(400, "invalid_request", "Invalid entry").response();
            }
            if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                return Refusal::new(410, "sharing_ended", "Account deleted").response();
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
        async fn initialize_owned(&self, document: &str, uid: &str) -> Result<Response> {
            self.document(
                document,
                &format!("/internal/documents/{document}/init"),
                Method::Post,
                Headers::new(),
                Some(serde_json::to_string(&super::DocumentInit { owner_uid: uid.into(), document: document.into() })?),
            )
            .await
        }
        fn finish_creation(&self, document: &str) -> Result<()> {
            // Deletion owns pending intents once fenced; do not erase its recovery work.
            if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n != 0 {
                return Ok(());
            }
            self.sql.exec(
                "INSERT INTO documents VALUES(?,1,0) ON CONFLICT(document) DO UPDATE SET owned=1",
                vec![document.into()],
            )?;
            self.sql.exec("DELETE FROM creation_intents WHERE document=?", vec![document.into()])?;
            Ok(())
        }
        async fn bootstrap_library(&self, identity: &VerifiedIdentity) -> Result<()> {
            use futures_util::{StreamExt, stream};
            let pending =
                self.sql.exec("SELECT document,1 AS owned FROM creation_intents", None)?.to_array::<Registration>()?;
            for row in pending {
                let reply = self.initialize_owned(&row.document, &identity.uid).await?;
                match reply.status_code() {
                    200..=299 => self.finish_creation(&row.document)?,
                    403 | 410 => {
                        self.sql.exec("DELETE FROM creation_intents WHERE document=?", vec![row.document.into()])?;
                    }
                    _ => return Err(Error::RustError("Creation recovery unavailable".into())),
                }
            }
            let missing = self.sql.exec("SELECT document,owned FROM documents LEFT JOIN projections USING(document) WHERE projections.document IS NULL", None)?.to_array::<Registration>()?;
            let mut replies = stream::iter(missing.into_iter().map(|row| async move {
                let mut headers = Headers::new();
                identity.forward(&mut headers)?;
                self.document(
                    &row.document,
                    &format!("/internal/documents/{}/library", row.document),
                    Method::Post,
                    headers,
                    None,
                )
                .await
            }))
            .buffer_unordered(4);
            while let Some(reply) = replies.next().await {
                let mut reply = reply?;
                match reply.status_code() {
                    200 => {
                        let input: crate::publication::Description = reply.json().await?;
                        self.describe(input).await?;
                    }
                    // The authority explicitly reports a hidden upload or lost access.
                    403 | 404 | 409 | 410 => {}
                    _ => return Err(Error::RustError("Library lookup unavailable".into())),
                }
            }
            Ok(())
        }
        async fn serve(&self, mut req: Request) -> Result<Response> {
            let Some((route, _, _)) = routes::resolve(&req)? else {
                return Refusal::new(404, "not_found", "Not found").response();
            };
            if route.scope != routes::Scope::Account {
                return Refusal::new(404, "not_found", "Not found").response();
            }
            let op = route.op;
            if let Err(refusal) =
                routes::authorize(route, routes::Caller { internal: route.internal, role: None }, None)
            {
                return refusal.response();
            }

            if op == Op::Describe {
                let input: crate::publication::Description =
                    match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                        Ok(v) => v,
                        Err(e) => return e.response(),
                    };
                return self.describe(input).await;
            }
            if op == Op::Release {
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
            if op == Op::Register {
                let input: super::RegistryRequest =
                    match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                        Ok(input) => input,
                        Err(error) => return error.response(),
                    };
                if !pop_contracts::identity::valid_document_id(&input.document) || input.generation.is_none() {
                    return Refusal::new(400, "invalid_request", "Invalid document").response();
                }
                if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                    return Refusal::new(409, "conflict", "Account deletion pending").response();
                }
                self.sql.exec("INSERT INTO documents VALUES(?,0,?) ON CONFLICT(document) DO UPDATE SET generation=max(documents.generation,excluded.generation) WHERE documents.owned=0",vec![input.document.into(),(input.generation.unwrap_or(0) as i64).into()])?;
                self.state.storage().sync().await?;
                return Response::ok("registered");
            }
            let identity = VerifiedIdentity::from_request(&req)?;
            // The `/internal/*` calls above stay outside this lock: a document holding its
            // membership lock awaits them while account deletion, holding this one, leaves it.
            if op == Op::AccountLibrary && bounded(self.bootstrap_library(&identity), LIBRARY_TIMEOUT).await.is_err() {
                return Refusal::new(503, "unavailable", "The sharing service refused this request").response();
            }

            let _guard = self.operations.lock().await;
            if op == Op::DeleteAccount {
                self.sql.exec("INSERT OR IGNORE INTO deleting VALUES(1)", None)?;
                for ws in self.state.get_websockets() {
                    let _ = ws.close(Some(1008), Some("Account deleted"));
                }
                self.state.storage().sync().await?;
                for row in self.sql.exec("SELECT document,max(owned) AS owned FROM (SELECT document,owned FROM documents UNION ALL SELECT document,1 AS owned FROM creation_intents) GROUP BY document", None)?.to_array::<Registration>()? {
                    if self.sql.exec("SELECT count(*) AS n FROM creation_intents WHERE document=?", vec![row.document.clone().into()])?.one::<Number>()?.n > 0 {
                        let reply = self.initialize_owned(&row.document, &identity.uid).await?;
                        if !matches!(reply.status_code(), 200..=299 | 403 | 410) { return Ok(reply); }
                    }
                    let path = if row.owned != 0 {
                        format!("/internal/documents/{}/delete", row.document)
                    } else {
                        format!("/internal/documents/{}/leave", row.document)
                    };
                    let mut headers = Headers::new();
                    identity.forward(&mut headers)?;
                    let response = self.document(&row.document, &path, Method::Delete, headers, None).await?;
                    if !matches!(response.status_code(), 200 | 403 | 404 | 410) {
                        return Ok(response);
                    }
                    self.sql.exec("DELETE FROM creation_intents WHERE document=?", vec![row.document.clone().into()])?;
                    self.sql.exec("DELETE FROM projections WHERE document=?", vec![row.document.clone().into()])?;
                    self.sql.exec("DELETE FROM documents WHERE document=?", vec![row.document.into()])?;
                }
                // The fence stays durable. A new Firebase UID is a new account.
                self.state.storage().sync().await?;
                return Response::ok("deleted");
            }
            if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                return Refusal::new(410, "sharing_ended", "Account deleted").response();
            }
            if op == Op::AccountLibrary {
                if req.headers().get("Upgrade")?.as_deref() != Some("websocket") {
                    return Refusal::new(426, "unavailable", "WebSocket required").response();
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
                if active >= crate::limits::ACCOUNT_CONNECTIONS {
                    return Refusal::new(429, "limited", "Account connection limit").response();
                }
                for ws in sockets {
                    if ws
                        .deserialize_attachment::<SocketIdentity>()?
                        .is_none_or(|s| s.expires <= js_sys::Date::now() as u64 / 1000)
                    {
                        let _ = ws.close(Some(4401), Some("Authentication expired"));
                    }
                }
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
                pair.server.serialize_attachment(SocketIdentity { expires: identity.expires })?;
                self.state.accept_web_socket(&pair.server);
                for page in pages {
                    self.send_frame(&pair.server, &page)?;
                }
                return Response::from_websocket(pair.client);
            }
            if op != Op::CreateDocument {
                return Refusal::new(404, "not_found", "Not found").response();
            }
            let input: CreateRoom = match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                Ok(input) => input,
                Err(error) => return error.response(),
            };
            if !pop_contracts::identity::valid_document_id(&input.document) {
                return Refusal::new(400, "invalid_request", "Invalid document").response();
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
                    return Refusal::new(503, "unavailable", "The sharing service refused this request").response();
                }
                if !self.env.rate_limiter("ACCOUNT_CREATE_LIMIT")?.limit(hash(&identity.uid)).await?.success {
                    return Refusal::new(429, "limited", "Limited").response();
                }
            }
            self.sql.exec("INSERT OR IGNORE INTO creation_intents VALUES(?)", vec![input.document.clone().into()])?;
            self.state.storage().sync().await?;
            let result = self.initialize_owned(&input.document, &identity.uid).await?;
            match result.status_code() {
                200..=299 => self.finish_creation(&input.document)?,
                403 | 410 => {
                    self.sql.exec("DELETE FROM creation_intents WHERE document=?", vec![input.document.into()])?;
                }
                _ => {}
            }
            self.state.storage().sync().await?;
            Ok(result)
        }
    }
    impl DurableObject for DocumentAccount {
        fn new(state: State, env: Env) -> Self {
            state.set_websocket_auto_response(
                &worker::worker_sys::WebSocketRequestResponsePair::new("ping", "pong").expect("valid heartbeat"),
            );
            let sql = state.storage().sql();
            sql.exec("CREATE TABLE IF NOT EXISTS creation_intents(document TEXT PRIMARY KEY)", None)
                .expect("creation intents");
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
