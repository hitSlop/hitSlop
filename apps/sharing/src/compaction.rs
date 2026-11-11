//! Document compaction operations.

use crate::refusal::Refusal;
use crate::room::{CHUNK, NumberRow, RecordRow, version, wire_version};
use crate::room::{CloudDocument, fail};
use pop_sharing_protocol::http::RoomPhase;
use worker::*;

use pop_contracts::{hex, relay::limits::BASE_BYTES};
use serde::Deserialize;
use sha2::{Digest, Sha256};

impl CloudDocument {
    pub(crate) fn update_usage(&self) -> Result<(usize, usize)> {
        #[derive(Deserialize)]
        struct Usage {
            bytes: usize,
            rows: usize,
        }
        let usage = self
            .sql
            .exec("SELECT coalesce(sum(size),0) AS bytes,count(*) AS rows FROM records WHERE seq>0", None)?
            .one::<Usage>()?;
        Ok((usage.bytes, usage.rows))
    }
    pub(crate) fn compact_due(&self) -> Result<bool> {
        let (bytes, rows) = self.update_usage()?;
        Ok(crate::policy::compact_due(bytes, rows))
    }
    pub(crate) async fn compact(&self, req: &mut Request, uid: &str) -> Result<Response> {
        // Serialize competing archives; Push and Stop may still progress across the R2 await.
        let _guard = self.uploads.lock().await;
        let body = match crate::input::bytes(req, BASE_BYTES).await {
            Ok(bytes) => bytes,
            Err(error) => return error.response(),
        };
        match self.phase()? {
            Some(RoomPhase::Live) => {}
            Some(RoomPhase::Uploading) => return Refusal::new(409, "conflict", "Publication pending").response(),
            Some(RoomPhase::Ended) => return Refusal::new(410, "sharing_ended", "Sharing ended").response(),
            None => return Refusal::new(404, "not_found", "Unknown room").response(),
        }
        let digest = req.headers().get("X-Sharing-SHA256")?.unwrap_or_default();
        let parse = |text: Option<String>| text.and_then(|s| crate::policy::version_header(&s));
        let Some(frontier) = parse(req.headers().get("X-Sharing-Version")?) else {
            return Refusal::new(400, "invalid_request", "Invalid version").response();
        };
        // Every live document stored its root with its seed; a missing or unreadable root is
        // a fault in this document, never something a client may supply.
        let root =
            parse(self.document_state()?.and_then(|s| s.root)).ok_or_else(|| fail("Missing stored root version"))?;
        let previous_json = self.document_state()?.and_then(|s| s.base_version);
        let previous = match &previous_json {
            Some(value) => parse(Some(value.clone())).ok_or_else(|| fail("Invalid stored base version"))?,
            None => root.clone(),
        };
        let heads = self.heads()?;
        let refused = {
            let (bytes, rows) = self.update_usage()?;
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
        for row in self.sql.exec("SELECT seq,spans,size FROM records ORDER BY seq", None)?.next::<RecordRow>() {
            let row = row?;
            let spans: Vec<(String, u32, u32)> = serde_json::from_str(&row.spans)?;
            let spans: Vec<pop_contracts::relay::frame::Span> = spans
                .into_iter()
                .map(|(peer, start, end)| {
                    Ok(pop_contracts::relay::frame::Span {
                        peer: u64::from_str_radix(&peer, 16).map_err(|_| fail("Stored peer"))?,
                        start,
                        end,
                    })
                })
                .collect::<Result<_>>()?;
            if crate::policy::folds_record(row.seq, &spans, &frontier) {
                covered.push(row.seq);
            }
        }
        // No await within the swap. Recheck the generation and due state inside transactionSync.
        let sql = self.sql.clone();
        let frontier_json = serde_json::to_string(&wire_version(&frontier))?;
        let root_json = serde_json::to_string(&wire_version(&root))?;
        let spans: Vec<_> = frontier
            .iter()
            .filter_map(|(p, end)| {
                let start = root.get(p).copied().unwrap_or(0);
                (*end > start).then(|| (format!("{p:016x}"), start, *end))
            })
            .collect();
        let spans_json = serde_json::to_string(&spans)?;
        let installed = std::rc::Rc::new(std::cell::Cell::new(false));
        let swapped = installed.clone();
        self.transaction(move || {
            let state =
                crate::document_state::DocumentState::load(&sql)?.ok_or_else(|| fail("Missing document state"))?;
            let bytes =
                sql.exec("SELECT coalesce(sum(size),0) AS n FROM records WHERE seq>0", None)?.one::<NumberRow>()?.n
                    as usize;
            let rows = sql.exec("SELECT count(*) AS n FROM records WHERE seq>0", None)?.one::<NumberRow>()?.n as usize;
            if state.phase != RoomPhase::Live || state.base_version != previous_json {
                return Ok(());
            }
            let committed_heads = version(&serde_json::from_str::<Vec<(String, u32)>>(
                &state.heads.ok_or_else(|| fail("Missing heads"))?,
            )?)?;
            if !(crate::policy::Fold {
                root: &root,
                previous: &previous,
                heads: &committed_heads,
                frontier: &frontier,
                bytes,
                rows,
            })
            .admitted()
            {
                return Ok(());
            }
            for seq in &covered {
                sql.exec("DELETE FROM chunks WHERE seq=?", vec![(*seq).into()])?;
                sql.exec("DELETE FROM records WHERE seq=?", vec![(*seq).into()])?;
            }
            sql.exec(
                "INSERT INTO records(seq,spans,size) VALUES(0,?,?)",
                vec![spans_json.clone().into(), (body.len() as i64).into()],
            )?;
            for (part, chunk) in body.chunks(CHUNK).enumerate() {
                sql.exec("INSERT INTO chunks VALUES(0,?,?)", vec![(part as i64).into(), chunk.to_vec().into()])?;
            }
            sql.exec(
                "UPDATE document_state SET base_version=?,root=?",
                vec![frontier_json.clone().into(), root_json.clone().into()],
            )?;
            swapped.set(true);
            Ok(())
        })?;
        if !installed.get() {
            return Refusal::new(409, "conflict", "Compaction conflict").response();
        }
        self.state.storage().sync().await?;
        Response::empty().map(|r| r.with_status(204))
    }
}
