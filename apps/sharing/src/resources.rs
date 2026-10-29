//! Private resource delivery; the relay still never interprets Loro updates.
use super::*;
use crate::resource_types::{self, Manifest, Resource};
use policy::{ResourceInfo, ResourceRoute};
use tinys_sharing_protocol::policy;

#[derive(Deserialize)]
struct SessionRow {
    host: String,
    app: String,
    expires: f64,
}
fn denied() -> Result<Response> {
    Response::error("Resource access denied", 403)
}
fn cookie(req: &Request) -> Option<(String, String)> {
    let text = req.headers().get("Cookie").ok().flatten()?;
    let value = text.split(';').filter_map(|c| c.trim().split_once('=')).find(|(k, _)| *k == "__Host-tiny-resource")?.1;
    let (room, token) = value.split_once('.')?;
    (room.len() == 32 && room.bytes().all(|c| c.is_ascii_hexdigit()) && resource_types::hash_id(token))
        .then(|| (room.into(), token.into()))
}

/// Called before ordinary relay CORS/gate handling. Resource cookies authorize only reads.
pub(super) async fn entry(mut req: Request, env: Env) -> Result<Response> {
    let host = req.url()?.origin().ascii_serialization();
    let trusted = env.var("BROWSER_ORIGIN")?.to_string();
    if !tinys_sharing_protocol::is_authored_origin(&host) {
        return denied();
    }
    let origin = req.headers().get("Origin")?;
    let session = req.path().starts_with("/_resources/");
    if session && origin.as_deref() != Some(&trusted) {
        return denied();
    }
    if req.method() == Method::Options {
        if !session {
            return denied();
        }
        let mut response = Response::empty()?.with_status(204);
        for (k, v) in [
            ("Access-Control-Allow-Origin", trusted.as_str()),
            ("Access-Control-Allow-Credentials", "true"),
            ("Access-Control-Allow-Methods", "POST"),
            ("Access-Control-Allow-Headers", "Authorization, Content-Type"),
        ] {
            response.headers_mut().set(k, v)?;
        }
        return Ok(response);
    }
    let room = if session {
        let parts: Vec<_> = req.path().split('/').map(str::to_owned).collect();
        if parts.len() != 3 || parts[2].len() != 32 {
            return denied();
        }
        parts[2].clone()
    } else {
        if !matches!(req.method(), Method::Get | Method::Head) {
            return denied();
        }
        if origin.as_deref().is_some_and(|o| o != host) {
            return denied();
        }
        match cookie(&req) {
            Some(value) => value.0,
            None => return denied(),
        }
    };
    // A service-bound request keeps its original dottiny URL, including the exact host.
    req = req.clone_mut()?;
    req.headers_mut()?.set("X-Resource-Request", "1")?;
    let mut response = env.durable_object("ROOMS")?.get_by_name(&room)?.fetch_with_request(req).await?;
    let headers = response.headers().clone();
    response = response.with_headers(headers);
    response.headers_mut().set("Cache-Control", "no-store")?;
    response.headers_mut().set("X-Content-Type-Options", "nosniff")?;
    response.headers_mut().set("Cross-Origin-Resource-Policy", "same-origin")?;
    if session {
        response.headers_mut().set("Access-Control-Allow-Origin", &trusted)?;
        response.headers_mut().set("Access-Control-Allow-Credentials", "true")?;
        response.headers_mut().set("Vary", "Origin")?;
    }
    Ok(response)
}

