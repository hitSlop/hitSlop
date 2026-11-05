//! Conditional persistent checkpoint publication, with opaque bytes retained in R2.
use super::*;
use pop_sharing_protocol::lifecycle::{EpochState, PublishEpoch, StagedSeed};

#[derive(Deserialize)]
struct SeedRow {
    hash: String,
    size: u32,
    expires: u64,
}
#[derive(Deserialize)]
struct KeyRow {
    key: String,
}
fn get(sql: &SqlStorage, key: &str) -> Result<Option<String>> {
    Ok(sql
        .exec("SELECT value FROM meta WHERE key=?", vec![key.into()])?
        .to_array::<TextRow>()?
        .into_iter()
        .next()
        .map(|r| r.value))
}
fn put(sql: &SqlStorage, key: &str, value: &str) -> Result<()> {
    sql.exec(
        "INSERT INTO meta VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        vec![key.into(), value.into()],
    )?;
    Ok(())
}
impl CloudDocument {
    pub(super) fn epoch_state(&self) -> Result<EpochState> {
        Ok(EpochState {
            seed_hash: self.text("seed_hash")?.ok_or_else(|| fail("Seed not ready"))?,
            root: serde_json::from_str(&self.text("root")?.unwrap_or_else(|| "[]".into()))?,
            heads: wire_version(&self.heads()?),
        })
    }
    fn seed_key(&self, room: &str) -> Result<String> {
        Ok(self.text("seed_key")?.unwrap_or_else(|| format!("{room}/seed")))
    }
    pub(super) async fn read_seed(&self, req: &Request, room: &str) -> Result<Response> {
        let wanted = req.url()?.query_pairs().find(|(k, _)| k == "hash").map(|(_, v)| v.into_owned());
        let current = self.text("seed_hash")?;
        if wanted.is_some() && wanted != current {
            return Response::error("Checkpoint changed", 409);
        }
        let key = self.seed_key(room)?;
        let object = self.env.bucket("SEEDS")?.get(key).execute().await?;
        if self.phase()? == Some(RoomPhase::Ended) || !self.authorized_guest(req).await? {
            return Response::error("Sharing ended", 410);
        }
        if self.text("seed_hash")? != current {
            return Response::error("Checkpoint changed", 409);
        }
        match object {
            Some(object) => {
                Response::from_body(object.body().ok_or_else(|| fail("Missing seed body"))?.response_body()?)
            }
            None => Response::error("Seed missing", 404),
        }
    }
    /// Caller holds uploads, so no blob/seed write can repopulate a deleted object.
    pub(super) async fn sweep_epochs(&self, room: &str) -> Result<()> {
        let bucket = self.env.bucket("SEEDS")?;
        let expired = self
            .sql
            .exec("SELECT hash,size,expires FROM epoch_seeds WHERE expires<=?", vec![(now() as i64).into()])?
            .to_array::<SeedRow>()?;
        for seed in expired {
            if self.text("seed_hash")?.as_deref() != Some(seed.hash.as_str()) {
                bucket.delete(format!("{room}/epochs/{}", seed.hash)).await?;
            }
            self.sql.exec(
                "INSERT OR IGNORE INTO epoch_garbage SELECT ? || id FROM epoch_blobs WHERE seed=?",
                vec![format!("{room}/blobs/").into(), seed.hash.clone().into()],
            )?;
            self.sql.exec("DELETE FROM epoch_blobs WHERE seed=?", vec![seed.hash.clone().into()])?;
            self.sql.exec("DELETE FROM epoch_seeds WHERE hash=?", vec![seed.hash.into()])?;
        }
        let garbage = self.sql.exec("SELECT key FROM epoch_garbage", None)?.to_array::<KeyRow>()?;
        for entry in garbage {
            if entry.key != self.seed_key(room)? && !self.retained_epoch_blob(room, &entry.key)? {
                bucket.delete(&entry.key).await?;
            }
            self.sql.exec("DELETE FROM epoch_garbage WHERE key=?", vec![entry.key.into()])?;
        }
        Ok(())
    }
    pub(super) fn epoch_alarm_delay(&self, otherwise: u64) -> Result<u64> {
        let next = self
            .sql
            .exec("SELECT min(expires) AS n FROM epoch_seeds HAVING count(*)>0", None)?
            .to_array::<NumberRow>()?
            .into_iter()
            .next();
        Ok(next.map_or(otherwise, |row| otherwise.min((row.n as u64).saturating_sub(now()).max(1))))
    }
    pub(super) async fn stage_epoch(&self, req: &mut Request, room: &str, hash: &str) -> Result<Response> {
        let _guard = self.uploads.lock().await;
        if !self.authorized_guest(req).await? {
            return Response::error("Access revoked", 403);
        }
        if !crate::resource_types::hash_id(hash) {
            return Response::error("Invalid seed hash", 400);
        }
        if self.phase()? != Some(RoomPhase::Live) {
            return Response::error("Checkpoint unavailable", 409);
        }
        self.sweep_epochs(room).await?;
        let epoch = self.text("seed_hash")?;
        let bytes = match bounded_body(req, SEED_BYTES).await {
            Ok(bytes) => bytes,
            Err(worker::Error::RustError(message)) if message == "Request exceeds its byte limit" => {
                return Response::error(message, 413);
            }
            Err(error) => return Err(error),
        };
        if bytes.len() < 100 || hex(&Sha256::digest(&bytes)) != hash {
            return Response::error("Seed checksum mismatch", 422);
        }
        if !self.authorized_guest(req).await?
            || self.phase()? != Some(RoomPhase::Live)
            || self.text("seed_hash")? != epoch
        {
            return Response::error("Checkpoint changed", 409);
        }
        if epoch.as_deref() == Some(hash) {
            return Response::from_json(&StagedSeed {
                seed_hash: hash.into(),
                size: bytes.len() as u32,
                expires_at: now() + EPOCH_STAGE_TTL_MS,
            });
        }
        let existing = self
            .sql
            .exec("SELECT hash,size,expires FROM epoch_seeds WHERE hash=?", vec![hash.into()])?
            .to_array::<SeedRow>()?
            .into_iter()
            .next();
        if existing.is_none()
            && self.sql.exec("SELECT count(*) AS n FROM epoch_seeds", None)?.one::<NumberRow>()?.n as usize
                >= EPOCH_STAGE_COUNT
        {
            return Response::error("Staging slots full", 409);
        }
        let expires = existing.map_or(now() + EPOCH_STAGE_TTL_MS, |row| row.expires);
        let size = bytes.len() as u32;
        // Reserve cleanup before R2 IO: interrupted uploads cannot become untracked orphans.
        self.sql.exec(
            "INSERT OR IGNORE INTO epoch_seeds VALUES(?,?,?)",
            vec![hash.into(), (size as i64).into(), (expires as i64).into()],
        )?;
        let alarm = self.state.storage().get_alarm().await?;
        if alarm.is_none_or(|at| at as u64 > expires) {
            self.state
                .storage()
                .set_alarm(std::time::Duration::from_millis(expires.saturating_sub(now()).max(1)))
                .await?;
        }
        self.state.storage().sync().await?;
        let key = format!("{room}/epochs/{hash}");
        self.env.bucket("SEEDS")?.put(key.clone(), bytes).sha256(decode_hex(hash)?).execute().await?;
        if !self.authorized_guest(req).await?
            || self.phase()? != Some(RoomPhase::Live)
            || self.text("seed_hash")? != epoch
        {
            self.env.bucket("SEEDS")?.delete(key).await?;
            self.sql.exec("DELETE FROM epoch_seeds WHERE hash=?", vec![hash.into()])?;
            return Response::error("Checkpoint changed", 409);
        }
        Response::from_json(&StagedSeed { seed_hash: hash.into(), size, expires_at: expires })
    }
    fn retained_epoch_blob(&self, room: &str, key: &str) -> Result<bool> {
        let Some(id) = key.strip_prefix(&format!("{room}/blobs/")) else { return Ok(false) };
        let row = self.sql.exec(
            "SELECT (EXISTS(SELECT 1 FROM resource_blobs WHERE id=?) OR EXISTS(SELECT 1 FROM epoch_blobs b JOIN epoch_seeds s ON s.hash=b.seed WHERE b.id=? AND s.expires>?)) AS n",
            vec![id.into(), id.into(), (now() as i64).into()])?.one::<NumberRow>()?;
        Ok(row.n != 0)
    }

