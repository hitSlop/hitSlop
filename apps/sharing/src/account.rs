/// The account's request that a document DO record its owner, internal to the Worker.
#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct DocumentInit {
    pub owner_uid: String,
    pub document: String,
}
// Per-account discovery and deletion coordination; Document DO owns permissions.
#[cfg(target_arch = "wasm32")]
mod runtime {
    use crate::room::hash;
    use crate::routes::{self, Op};
    use crate::{auth::VerifiedIdentity, bounded::bounded, limits::OBJECT_TIMEOUT, refusal::Refusal};
    use pop_sharing_protocol::http::CreateRoom;
    use pop_sharing_protocol::library::{AccountEntry, LibraryFrame};
    use serde::{Deserialize, Serialize};
    use worker::*;
    #[derive(Deserialize)]
    struct Registration {
        document: String,
        owned: i64,
        initializing: i64,
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
    pub struct AccountLibrary {
        state: State,
        env: Env,
        sql: SqlStorage,
        operations: futures_util::lock::Mutex<()>,
    }
    impl AccountLibrary {
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
        async fn cleanup_entry(&self, document: &str, owned: bool, uid: &str) -> Result<Response> {
            let identity = VerifiedIdentity { uid: uid.into(), expires: 0 };
            let mut headers = Headers::new();
            identity.forward(&mut headers)?;
            let action = if owned { "delete" } else { "leave" };
            self.document(document, &format!("/internal/documents/{document}/{action}"), Method::Delete, headers, None)
                .await
        }
        async fn apply_entry(&self, input: crate::publication::ApplyEntry) -> Result<Response> {
            if !pop_contracts::identity::valid_document_id(&input.document)
                || input.recipient.is_empty()
                || input.revision == 0
                || input.revision > i64::MAX as u64
                || input.entry.as_ref().is_some_and(|e| {
                    e.document != input.document || e.revision != input.revision || e.validate().is_err()
                })
            {
                return Refusal::new(400, "invalid_request", "Invalid entry").response();
            }
            #[derive(Deserialize)]
            struct Revision {
                revision: u64,
            }
            let prior = self
                .sql
                .exec("SELECT revision FROM entries WHERE document=?", vec![input.document.clone().into()])?
                .to_array::<Revision>()?
                .pop();
            if crate::policy::apply_library_revision(prior.as_ref().map(|r| r.revision), input.revision) {
                let payload = input
                    .entry
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?
                    .map(Into::into)
                    .unwrap_or(SqlStorageValue::Null);
                let owned = i64::from(
                    input.entry.as_ref().is_some_and(|e| e.role == pop_sharing_protocol::document::DocumentRole::Owner),
                );
                // NULL payloads retain only the high-water mark. Calls can arrive arbitrarily
                // late, so these marks live as long as the account (UUIDs are never reused).
                self.sql.exec("INSERT INTO entries(document,revision,payload,visible,owned,initializing) VALUES(?,?,?,?,?,0) ON CONFLICT(document) DO UPDATE SET revision=excluded.revision,payload=excluded.payload,visible=excluded.visible,owned=excluded.owned,initializing=0",
                    vec![input.document.clone().into(),(input.revision as i64).into(),payload,i64::from(input.visible).into(),owned.into()])?;
            }
            self.state.storage().sync().await?;
            // Deletion may fence the account while an entry is still in flight. Its
            // outbox retries until this direct authority call has removed that membership.
            if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                if let Some(entry) = input.entry {
                    let reply = self
                        .cleanup_entry(
                            &input.document,
                            entry.role == pop_sharing_protocol::document::DocumentRole::Owner,
                            &input.recipient,
                        )
                        .await?;
                    if !matches!(reply.status_code(), 200 | 403 | 404 | 410) {
                        return Ok(reply);
                    }
                }
                self.sql.exec(
                    "UPDATE entries SET payload=NULL,visible=0,initializing=0 WHERE document=?",
                    vec![input.document.clone().into()],
                )?;
                self.state.storage().sync().await?;
            } else {
                let current = self
                    .sql
                    .exec(
                        "SELECT payload FROM entries WHERE document=? AND visible=1 AND payload IS NOT NULL",
                        vec![input.document.clone().into()],
                    )?
                    .to_array::<Projection>()?
                    .pop();
                let frame = match current {
                    Some(row) => LibraryFrame::Upsert { entry: serde_json::from_str(&row.payload)? },
                    None => LibraryFrame::Remove { document: input.document.clone() },
                };
                self.broadcast(frame)?;
            }
            let current = self
                .sql
                .exec("SELECT revision FROM entries WHERE document=?", vec![input.document.into()])?
                .one::<Revision>()?;
            Response::from_json(&current.revision)
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

