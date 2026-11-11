//! Document sync operations.
use crate::limits::{ROOM_CONNECTIONS, ROOM_IP_CONNECTIONS};
use crate::metrics::{Metric, record};
use crate::refusal::Refusal;
use crate::room::{BlobRow, CHUNK, NumberRow, RecordRow, SocketResult, SocketState, now, refused, wire_version};
use crate::room::{CloudDocument, fail};
use pop_contracts::{
    hex,
    relay::frame::{Ack, ErrorCode, Frame, PROTOCOL, Permission, Record, Version},
};
use pop_sharing_protocol::http::RoomPhase;
use worker::*;

impl CloudDocument {
    pub(crate) fn bytes(&self, seq: i64, size: usize) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(size);
        for chunk in
            self.sql.exec("SELECT bytes FROM chunks WHERE seq=? ORDER BY part", vec![seq.into()])?.next::<BlobRow>()
        {
            bytes.extend(chunk?.bytes);
        }
        if bytes.len() != size {
            return Err(fail("Incomplete stored record"));
        }
        Ok(bytes)
    }
    pub(crate) fn send(&self, ws: &WebSocket, frame: Frame) -> Result<()> {
        ws.send_with_bytes(pop_contracts::relay::envelope::encode(frame).map_err(fail)?)
    }
    pub(crate) fn end_connections(&self, code: ErrorCode, message: &str, close: u16) {
        for ws in self.state.get_websockets() {
            let _ = self.clear_catchup(&ws);
            let _ = ws.serialize_attachment(SocketState::default());
            let _ = self.send(&ws, Frame::Error { code: code.into(), message: message.into() });
            let _ = ws.close(Some(close), Some(message));
        }
    }
    pub(crate) fn clear_catchup(&self, ws: &WebSocket) -> Result<()> {
        if let Some(state) = ws.deserialize_attachment::<SocketState>()? {
            self.sql.exec("DELETE FROM catchups WHERE socket_id=?", vec![state.socket_id.into()])?;
        }
        Ok(())
    }
    pub(crate) async fn hello(&self, ws: &WebSocket, vv: Version) -> Result<()> {
        let mut socket = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        socket.hello = true;
        ws.serialize_attachment(&socket)?;
        self.sql.exec(
            "INSERT INTO catchups VALUES(?,?,-1,?)",
            vec![
                socket.socket_id.into(),
                serde_json::to_string(&wire_version(&vv))?.into(),
                self.stored()?.base_version.map(Into::into).unwrap_or(SqlStorageValue::Null),
            ],
        )?;
        self.send(ws, Frame::Welcome { permission: Permission::Write, version: self.heads()? })?;
        self.schedule_cleanup(crate::limits::CATCHUP_DELAY_MS).await
    }
    pub(crate) async fn catch_up_sockets(&self) -> Result<()> {
        let mut bytes = 0;
        let mut rows = 0;
        for socket in self.state.get_websockets() {
            if bytes >= crate::limits::CATCHUP_BYTES || rows >= crate::limits::CATCHUP_ROWS {
                break;
            }
            if self.catch_up_slice(&socket, &mut bytes, &mut rows).is_err() {
                self.clear_catchup(&socket)?;
                let _ = socket.close(Some(1011), None::<String>);
            }
        }
        if self.sql.exec("SELECT count(*) AS n FROM catchups", None)?.one::<NumberRow>()?.n > 0 {
            self.schedule_cleanup(crate::limits::CATCHUP_DELAY_MS).await?;
        }
        Ok(())
    }
    fn catch_up_slice(&self, ws: &WebSocket, sent: &mut usize, scanned: &mut usize) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct Catchup {
            version: String,
            cursor: i64,
            base_version: Option<String>,
        }
        let mut socket = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        let Some(mut pending) = self
            .sql
            .exec(
                "SELECT version,cursor,base_version FROM catchups WHERE socket_id=?",
                vec![socket.socket_id.clone().into()],
            )?
            .to_array::<Catchup>()?
            .pop()
        else {
            return Ok(());
        };
        if self.phase()? != Some(RoomPhase::Live) || self.role_for_uid(&socket.uid)?.is_none() {
            self.clear_catchup(ws)?;
            return ws.close(Some(1008), Some("Access revoked"));
        }
        let base = self.stored()?.base_version;
        // A concurrent fold replaces seq=0 and may delete rows ahead of the cursor.
        // Restart at the new base so no accepted history can be skipped.
        if pending.base_version != base {
            pending.cursor = -1;
        }
        let version = crate::room::version(&serde_json::from_str::<Vec<(String, u32)>>(&pending.version)?)?;
        let mut exhausted = true;
        for row in self
            .sql
            .exec(
                "SELECT seq,spans,size FROM records WHERE seq>? ORDER BY seq LIMIT ?",
                vec![pending.cursor.into(), ((crate::limits::CATCHUP_ROWS - *scanned) as i64).into()],
            )?
            .next::<RecordRow>()
        {
            let row = row?;
            let spans = crate::policy::decode_spans(&row.spans).map_err(fail)?;
            let needed = spans.iter().any(|s| s.end > version.get(&s.peer).copied().unwrap_or(0));
            // Conservatively include framing and span varints before loading payload chunks.
            let wire_bytes = row.size.saturating_add(spans.len().saturating_mul(18)).saturating_add(16);
            if needed && *sent > 0 && sent.saturating_add(wire_bytes) > crate::limits::CATCHUP_BYTES {
                exhausted = false;
                break;
            }
            *scanned += 1;
            if needed {
                self.send(ws, Frame::Records(vec![Record { spans, bytes: self.bytes(row.seq, row.size)? }]))?;
                *sent = sent.saturating_add(wire_bytes);
            }
            pending.cursor = row.seq;
            if *sent >= crate::limits::CATCHUP_BYTES || *scanned >= crate::limits::CATCHUP_ROWS {
                exhausted = false;
                break;
            }
        }
        if exhausted {
            self.send(ws, Frame::CaughtUp)?;
            socket.joined = true;
            ws.serialize_attachment(socket)?;
            self.clear_catchup(ws)
        } else {
            self.sql.exec(
                "UPDATE catchups SET cursor=?,base_version=? WHERE socket_id=?",
                vec![
                    pending.cursor.into(),
                    base.map(Into::into).unwrap_or(SqlStorageValue::Null),
                    socket.socket_id.into(),
                ],
            )?;
            Ok(())
        }
    }
    pub(crate) async fn push(&self, ws: &WebSocket, id: u64, records: Vec<Record>, wire_bytes: usize) -> Result<()> {
        let mut budget = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        let mut charge =
            crate::policy::PushBudget { window_start: budget.window_start, bytes: budget.bytes, pushes: budget.pushes };
        let limited = !charge.charge(now(), wire_bytes);
        budget.window_start = charge.window_start;
        budget.bytes = charge.bytes;
        budget.pushes = charge.pushes;
        ws.serialize_attachment(budget)?;
        if limited {
            ws.close(Some(1013), Some("Push budget exceeded"))?;
            return Ok(());
        }
        if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.hello) {
            return Err(fail("Hello required"));
        }
        let Some((heads, added)) = crate::policy::admit_push(self.heads()?, records) else {
            return self.send(ws, Frame::Ack { id, status: Ack::Gap });
        };
        // Lost acknowledgements are retried even when later writes filled the room.
        // The socket budget above still charges duplicates to prevent abuse.
        if added.is_empty() {
            return self.send(ws, Frame::Ack { id, status: Ack::Stored });
        }
        let (stored, rows) = self.update_usage()?;
        if !crate::policy::push_fits(stored, rows, &added) {
            return self.send(ws, Frame::Ack { id, status: Ack::Quota });
        }
        let sql = self.sql.clone();
        let save = added.clone();
        let changed_heads = added.iter().flat_map(|r| r.spans.iter()).map(|s| (s.peer, heads[&s.peer])).collect();
        self.transaction(move || {
            for record in &save {
                let spans = crate::policy::encode_spans(&record.spans);
                let seq = sql
                    .exec(
                        "INSERT INTO records(spans,size) VALUES(?,?) RETURNING seq AS n",
                        vec![spans.into(), (record.bytes.len() as i64).into()],
                    )?
                    .one::<NumberRow>()?
                    .n;
                for (part, chunk) in record.bytes.chunks(CHUNK).enumerate() {
                    sql.exec(
                        "INSERT INTO chunks VALUES(?,?,?)",
                        vec![seq.into(), (part as i64).into(), chunk.to_vec().into()],
                    )?;
                }
            }
            Self::save_heads(&sql, &changed_heads)?;
            sql.exec(
                "UPDATE document_state SET update_bytes=update_bytes+?,update_rows=update_rows+?",
                vec![(save.iter().map(|r| r.bytes.len()).sum::<usize>() as i64).into(), (save.len() as i64).into()],
            )?;
            Ok(())
        })?;
        self.state.storage().sync().await?;
        if self.phase()? == Some(RoomPhase::Ended) {
            return Err(fail("Sharing ended"));
        }
        self.send(ws, Frame::Ack { id, status: Ack::Stored })?;
        for other in self.state.get_websockets() {
            // Echo is harmless and keeps broadcast routing independent of JS object identity.
            if other.deserialize_attachment::<SocketState>().ok().flatten().is_some_and(|s| s.joined) {
                for record in &added {
                    if self.send(&other, Frame::Records(vec![record.clone()])).is_err() {
                        let _ = other.close(Some(1011), None::<String>);
                        break;
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) async fn message(&self, ws: &WebSocket, msg: WebSocketIncomingMessage) -> SocketResult {
        let access = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        if !access.owner && self.role_for_uid(&access.uid)?.is_none() {
            let _ = ws.close(Some(1008), Some("Access revoked"));
            return Ok(());
        }
        if self.phase()? != Some(RoomPhase::Live) {
            return Err(refused(ErrorCode::Ended, "Sharing ended"));
        }
        let WebSocketIncomingMessage::Binary(bytes) = msg else {
            return Err(refused(ErrorCode::Malformed, "Binary frame required"));
        };
        let frame = pop_contracts::relay::envelope::decode(&bytes).map_err(|e| refused(ErrorCode::Malformed, &e))?;
        match frame {
            Frame::Hello { protocol, app_digest, version } => {
                if access.hello {
                    return Err(refused(ErrorCode::Protocol, "One Hello per connection"));
                }
                if protocol != PROTOCOL {
                    return Err(refused(ErrorCode::Protocol, "Different app or protocol"));
                }
                let expected_app = self.document_state()?.and_then(|s| s.app).ok_or_else(|| fail("Missing app"))?;
                if expected_app != hex(app_digest.as_bytes()) {
                    return Err(refused(ErrorCode::Protocol, "Different app or protocol"));
                }
                self.hello(ws, version).await?;
                Ok(())
            }
            Frame::Push { id, records } => {
                if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.hello) {
                    return Err(refused(ErrorCode::Malformed, "Hello required"));
                }
                self.push(ws, id, records, bytes.len()).await?;
                Ok(())
            }
            _ => Err(refused(ErrorCode::Malformed, "Client frame required")),
        }
    }
    pub(crate) async fn connect_socket(&self, req: &Request, uid: &str, owner: bool) -> Result<Response> {
        if req.headers().get("Upgrade")?.as_deref() != Some("websocket") {
            return Refusal::new(404, "not_found", "Not found").response();
        }
        if self.state.get_websockets().len() >= ROOM_CONNECTIONS {
            return Refusal::new(429, "limited", "Room connection limit").response();
        }
        let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_else(|| "unknown".into());
        let mut count = 0;
        for socket in self.state.get_websockets() {
            if socket.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.ip == ip) {
                count += 1;
            }
        }
        if count >= ROOM_IP_CONNECTIONS {
            return Refusal::new(429, "limited", "Room IP connection limit").response();
        }
        if self.phase()? != Some(RoomPhase::Live) || self.role_for_uid(uid)?.is_none() {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        let pair = WebSocketPair::new()?;
        self.state.accept_web_socket(&pair.server);
        pair.server.serialize_attachment(SocketState {
            joined: false,
            hello: false,
            socket_id: crate::room::random_hex(16)?,
            owner,
            uid: uid.into(),
            ip,
            window_start: now(),
            bytes: 0,
            pushes: 0,
        })?;
        record(&self.env, Metric::GuestJoin);
        Response::from_websocket(pair.client)
    }
}
