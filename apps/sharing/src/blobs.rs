//! Attachment blobs: immutable, content-addressed files in R2, inventoried in `resource_blobs`.
use super::*;
use pop_sharing_protocol::resources::{self, Resource};

impl CloudDocument {
    /// Uploads register their R2 key here before writing it. Caller holds uploads, so
    /// no blob write can repopulate a deleted object.
    pub(super) async fn sweep_garbage(&self, room: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct KeyRow {
            key: String,
        }
        let bucket = self.env.bucket("SEEDS")?;
        for entry in self.sql.exec("SELECT key FROM garbage", None)?.to_array::<KeyRow>()? {
            let id = entry.key.strip_prefix(&object_key(room, "blobs/")).unwrap_or_default();
            let retained = self
                .sql
                .exec("SELECT EXISTS(SELECT 1 FROM resource_blobs WHERE id=?) AS n", vec![id.into()])?
                .one::<NumberRow>()?
                .n
                != 0;
            if !retained && let Err(error) = bucket.delete(&entry.key).await {
                console_error!("Blob cleanup failed: {}", error);
                continue;
            }
            self.sql.exec("DELETE FROM garbage WHERE key=?", vec![entry.key.into()])?;
        }
        Ok(())
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
    pub(super) async fn blob_api(&self, req: &mut Request, room: &str, op: Op, hash: Option<&str>) -> Result<Response> {
        if op == Op::ListBlobs {
            return Response::from_json(&self.blobs()?);
        }
        let id = match hash {
            Some(id) if pop_contracts::wire::valid_attachment_id(id) => id,
            _ => return Response::error("Not found", 404),
        };
        let key = object_key(room, &format!("blobs/{id}"));
        let bucket = self.env.bucket("SEEDS")?;
        if op == Op::PutBlob {
            // Serialize blob uploads with garbage sweeps; Stop/freeze remain independent.
            let _upload = self.uploads.lock().await;
            self.sweep_garbage(room).await?;
            if !self.authorized_guest(req).await? {
                return Response::error("Access revoked", 403);
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Response::error("Sharing ended", 410);
            }
            let Some(size) = req.headers().get("Content-Length")?.and_then(|value| value.parse().ok()) else {
                return Response::error("Content length required", 411);
            };
            let item = Resource {
                key: id.into(),
                hash: id.into(),
                size,
                media_type: req.headers().get("Content-Type")?.unwrap_or_else(|| "application/octet-stream".into()),
            };
            if resources::validate(std::slice::from_ref(&item)).is_err() {
                return Response::error("Invalid resource metadata", 400);
            }
            let mut all = self.blobs()?;
            if let Some(old) = all.iter().find(|i| i.hash == id) {
                return if old.size == item.size && old.media_type == item.media_type {
                    Response::ok("existing")
                } else {
                    Response::error("Immutable blob conflict", 409)
                };
            }
            all.push(item.clone());
            if resources::validate(&all).is_err() {
                return Response::error("Room file limit reached", 413);
            }
            // Probe-sized files are bounded by Rust limits; R2 verifies the supplied digest.
            let Ok(limit) = usize::try_from(item.size) else {
                return Response::error("Resource exceeds its byte limit", 413);
            };
            let body = match crate::input::bytes(req, limit).await {
                Ok(bytes) => bytes,
                Err(error) => return error.response(),
            };
            if body.len() as u64 != item.size || hex(&Sha256::digest(&body)) != id {
                return Response::error("Resource checksum or length mismatch", 422);
            }
            // Reading the body yielded to Stop. Refuse before beginning a new write;
            // if Stop races the R2 write itself, remove that late object afterwards.
            if self.phase()? == Some(RoomPhase::Ended) || !self.authorized_guest(req).await? {
                return Response::error("Access revoked", 403);
            }
            // Register bounded cleanup before R2: a crash after PUT must not
            // leave bytes outside both quota inventory and garbage tracking.
            self.sql.exec("INSERT OR IGNORE INTO garbage VALUES(?)", vec![key.clone().into()])?;
            self.schedule_cleanup(60_000).await?;
            self.state.storage().sync().await?;
            bucket.put(key.clone(), body).sha256(decode_hex(id)?).execute().await?;
            if self.phase()? == Some(RoomPhase::Ended) || !self.authorized_guest(req).await? {
                bucket.delete(key).await?;
                return Response::error("Sharing ended", 410);
            }
            let mut current = self.blobs()?;
            if !current.iter().any(|i| i.hash == id) {
                current.push(item.clone());
            }
            if resources::validate(&current).is_err() {
                bucket.delete(key).await?;
                return Response::error("Room file limit reached", 413);
            }
            self.sql.exec(
                "INSERT OR IGNORE INTO resource_blobs VALUES(?,?)",
                vec![id.into(), serde_json::to_string(&item)?.into()],
            )?;
            self.sql.exec("DELETE FROM garbage WHERE key=?", vec![key.into()])?;
            self.state.storage().sync().await?;
            return Response::ok("stored");
        }
        if !self.blobs()?.iter().any(|item| item.hash == id) {
            return Response::error("Not found", 404);
        }
        if let Some(object) = bucket.get(key).execute().await? {
            if self.phase()? == Some(RoomPhase::Ended) || !self.authorized_guest(req).await? {
                return Response::error("Sharing ended", 410);
            }
            if !self.blobs()?.iter().any(|item| item.hash == id) {
                return Response::error("Not found", 404);
            }
            return Response::from_body(object.body().ok_or_else(|| fail("Missing body"))?.response_body()?);
        }
        Response::error("Not found", 404)
    }
}