impl SlopRoom {
    pub(super) async fn resource_fetch(&self, mut req: Request) -> Result<Response> {
        if self.phase()? == Some(RoomPhase::Ended) {
            return Response::error("Sharing ended", 410);
        }
        if self.phase()? != Some(RoomPhase::Live) {
            return Response::error("Publication pending", 409);
        }
        let host = req.url()?.origin().ascii_serialization();
        if req.path().starts_with("/_resources/") {
            if req.method() != Method::Post || !self.authorized(&req, false).await? {
                return denied();
            }
            let bytes = bounded_body(&mut req, CONTROL_BYTES).await?;
            let input: tinys_sharing_protocol::http::ResourceGrant = serde_json::from_slice(&bytes)?;
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            let app = self.text("app")?.ok_or_else(|| fail("Missing app"))?;
            if input.app != app || self.text("resources")?.is_none() {
                return denied();
            }
            let mut bytes = [0u8; 32];
            getrandom::getrandom(&mut bytes).map_err(|_| fail("Resource session unavailable"))?;
            let token = hex(&bytes);
            let expires = js_sys::Date::now() + tinys_sharing_protocol::RESOURCE_TTL_MS as f64;
            self.sql.exec("DELETE FROM resource_sessions WHERE expires<=?", vec![js_sys::Date::now().into()])?;
            if self.sql.exec("SELECT count(*) AS n FROM resource_sessions", None)?.one::<NumberRow>()?.n >= 128 {
                return Response::error("Too many resource sessions", 429);
            }
            self.sql.exec(
                "INSERT INTO resource_sessions VALUES(?,?,?,?)",
                vec![hash(&token).into(), host.into(), app.into(), expires.into()],
            )?;
            self.state.storage().sync().await?;
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            let room = req.path().trim_start_matches("/_resources/").to_string();
            let mut response = Response::ok("ready")?;
            response.headers_mut().set("Set-Cookie", &format!("__Host-tiny-resource={room}.{token}; Path=/; Secure; HttpOnly; SameSite=None; Partitioned; Max-Age={}",tinys_sharing_protocol::RESOURCE_TTL_MS/1000))?;
            return Ok(response);
        }
        let Some((_, token)) = cookie(&req) else {
            return denied();
        };
        let rows = self
            .sql
            .exec("SELECT host,app,expires FROM resource_sessions WHERE token=?", vec![hash(&token).into()])?
            .to_array::<SessionRow>()?;
        let Some(row) = rows.first() else {
            return denied();
        };
        if row.host != host || row.expires <= js_sys::Date::now() || self.text("app")?.as_deref() != Some(&row.app) {
            return denied();
        }
        let path = req.path();
        let (item, route, key) = if let Some(key) = path.strip_prefix("/assets/") {
            let manifest: Manifest =
                serde_json::from_str(&self.text("resources")?.ok_or_else(|| fail("Missing resources"))?)?;
            let Some(item) = manifest.resources.into_iter().find(|i| i.key == key) else {
                return Response::error("Not found", 404);
            };
            let key = format!("{}/assets/{}", cookie(&req).unwrap().0, item.hash);
            (item, ResourceRoute::App, key)
        } else if let Some(id) = path.strip_prefix("/attachments/") {
            let Some(item) = self.blobs()?.into_iter().find(|i| i.hash == id) else {
                return Response::error("Not found", 404);
            };
            (item, ResourceRoute::Attachment, format!("{}/blobs/{id}", cookie(&req).unwrap().0))
        } else {
            return Response::error("Not found", 404);
        };
        let info = ResourceInfo { size: item.size, media_type: item.media_type };
        let plan = policy::resource_response(&info, route, req.headers().get("Range")?.as_deref());
        let mut response = if let Some(span) = plan.span.filter(|_| req.method() != Method::Head) {
            let object = self
                .env
                .bucket("SEEDS")?
                .get(key)
                .range(worker::Range::OffsetWithLength { offset: span.offset, length: span.length })
                .execute()
                .await?;
            let Some(object) = object else {
                return Response::error("Resource unavailable", 503);
            };
            Response::from_body(object.body().ok_or_else(|| fail("Missing object body"))?.response_body()?)?
                .with_status(plan.status)
        } else {
            Response::empty()?.with_status(plan.status)
        };
        for (k, v) in plan.headers {
            response.headers_mut().set(&k, &v)?;
        }
        Ok(response)
    }
    pub(super) fn blobs(&self) -> Result<Vec<Resource>> {
        #[derive(Deserialize)]
        struct Row {
            value: String,
        }
        self.sql
            .exec("SELECT value FROM resource_blobs ORDER BY id", None)?
            .to_array::<Row>()?
            .into_iter()
            .map(|r| serde_json::from_str(&r.value).map_err(Into::into))
            .collect()
    }
    pub(super) async fn resource_api(&self, req: &mut Request, room: &str, route: &str) -> Result<Response> {
        if route == "resources" {
            if req.method() == Method::Get {
                return match self.text("resources")? {
                    Some(body) => Response::ok(body).map(|mut r| {
                        r.headers_mut().set("Content-Type", "application/json").expect("static header");
                        r
                    }),
                    None => Response::error("Resources not ready", 409),
                };
            }
            if req.method() != Method::Post {
                return denied();
            }
            let body = bounded_body(req, CONTROL_BYTES).await?;
            let input: Manifest = serde_json::from_slice(&body)?;
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            if resource_types::validate(&input.resources, false).is_err() || !resource_types::hash_id(&input.app) {
                return Response::error("Invalid app inventory", 400);
            }
            if self.text("app")?.as_deref() != Some(&input.app) {
                return denied();
            }
            let text = serde_json::to_string(&input)?;
            self.put("resources", &text)?;
            self.state.storage().sync().await?;
            return Response::ok("ready");
        }
        if route == "blobs" && req.method() == Method::Get {
            return Response::from_json(&self.blobs()?);
        }
        let path = req.path();
        let parts: Vec<_> = path.split('/').collect();
        if parts.len() != 6 || !resource_types::hash_id(parts[5]) {
            return Response::error("Not found", 404);
        }
        let id = parts[5];
        let blob = route == "blob";
        let key = format!("{room}/{}/{id}", if blob { "blobs" } else { "assets" });
        let bucket = self.env.bucket("SEEDS")?;
        if req.method() == Method::Put {
            let item = if blob {
                Resource {
                    key: id.into(),
                    hash: id.into(),
                    size: req
                        .headers()
                        .get("Content-Length")?
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(|| fail("Missing length"))?,
                    media_type: req.headers().get("Content-Type")?.unwrap_or_else(|| "application/octet-stream".into()),
                }
            } else {
                let manifest: Manifest =
                    serde_json::from_str(&self.text("resources")?.ok_or_else(|| fail("Missing manifest"))?)?;
                manifest.resources.into_iter().find(|r| r.hash == id).ok_or_else(|| fail("Unknown app resource"))?
            };
            if resource_types::validate(std::slice::from_ref(&item), blob).is_err() {
                return Response::error("Invalid resource metadata", 400);
            }
            if blob {
                let mut all = self.blobs()?;
                if let Some(old) = all.iter().find(|i| i.hash == id) {
                    return if old.size == item.size && old.media_type == item.media_type {
                        Response::ok("existing")
                    } else {
                        Response::error("Immutable blob conflict", 409)
                    };
                }
                all.push(item.clone());
                if resource_types::validate(&all, true).is_err() {
                    return Response::error("Room file limit reached", 413);
                }
            }
            // Probe-sized files are bounded by Rust limits; R2 verifies the supplied digest.
            let body = bounded_body(req, item.size as usize).await?;
            if body.len() as u64 != item.size || hex(&Sha256::digest(&body)) != id {
                return Response::error("Resource checksum or length mismatch", 422);
            }
            // Reading the body yielded to Stop. Refuse before beginning a new write;
            // if Stop races the R2 write itself, remove that late object afterwards.
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            bucket.put(key, body).sha256(decode_hex(id)?).execute().await?;
            if self.phase()? == Some(RoomPhase::Ended) {
                bucket.delete(format!("{room}/{}/{id}", if blob { "blobs" } else { "assets" })).await?;
                return Response::error("Sharing ended", 410);
            }
            if blob {
                let mut current = self.blobs()?;
                if !current.iter().any(|i| i.hash == id) {
                    current.push(item.clone());
                }
                if resource_types::validate(&current, true).is_err() {
                    bucket.delete(format!("{room}/blobs/{id}")).await?;
                    return Response::error("Room file limit reached", 413);
                }
            }
            if blob {
                self.sql.exec(
                    "INSERT OR IGNORE INTO resource_blobs VALUES(?,?)",
                    vec![id.into(), serde_json::to_string(&item)?.into()],
                )?;
                self.state.storage().sync().await?;
            }
            return Response::ok("stored");
        }
        if req.method() == Method::Get {
            if let Some(object) = bucket.get(key).execute().await? {
                return Response::from_body(object.body().ok_or_else(|| fail("Missing body"))?.response_body()?);
            }
            return Response::error("Not found", 404);
        }
        Response::error("Method refused", 405)
    }
}