    fn staged_blobs(&self, seed: &str) -> Result<Vec<crate::resource_types::Resource>> {
        self.sql
            .exec("SELECT value FROM epoch_blobs WHERE seed=? ORDER BY id", vec![seed.into()])?
            .to_array::<TextRow>()?
            .into_iter()
            .map(|row| serde_json::from_str(&row.value).map_err(Into::into))
            .collect()
    }

    /// Fresh candidates have their own finite lease. Current room occupancy cannot
    /// prevent uploading the bounded replacement that will reclaim old attachments.
    pub(super) async fn stage_epoch_blob(
        &self,
        req: &mut Request,
        room: &str,
        seed: &str,
        id: &str,
    ) -> Result<Response> {
        use crate::resource_types::{self, Resource};
        let _guard = self.uploads.lock().await;
        if !self.authorized_guest(req).await? {
            return Response::error("Access revoked", 403);
        }
        if !resource_types::hash_id(seed) || !resource_types::hash_id(id) {
            return Response::error("Invalid hash", 400);
        }
        if self.phase()? != Some(RoomPhase::Live) {
            return Response::error("Checkpoint unavailable", 409);
        }
        self.sweep_epochs(room).await?;
        let epoch = self.text("seed_hash")?;
        let stage = self
            .sql
            .exec("SELECT hash,size,expires FROM epoch_seeds WHERE hash=?", vec![seed.into()])?
            .to_array::<SeedRow>()?
            .into_iter()
            .next();
        let Some(stage) = stage.filter(|row| row.expires > now()) else {
            return Response::error("Stage missing or expired", 409);
        };
        let size = match req.headers().get("Content-Length")?.and_then(|s| s.parse::<u64>().ok()) {
            Some(size) => size,
            None => return Response::error("Content length required", 411),
        };
        let item = Resource {
            key: id.into(),
            hash: id.into(),
            size,
            media_type: req.headers().get("Content-Type")?.unwrap_or_else(|| "application/octet-stream".into()),
        };
        if resource_types::validate(std::slice::from_ref(&item), true).is_err() {
            return Response::error("Invalid resource metadata", 400);
        }
        let mut staged = self.staged_blobs(seed)?;
        let current_blobs = self.blobs()?;
        if let Some(old) = staged.iter().chain(current_blobs.iter()).find(|old| old.hash == id)
            && (old.size != item.size || old.media_type != item.media_type)
        {
            return Response::error("Immutable blob conflict", 409);
        }
        if !staged.iter().any(|old| old.hash == id) {
            staged.push(item.clone());
        }
        if resource_types::validate(&staged, true).is_err()
            || staged.iter().map(|item| item.size).sum::<u64>() > EPOCH_STAGE_BLOB_BYTES as u64
        {
            return Response::error("Staged file limit reached", 413);
        }
        let body = bounded_body(req, item.size as usize).await?;
        if body.len() as u64 != size || hex(&Sha256::digest(&body)) != id {
            return Response::error("Resource checksum or length mismatch", 422);
        }
        if self.phase()? != Some(RoomPhase::Live) || self.text("seed_hash")? != epoch || stage.expires <= now() {
            return Response::error("Checkpoint changed", 409);
        }
        // The stage's alarm already bounds this registration. Persist it before R2
        // IO so a crash cannot leave an untracked attachment outside either inventory.
        self.sql.exec(
            "INSERT OR IGNORE INTO epoch_blobs VALUES(?,?,?)",
            vec![seed.into(), id.into(), serde_json::to_string(&item)?.into()],
        )?;
        self.state.storage().sync().await?;
        let key = format!("{room}/blobs/{id}");
        self.env.bucket("SEEDS")?.put(key.clone(), body).sha256(decode_hex(id)?).execute().await?;
        if self.phase()? != Some(RoomPhase::Live) || self.text("seed_hash")? != epoch || stage.expires <= now() {
            // Retain the registration until its bounded stage sweep: another current
            // epoch or unexpired candidate may reference this same immutable hash.
            self.sql.exec("INSERT OR IGNORE INTO epoch_garbage VALUES(?)", vec![key.into()])?;
            return Response::error("Checkpoint changed", 409);
        }
        Response::ok("staged")
    }

