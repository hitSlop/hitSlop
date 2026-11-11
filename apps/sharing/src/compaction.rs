//! Document compaction operations.

use crate::refusal::Refusal;
use crate::room::{CHUNK, RecordRow, wire_version};
use crate::room::{CloudDocument, fail};
use pop_sharing_protocol::http::RoomPhase;
use worker::*;

use pop_contracts::{hex, relay::limits::BASE_BYTES};
use sha2::{Digest, Sha256};

impl CloudDocument {
    pub(crate) fn update_usage(&self) -> Result<(usize, usize)> {
        let state = self.stored()?;
        Ok((
            state.update_bytes.ok_or_else(|| fail("Missing update bytes"))?,
            state.update_rows.ok_or_else(|| fail("Missing update rows"))?,
        ))
    }
    pub(crate) fn compact_due(&self) -> Result<bool> {
        let (bytes, rows) = self.update_usage()?;
        Ok(crate::policy::compact_due(bytes, rows))
    }
    pub(crate) async fn compact(&self, req: &mut Request, uid: &str) -> Result<Response> {
        let body = match crate::input::bytes(req, BASE_BYTES).await {
            Ok(bytes) => bytes,
            Err(error) => return error.response(),
        };
        let state = self.document_state()?;
        match state.as_ref().map(|s| s.phase) {
            Some(RoomPhase::Live) => {}
            Some(RoomPhase::Uploading) => return Refusal::new(409, "conflict", "Publication pending").response(),
            Some(RoomPhase::Ended) => return Refusal::new(410, "sharing_ended", "Sharing ended").response(),
            None => return Refusal::new(404, "not_found", "Unknown room").response(),
        }
        let state = state.ok_or_else(|| fail("Missing document state"))?;
        let digest = req.headers().get("X-Sharing-SHA256")?.unwrap_or_default();
        let parse = |text: Option<String>| text.and_then(|s| crate::policy::version_header(&s));
        let Some(frontier) = parse(req.headers().get("X-Sharing-Version")?) else {
            return Refusal::new(400, "invalid_request", "Invalid version").response();
        };
        // Every live document stored its root with its seed; a missing or unreadable root is
        // a fault in this document, never something a client may supply.
        let root = parse(state.root).ok_or_else(|| fail("Missing stored root version"))?;
        let previous_json = state.base_version;
        let previous = match &previous_json {
            Some(value) => parse(Some(value.clone())).ok_or_else(|| fail("Invalid stored base version"))?,
            None => root.clone(),
        };
        let heads = self.heads()?;
        let refused = {
            let bytes = state.update_bytes.ok_or_else(|| fail("Missing update bytes"))?;
            let rows = state.update_rows.ok_or_else(|| fail("Missing update rows"))?;
            !crate::policy::Fold { root: &root, previous: &previous, heads: &heads, frontier: &frontier, bytes, rows }
                .admitted()
        };
        if refused {
            return Refusal::new(409, "conflict", "Compaction conflict").response();
        }
        if !pop_contracts::wire::valid_attachment_id(&digest) || hex(&Sha256::digest(&body)) != digest {
            return Refusal::new(400, "invalid_request", "Base checksum mismatch").response();
        }
        if self.role_for_uid(uid)?.is_none() {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        let mut covered = vec![];
        let mut removed_bytes = 0;
        let mut removed_rows = 0;
        for row in self.sql.exec("SELECT seq,spans,size FROM records ORDER BY seq", None)?.next::<RecordRow>() {
            let row = row?;
            let spans = crate::policy::decode_spans(&row.spans).map_err(fail)?;
            if crate::policy::folds_record(row.seq, &spans, &frontier) {
                covered.push(row.seq);
                if row.seq > 0 {
                    removed_bytes += row.size;
                    removed_rows += 1;
                }
            }
        }
        // All inputs and the atomic swap share this turn; no request can interleave without an await.
        let sql = self.sql.clone();
        let frontier_json = serde_json::to_string(&wire_version(&frontier))?;
        let spans: Vec<_> = frontier
            .iter()
            .filter_map(|(p, end)| {
                let start = root.get(p).copied().unwrap_or(0);
                (*end > start).then_some(pop_contracts::relay::frame::Span { peer: *p, start, end: *end })
            })
            .collect::<Vec<_>>();
        let spans = crate::policy::encode_spans(&spans);
        self.transaction(move || {
            for seq in &covered {
                sql.exec("DELETE FROM chunks WHERE seq=?", vec![(*seq).into()])?;
                sql.exec("DELETE FROM records WHERE seq=?", vec![(*seq).into()])?;
            }
            sql.exec(
                "INSERT INTO records(seq,spans,size) VALUES(0,?,?)",
                vec![spans.clone().into(), (body.len() as i64).into()],
            )?;
            for (part, chunk) in body.chunks(CHUNK).enumerate() {
                sql.exec("INSERT INTO chunks VALUES(0,?,?)", vec![(part as i64).into(), chunk.to_vec().into()])?;
            }
            sql.exec(
                "UPDATE document_state SET base_version=?,update_bytes=update_bytes-?,update_rows=update_rows-?",
                vec![frontier_json.clone().into(), (removed_bytes as i64).into(), (removed_rows as i64).into()],
            )?;
            Ok(())
        })?;
        self.state.storage().sync().await?;
        Response::empty().map(|r| r.with_status(204))
    }
}
