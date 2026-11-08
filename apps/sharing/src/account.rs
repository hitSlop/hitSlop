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
    use pop_sharing_protocol::{
        http::CreateRoom,
        routes::{self, Op},
    };
    use serde::Deserialize;
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
        async fn serve(&self, mut req: Request) -> Result<Response> {
            if req.path() == "/internal/release" && req.method() == Method::Post {
                let input: super::RegistryRequest =
                    match crate::input::body(&mut req, pop_sharing_protocol::CONTROL_BYTES).await {
                        Ok(input) => input,
                        Err(error) => return error.response(),
                    };
                if let Some(generation) = input.generation {
                    self.sql.exec(
                        "DELETE FROM documents WHERE document=? AND owned=0 AND generation<=?",
                        vec![input.document.into(), (generation as i64).into()],
                    )?;
                } else {
                    self.sql.exec("DELETE FROM documents WHERE document=? AND owned=1", vec![input.document.into()])?;
                }
                self.state.storage().sync().await?;
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
                    self.sql.exec("DELETE FROM documents WHERE document=?", vec![row.document.into()])?;
                }
                // The fence stays durable. A new Firebase UID is a new account.
                self.state.storage().sync().await?;
                return Response::ok("deleted");
            }
            if self.sql.exec("SELECT count(*) AS n FROM deleting", None)?.one::<Number>()?.n > 0 {
                return Response::error("Account deleted", 410);
            }
            if op == Op::ListAccountDocuments {
                use pop_sharing_protocol::document::{DocumentRole, RegisteredDocument, RegisteredDocuments};
                // Keyset pages: every UUID sorts after the empty first cursor.
                let after = match req.url()?.query_pairs().find(|(key, _)| key == "after") {
                    Some((_, after)) if pop_sharing_protocol::valid_copy(&after) => after.into_owned(),
                    Some(_) => return Response::error("Invalid cursor", 400),
                    None => String::new(),
                };
                let limit = pop_sharing_protocol::limits::ACCOUNT_DOCUMENTS_PAGE as i64 + 1;
                let rows = self
                    .sql
                    .exec(
                        "SELECT document,owned FROM documents WHERE document>? ORDER BY document LIMIT ?",
                        vec![after.into(), limit.into()],
                    )?
                    .to_array::<Registration>()?
                    .into_iter()
                    .map(|row| RegisteredDocument {
                        document: row.document,
                        role: if row.owned != 0 { DocumentRole::Owner } else { DocumentRole::Editor },
                    })
                    .collect();
                return Response::from_json(&RegisteredDocuments::page(rows));
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
            let was_member = self
                .sql
                .exec(
                    "SELECT count(*) AS n FROM documents WHERE document=? AND owned=0",
                    vec![input.document.clone().into()],
                )?
                .one::<Number>()?
                .n
                > 0;
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
                self.sql.exec(
                    "INSERT INTO documents VALUES(?,1,0) ON CONFLICT(document) DO UPDATE SET owned=1",
                    vec![input.document.clone().into()],
                )?;
                self.state.storage().sync().await?;
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
            if matches!(result.status_code(), 403 | 410) {
                if was_member {
                    self.sql.exec("UPDATE documents SET owned=0 WHERE document=?", vec![input.document.into()])?;
                } else {
                    self.sql.exec("DELETE FROM documents WHERE document=?", vec![input.document.into()])?;
                }
                self.state.storage().sync().await?;
            }
            Ok(result)
        }
    }
    impl DurableObject for DocumentAccount {
        fn new(state: State, env: Env) -> Self {
            let sql = state.storage().sql();
            sql.exec("CREATE TABLE IF NOT EXISTS documents(document TEXT PRIMARY KEY,owned INTEGER NOT NULL,generation INTEGER NOT NULL);CREATE TABLE IF NOT EXISTS deleting(id INTEGER PRIMARY KEY)",None).expect("account registry");
            Self { state, env, sql, operations: futures_util::lock::Mutex::new(()) }
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