    /// Explicit candidate release is safe after a conflict or lost response: the
    /// current seed is never eligible, and blob cleanup respects every other lease.
    pub(super) async fn release_epoch(&self, req: &Request, room: &str, hash: &str) -> Result<Response> {
        let _guard = self.uploads.lock().await;
        if !self.authorized_guest(req).await? {
            return Response::error("Access revoked", 403);
        }
        if !crate::resource_types::hash_id(hash) {
            return Response::error("Invalid seed hash", 400);
        }
        if self.text("seed_hash")?.as_deref() == Some(hash) {
            return Response::error("Checkpoint is current", 409);
        }
        self.sql.exec("UPDATE epoch_seeds SET expires=0 WHERE hash=?", vec![hash.into()])?;
        self.state.storage().sync().await?;
        self.sweep_epochs(room).await?;
        Response::ok("released")
    }

    pub(super) async fn publish_epoch(&self, req: &mut Request, room: &str) -> Result<Response> {
        let bytes = bounded_body(req, CONTROL_BYTES).await?;
        let Ok(input) = serde_json::from_slice::<PublishEpoch>(&bytes) else {
            return Response::error("Invalid publication", 400);
        };
        if input.validate().is_err() {
            return Response::error("Invalid publication", 400);
        }
        let _guard = self.uploads.lock().await;
        if self.phase()? != Some(RoomPhase::Live) {
            return Response::error("Checkpoint unavailable", 409);
        }
        let current = self.epoch_state()?;
        if current.seed_hash == input.seed_hash && current.root == input.root {
            return Response::from_json(&current);
        }
        if current.seed_hash != input.expected.seed_hash || current.heads != input.expected.heads {
            return Response::error("Checkpoint conflict", 409);
        }
        if self.text("app")?.as_deref() != Some(input.resources.app.as_str()) {
            return Response::error("Different app", 409);
        }
        let staged = self
            .sql
            .exec("SELECT hash,size,expires FROM epoch_seeds WHERE hash=?", vec![input.seed_hash.clone().into()])?
            .to_array::<SeedRow>()?
            .into_iter()
            .next();
        let Some(staged) = staged.filter(|row| row.expires > now()) else {
            return Response::error("Stage missing or expired", 409);
        };
        let bucket = self.env.bucket("SEEDS")?;
        let seed_key = format!("{room}/epochs/{}", input.seed_hash);
        if bucket.head(&seed_key).await?.is_none_or(|o| o.size() != staged.size as u64) {
            return Response::error("Stage missing", 409);
        }
        let existing = self.blobs()?;
        let staged_blobs = self.staged_blobs(&input.seed_hash)?;
        for item in &input.attachments {
            if !existing
                .iter()
                .chain(staged_blobs.iter())
                .any(|old| old.hash == item.hash && old.size == item.size && old.media_type == item.media_type)
                || bucket.head(format!("{room}/blobs/{}", item.hash)).await?.is_none_or(|o| o.size() != item.size)
            {
                return Response::error("Attachment missing", 409);
            }
        }
        for item in &input.resources.resources {
            if bucket.head(format!("{room}/assets/{}", item.hash)).await?.is_none_or(|o| o.size() != item.size) {
                return Response::error("App resource missing", 409);
            }
        }
        if !self.authorized_guest(req).await? {
            return Response::error("Access revoked", 403);
        }
        let previous_seed = self.seed_key(room)?;
        let garbage: Vec<String> = existing
            .iter()
            .filter(|old| !input.attachments.iter().any(|next| next.hash == old.hash))
            .map(|old| format!("{room}/blobs/{}", old.hash))
            .chain(staged_blobs.iter().map(|item| format!("{room}/blobs/{}", item.hash)))
            .chain(std::iter::once(previous_seed))
            .collect();
        let sql = self.sql.clone();
        let committed = std::rc::Rc::new(std::cell::Cell::new(false));
        let changed = committed.clone();
        self.transaction(move || {
            let heads: Vec<(String, u32)> = serde_json::from_str(&get(&sql, "heads")?.unwrap_or_else(|| "[]".into()))?;
            if get(&sql, "phase")?.as_deref() != Some("\"live\"")
                || get(&sql, "seed_hash")?.as_deref() != Some(input.expected.seed_hash.as_str())
                || heads != input.expected.heads
                || staged.expires <= now()
            {
                return Ok(());
            }
            sql.exec(
                "DELETE FROM records; DELETE FROM chunks; DELETE FROM resource_sessions; DELETE FROM resource_blobs",
                None,
            )?;
            for item in &input.attachments {
                sql.exec(
                    "INSERT INTO resource_blobs VALUES(?,?)",
                    vec![item.hash.clone().into(), serde_json::to_string(item)?.into()],
                )?;
            }
            for key in &garbage {
                sql.exec("INSERT OR IGNORE INTO epoch_garbage VALUES(?)", vec![key.clone().into()])?;
            }
            for (key, value) in [
                ("seed_hash", input.seed_hash.clone()),
                ("seed_key", seed_key.clone()),
                ("seed_size", staged.size.to_string()),
                ("root", serde_json::to_string(&input.root)?),
                ("heads", serde_json::to_string(&input.root)?),
                ("resources", serde_json::to_string(&input.resources)?),
                ("epoch_required", "1".into()),
            ] {
                put(&sql, key, &value)?;
            }
            sql.exec("DELETE FROM meta WHERE key='base_version'", None)?;
            sql.exec("DELETE FROM epoch_blobs WHERE seed=?", vec![input.seed_hash.clone().into()])?;
            sql.exec("DELETE FROM epoch_seeds WHERE hash=?", vec![input.seed_hash.clone().into()])?;
            changed.set(true);
            Ok(())
        })?;
        if !committed.get() {
            return Response::error("Checkpoint conflict", 409);
        }
        self.state.storage().sync().await?;
        self.end_connections(crate::frame::ERROR_EPOCH, "Checkpoint changed", 1012);
        self.sweep_epochs(room).await?;
        Response::from_json(&self.epoch_state()?)
    }
    pub(super) fn end_connections(&self, code: u8, message: &str, close: u16) {
        for ws in self.state.get_websockets() {
            let _ = ws.serialize_attachment(SocketState::default());
            let _ = self.send(&ws, Frame::Error { code, message: message.into() });
            let _ = ws.close(Some(close), Some(message));
        }
    }
}