            if op == Op::ApplyEntry {
                let input = match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                    Ok(input) => input,
                    Err(e) => return e.response(),
                };
                return self.apply_entry(input).await;
            }
            let identity = VerifiedIdentity::from_request(&req)?;
            let _guard = self.operations.lock().await;
            if op == Op::DeleteAccount {
                self.sql.exec("INSERT OR IGNORE INTO deleting VALUES(1)", None)?;
                for ws in self.state.get_websockets() {
                    let _ = ws.close(Some(1008), Some("Account deleted"));
                }
                self.state.storage().sync().await?;
                for row in self
                    .sql
                    .exec(
                        "SELECT document,owned,initializing FROM entries WHERE payload IS NOT NULL OR initializing=1",
                        None,
                    )?
                    .to_array::<Registration>()?
                {
                    // A reservation survives an uncertain init reply. Retry init before
                    // deletion so a delayed initial request cannot create an orphan.
                    if row.initializing != 0 {
                        let reply = self.initialize_owned(&row.document, &identity.uid).await?;
                        if !matches!(reply.status_code(), 200..=299 | 403 | 410) {
                            return Ok(reply);
                        }
                    }
                    let reply = self.cleanup_entry(&row.document, row.owned != 0, &identity.uid).await?;
                    if !matches!(reply.status_code(), 200 | 403 | 404 | 410) {
                        return Ok(reply);
                    }
                    self.sql.exec(
                        "UPDATE entries SET payload=NULL,visible=0,initializing=0 WHERE document=?",
                        vec![row.document.into()],
                    )?;
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
                    .exec(
                        "SELECT payload FROM entries WHERE visible=1 AND payload IS NOT NULL ORDER BY document",
                        None,
                    )?
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
                    "SELECT count(*) AS n FROM entries WHERE document=? AND owned=1 AND (payload IS NOT NULL OR initializing=1)",
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
            self.sql.exec("INSERT INTO entries(document,revision,payload,visible,owned,initializing) VALUES(?,0,NULL,0,1,1) ON CONFLICT(document) DO UPDATE SET initializing=1",vec![input.document.clone().into()])?;
            self.state.storage().sync().await?;
            let result = self.initialize_owned(&input.document, &identity.uid).await?;
            if matches!(result.status_code(), 403 | 410) {
                self.sql
                    .exec("UPDATE entries SET initializing=0 WHERE document=?", vec![input.document.clone().into()])?;
                self.sql.exec(
                    "DELETE FROM entries WHERE document=? AND revision=0 AND payload IS NULL",
                    vec![input.document.into()],
                )?;
            }
            self.state.storage().sync().await?;
            Ok(result)
        }
    }
    impl DurableObject for AccountLibrary {
        fn new(state: State, env: Env) -> Self {
            state.set_websocket_auto_response(
                &worker::worker_sys::WebSocketRequestResponsePair::new("ping", "pong").expect("valid heartbeat"),
            );
            let sql = state.storage().sql();
            sql.exec("CREATE TABLE IF NOT EXISTS entries(document TEXT PRIMARY KEY,revision INTEGER NOT NULL CHECK(revision>=0),payload TEXT,visible INTEGER NOT NULL,owned INTEGER NOT NULL,initializing INTEGER NOT NULL);CREATE TABLE IF NOT EXISTS deleting(id INTEGER PRIMARY KEY)",None).expect("account entries");
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
