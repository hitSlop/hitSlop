//! Seed and attachment blobs: immutable, content-addressed files in R2, inventoried in `resource_blobs`.
use crate::room::{CloudDocument, NumberRow, decode_hex, fail, object_key};

use crate::refusal::Refusal;

use pop_contracts::hex;
use pop_sharing_protocol::http::RoomPhase;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use worker::*;

use pop_sharing_protocol::resources::{self, Resource};

impl CloudDocument {
    /// Uploads register their R2 key here before writing it. Caller holds uploads, so
    /// no blob write can repopulate a deleted object.
    pub(crate) async fn sweep_garbage(&self, room: &str) -> Result<()> {
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
    pub(crate) fn blobs(&self) -> Result<Vec<Resource>> {
        self.blob_inventory(self.phase()? == Some(RoomPhase::Live))
    }
    pub(crate) fn blob_inventory(&self, attachments_only: bool) -> Result<Vec<Resource>> {
        #[derive(Deserialize)]
        struct Row {
            value: String,
        }
        self.sql
            .exec(
                if attachments_only {
                    "SELECT value FROM resource_blobs WHERE attachment=1 ORDER BY id"
                } else {
                    "SELECT value FROM resource_blobs ORDER BY id"
                },
                None,
            )?
            .to_array::<Row>()?
            .into_iter()
            .map(|r| serde_json::from_str(&r.value).map_err(Into::into))
            .collect()
    }
    pub(crate) async fn put_blob(
        &self,
        req: &mut Request,
        room: &str,
        hash: Option<&str>,
        uid: &str,
    ) -> Result<Response> {
        let id = match hash {
            Some(id) if pop_contracts::wire::valid_attachment_id(id) => id,
            _ => return Refusal::new(404, "not_found", "Not found").response(),
        };
        let key = object_key(room, &format!("blobs/{id}"));
        let bucket = self.env.bucket("SEEDS")?;

        // Serialize blob uploads with garbage sweeps; Stop/freeze remain independent.
        let _upload = self.uploads.lock().await;
        self.sweep_garbage(room).await?;
        if self.role_for_uid(uid)?.is_none() {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        let Some(size) = req.headers().get("Content-Length")?.and_then(|value| value.parse().ok()) else {
            return Refusal::new(411, "unavailable", "Content length required").response();
        };
        let item = Resource {
            key: id.into(),
            hash: id.into(),
            size,
            media_type: req.headers().get("Content-Type")?.unwrap_or_else(|| "application/octet-stream".into()),
        };
        if resources::validate_blobs(std::slice::from_ref(&item)).is_err() {
            return Refusal::new(400, "invalid_request", "Invalid resource metadata").response();
        }
        let live = self.phase()? == Some(RoomPhase::Live);
        let mut all = self.blobs()?;
        if let Some(old) = self.blob_inventory(false)?.iter().find(|i| i.hash == id) {
            return if old.size == item.size && old.media_type == item.media_type {
                Response::ok("existing")
            } else {
                Refusal::new(409, "conflict", "Immutable blob conflict").response()
            };
        }
        all.push(item.clone());
        if live && resources::validate(&all).is_err() {
            return Refusal::new(413, "invalid_request", "Room file limit reached").response();
        }
        // Probe-sized files are bounded by Rust limits; R2 verifies the supplied digest.
        let Ok(limit) = usize::try_from(item.size) else {
            return Refusal::new(413, "invalid_request", "Resource exceeds its byte limit").response();
        };
        let body = match crate::input::bytes(req, limit).await {
            Ok(bytes) => bytes,
            Err(error) => return error.response(),
        };
        if body.len() as u64 != item.size || hex(&Sha256::digest(&body)) != id {
            return Refusal::new(422, "invalid_request", "Resource checksum or length mismatch").response();
        }
        // Reading the body yielded to Stop. Refuse before beginning a new write;
        // if Stop races the R2 write itself, remove that late object afterwards.
        if self.phase()? == Some(RoomPhase::Ended) || self.role_for_uid(uid)?.is_none() {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        // Register bounded cleanup before R2: a crash after PUT must not
        // leave bytes outside both quota inventory and garbage tracking.
        self.sql.exec("INSERT OR IGNORE INTO garbage VALUES(?)", vec![key.clone().into()])?;
        self.schedule_cleanup(60_000).await?;
        self.state.storage().sync().await?;
        bucket.put(key.clone(), body).sha256(decode_hex(id)?).execute().await?;
        if self.phase()? == Some(RoomPhase::Ended) || self.role_for_uid(uid)?.is_none() {
            bucket.delete(key).await?;
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        let mut current = self.blobs()?;
        if !current.iter().any(|i| i.hash == id) {
            current.push(item.clone());
        }
        if live && resources::validate(&current).is_err() {
            bucket.delete(key).await?;
            return Refusal::new(413, "invalid_request", "Room file limit reached").response();
        }
        self.sql.exec(
            "INSERT OR IGNORE INTO resource_blobs VALUES(?,?,?)",
            vec![id.into(), serde_json::to_string(&item)?.into(), i64::from(live).into()],
        )?;
        self.sql.exec("DELETE FROM garbage WHERE key=?", vec![key.into()])?;
        self.state.storage().sync().await?;
        Response::ok("stored")
    }
    pub(crate) async fn read_blob(&self, room: &str, hash: Option<&str>, uid: &str) -> Result<Response> {
        let id = match hash {
            Some(id) if pop_contracts::wire::valid_attachment_id(id) => id,
            _ => return Refusal::new(404, "not_found", "Not found").response(),
        };
        let key = object_key(room, &format!("blobs/{id}"));
        let bucket = self.env.bucket("SEEDS")?;

        if !self.blobs()?.iter().any(|item| item.hash == id) {
            return Refusal::new(404, "not_found", "Not found").response();
        }
        if let Some(object) = bucket.get(key).execute().await? {
            if self.role_for_uid(uid)?.is_none() {
                return Refusal::new(403, "denied", "Access revoked").response();
            }
            if self.phase()? == Some(RoomPhase::Ended) {
                return Refusal::new(410, "sharing_ended", "Sharing ended").response();
            }
            if !self.blobs()?.iter().any(|item| item.hash == id) {
                return Refusal::new(404, "not_found", "Not found").response();
            }
            return Response::from_body(object.body().ok_or_else(|| fail("Missing body"))?.response_body()?);
        }
        Refusal::new(404, "not_found", "Not found").response()
    }
}
