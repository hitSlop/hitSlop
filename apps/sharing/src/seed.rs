//! Commit a staged blob set as the immutable document seed and initial attachments.
use crate::metrics::{Metric, record};
use crate::refusal::Refusal;
use crate::room::{SQLITE_HEADER_BYTES, SharedDocument, fail, object_key, version, wire_version};
use pop_contracts::hex;
use pop_sharing_protocol::http::RoomPhase;
use pop_sharing_protocol::{CONTROL_BYTES, SEED_BYTES, resources};
use worker::*;

impl SharedDocument {
    pub(crate) async fn open(&self, req: &mut Request, room: &str) -> Result<Response> {
        let _upload = self.uploads.lock().await;
        let input: pop_sharing_protocol::http::OpenRoom = match crate::input::body(req, CONTROL_BYTES).await {
            Ok(input) => input,
            Err(error) => return error.response(),
        };
        let heads = match version(&input.version) {
            Ok(heads) if heads.len() == input.version.len() => heads,
            _ => return Refusal::new(400, "invalid_request", "Invalid seed version").response(),
        };
        if !pop_contracts::wire::valid_attachment_id(&input.seed)
            || !pop_contracts::wire::valid_attachment_id(&input.app)
            || input.attachments.iter().any(|id| !pop_contracts::wire::valid_attachment_id(id) || id == &input.seed)
        {
            return Refusal::new(400, "invalid_request", "Invalid seed metadata").response();
        }
        let root = serde_json::to_string(&wire_version(&heads))?;
        let state = self.stored()?;
        if state.phase == RoomPhase::Ended {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        if state.phase == RoomPhase::Live {
            return if state.seed_hash.as_ref() == Some(&input.seed)
                && state.app.as_ref() == Some(&input.app)
                && state.root.as_ref() == Some(&root)
            {
                Response::from_json(&self.info()?)
            } else {
                Refusal::new(409, "conflict", "Different seed").response()
            };
        }
        let staged = self.blob_inventory(false)?;
        let Some(seed) = staged.iter().find(|item| item.hash == input.seed) else {
            return Refusal::new(409, "conflict", "Seed not ready").response();
        };
        if seed.size < SQLITE_HEADER_BYTES as u64 || seed.size > SEED_BYTES as u64 {
            return Refusal::new(400, "invalid_request", "Invalid seed size").response();
        }
        let mut attachments = Vec::with_capacity(input.attachments.len());
        for id in &input.attachments {
            let Some(item) = staged.iter().find(|item| &item.hash == id) else {
                return Refusal::new(409, "conflict", "Attachment not ready").response();
            };
            attachments.push(item.clone());
        }
        if resources::validate(&attachments).is_err() {
            return Refusal::new(413, "invalid_request", "Room file limit reached").response();
        }
        let bucket = self.env.bucket("SEEDS")?;
        for item in std::iter::once(seed).chain(attachments.iter()) {
            let object = bucket.head(object_key(room, &format!("blobs/{}", item.hash))).await?;
            if object.is_none_or(|o| {
                o.size() != item.size || o.checksum().sha256.is_some_and(|digest| hex(&digest) != item.hash)
            }) {
                return Refusal::new(409, "conflict", "Blob not ready").response();
            }
        }
        self.schedule_cleanup(1).await?;
        if self.phase()? == Some(RoomPhase::Ended) {
            return Refusal::new(410, "sharing_ended", "Sharing ended").response();
        }
        let seed_size = seed.size as u32;
        let room = room.to_owned();
        self.library_transaction(move |sql, s| {
            for item in &staged {
                let attachment = input.attachments.contains(&item.hash);
                if item.hash != input.seed && !attachment {
                    sql.exec(
                        "INSERT OR IGNORE INTO garbage VALUES(?)",
                        vec![object_key(&room, &format!("blobs/{}", item.hash)).into()],
                    )?;
                    sql.exec("DELETE FROM resource_blobs WHERE id=?", vec![item.hash.clone().into()])?;
                } else {
                    sql.exec(
                        "UPDATE resource_blobs SET attachment=? WHERE id=?",
                        vec![i64::from(attachment).into(), item.hash.clone().into()],
                    )?;
                }
            }
            s.seed_hash = Some(input.seed.clone());
            s.seed_size = Some(seed_size);
            s.app = Some(input.app.clone());
            s.root = Some(root.clone());
            s.phase = RoomPhase::Live;
            sql.exec("DELETE FROM heads", None)?;
            Self::save_heads(sql, &heads)?;
            Ok(vec![])
        })?;
        self.state.storage().sync().await?;
        // Failed deletes remain in the durable garbage queue; they cannot affect the live set.
        self.sweep_garbage(&self.stored()?.document).await?;
        record(&self.env, Metric::DocumentReady);
        Response::from_json(&self.info()?)
    }
    pub(crate) async fn read_seed(&self, uid: &str, room: &str) -> Result<Response> {
        let Some(hash) = self.stored()?.seed_hash else {
            return Refusal::new(404, "not_found", "Seed missing").response();
        };
        let object = self.env.bucket("SEEDS")?.get(object_key(room, &format!("blobs/{hash}"))).execute().await?;
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
