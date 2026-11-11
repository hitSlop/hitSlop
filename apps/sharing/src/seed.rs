//! Document seed operations.
use crate::metrics::{Metric, record};
use crate::refusal::Refusal;
use crate::room::{CloudDocument, fail};
use crate::room::{SQLITE_HEADER_BYTES, decode_hex, object_key, version, wire_version};
use pop_contracts::hex;
use pop_sharing_protocol::http::RoomPhase;
use pop_sharing_protocol::{CONTROL_BYTES, SEED_BYTES};
use sha2::{Digest, Sha256};
use worker::*;

impl CloudDocument {
    pub(crate) async fn open(&self, req: &mut Request, room: &str) -> Result<Response> {
        let _upload = self.uploads.lock().await;
        let phase = self.phase()?.ok_or_else(|| fail("Missing phase"))?;
        let input: pop_sharing_protocol::http::OpenRoom = match crate::input::body(req, CONTROL_BYTES).await {
            Ok(input) => input,
            Err(error) => return error.response(),
        };
        if !pop_contracts::wire::valid_attachment_id(&input.seed_sha256) {
            return Refusal::new(400, "invalid_request", "Invalid seed hash").response();
        }
        // Reading the body yielded to Stop, which also removes the seed.
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        // The one fence: only the stored seed can go live, so an older upload attempt
        // cannot publish a seed uploaded after it. Attachments are not checked here;
        // the creator uploads every one the seed references before Open.
        match self.document_state()?.and_then(|s| s.seed_hash) {
            None => return Refusal::new(409, "conflict", "Seed not ready").response(),
            Some(stored) if stored != input.seed_sha256 => {
                return Refusal::new(409, "conflict", "Different seed").response();
            }
            Some(_) => {}
        }
        if phase == RoomPhase::Live {
            return Response::from_json(&self.info()?);
        }
        let bucket = self.env.bucket("SEEDS")?;
        let seed_size: u64 = self.stored()?.seed_size.ok_or_else(|| fail("Invalid seed size"))? as u64;
        if bucket.head(object_key(room, "seed")).await?.is_none_or(|o| o.size() != seed_size) {
            return Refusal::new(409, "conflict", "Seed not ready").response();
        }
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        self.set_phase(RoomPhase::Live)?;
        self.publish_library().await?;
        record(&self.env, Metric::DocumentReady);
        Response::from_json(&self.info()?)
    }
    pub(crate) async fn put_seed(&self, req: &mut Request, room: &str) -> Result<Response> {
        let _upload = self.uploads.lock().await;
        match self.phase()? {
            Some(RoomPhase::Uploading) => {}
            Some(RoomPhase::Ended) => return Refusal::new(410, "sharing_ended", "Sharing ended").response(),
            _ => return Refusal::new(409, "conflict", "Room is live").response(),
        }
        let bucket = self.env.bucket("SEEDS")?;
        let key = object_key(room, "seed");
        let digest = req.headers().get("X-Sharing-SHA256")?.unwrap_or_default();
        let app = req.headers().get("X-Sharing-App")?.unwrap_or_default();
        let seed: Vec<(String, u32)> =
            match req.headers().get("X-Sharing-Version")?.and_then(|v| serde_json::from_str(&v).ok()) {
                Some(v) => v,
                None => return Refusal::new(400, "invalid_request", "Seed version required").response(),
            };
        let heads = match version(&seed) {
            Ok(heads) => heads,
            Err(_) => return Refusal::new(400, "invalid_request", "Invalid seed version").response(),
        };
        if !pop_contracts::wire::valid_attachment_id(&digest)
            || !pop_contracts::wire::valid_attachment_id(&app)
            || heads.len() != seed.len()
        {
            return Refusal::new(400, "invalid_request", "Invalid seed metadata").response();
        }
        let body = match crate::input::bytes(req, SEED_BYTES).await {
            Ok(bytes) => bytes,
            Err(error) => return error.response(),
        };
        if body.len() < SQLITE_HEADER_BYTES || hex(&Sha256::digest(&body)) != digest {
            return Refusal::new(422, "invalid_request", "Seed checksum mismatch").response();
        }
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        let size = body.len();
        bucket.put(key.clone(), body).sha256(decode_hex(&digest)?).execute().await?;
        if self.phase()? == Some(RoomPhase::Ended) {
            bucket.delete(key).await?;
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        let version = serde_json::to_string(&wire_version(&heads))?;
        self.update_state(|s| {
            s.seed_hash = Some(digest);
            s.seed_size = Some(size as u32);
            s.app = Some(app);
            s.root = Some(version);
        })?;
        self.sql.exec("DELETE FROM heads", None)?;
        Self::save_heads(&self.sql, &heads)?;
        Response::ok("ready")
    }
    pub(crate) async fn read_seed(&self, uid: &str, room: &str) -> Result<Response> {
        let object = self.env.bucket("SEEDS")?.get(object_key(room, "seed")).execute().await?;
        if self.role_for_uid(uid)?.is_none() {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        match object {
            Some(object) => {
                Response::from_body(object.body().ok_or_else(|| fail("Missing seed body"))?.response_body()?)
            }
            None => Refusal::new(404, "not_found", "Seed missing").response(),
        }
    }
}
