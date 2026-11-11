//! Document sync operations.
use crate::limits::{ROOM_CONNECTIONS, ROOM_IP_CONNECTIONS};
use crate::metrics::{Metric, record};
use crate::refusal::Refusal;
use crate::room::{
    BACKFILL, BlobRow, CHUNK, NumberRow, RecordRow, SocketResult, SocketState, now, refused, wire_version,
};
use crate::room::{CloudDocument, fail};
use pop_contracts::{
    hex,
    relay::frame::{Ack, ErrorCode, Frame, PROTOCOL, Permission, Record, Span, Version},
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
            let _ = ws.serialize_attachment(SocketState::default());
            let _ = self.send(&ws, Frame::Error { code: code.into(), message: message.into() });
            let _ = ws.close(Some(close), Some(message));
        }
    }
    pub(crate) fn hello(
        &self,
        ws: &WebSocket,
        protocol: u8,
        app: &pop_contracts::AppDigest,
        vv: Version,
    ) -> Result<()> {
        if protocol != PROTOCOL || self.document_state()?.and_then(|s| s.app).as_deref() != Some(&hex(app.as_bytes())) {
            return Err(fail("Different app or protocol"));
        }
        self.send(ws, Frame::Welcome { permission: Permission::Write, version: self.heads()? })?;
        let mut batch = vec![];
        let mut size = 0;
        for row in self.sql.exec("SELECT seq,spans,size FROM records ORDER BY seq", None)?.next::<RecordRow>() {
            let row = row?;
            let spans: Vec<(String, u32, u32)> = serde_json::from_str(&row.spans)?;
            let spans: Vec<Span> = spans
                .into_iter()
                .map(|(p, start, end)| {
                    Ok(Span { peer: u64::from_str_radix(&p, 16).map_err(|_| fail("Stored peer"))?, start, end })
                })
                .collect::<Result<_>>()?;
            if !spans.iter().any(|s| s.end > vv.get(&s.peer).copied().unwrap_or(0)) {
                continue;
            }
            if size + row.size > BACKFILL && !batch.is_empty() {
                self.send(ws, Frame::Records(std::mem::take(&mut batch)))?;
                size = 0;
            }
            size += row.size;
            batch.push(Record { spans, bytes: self.bytes(row.seq, row.size)? });
        }
        if !batch.is_empty() {
            self.send(ws, Frame::Records(batch))?;
        }
        self.send(ws, Frame::CaughtUp)?;
        let mut s = ws.deserialize_attachment::<SocketState>()?.ok_or_else(|| fail("Missing socket"))?;
        s.joined = true;
        ws.serialize_attachment(s)
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
        if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.joined) {
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
        let heads_json = serde_json::to_string(&wire_version(&heads))?;
        self.transaction(move || {
            for record in &save {
                let spans: Vec<_> = record.spans.iter().map(|s| (format!("{:016x}", s.peer), s.start, s.end)).collect();
                let seq = sql
                    .exec(
                        "INSERT INTO records(spans,size) VALUES(?,?) RETURNING seq AS n",
                        vec![serde_json::to_string(&spans)?.into(), (record.bytes.len() as i64).into()],
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
            sql.exec("UPDATE document_state SET heads=?", vec![heads_json.clone().into()])?;
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
        if !access.owner
            && (access.generation != self.generation()?
                || access.uid.is_empty()
                || (self
                    .sql
                    .exec(
                        "SELECT count(*) AS n FROM members WHERE uid=? AND generation=?",
                        vec![access.uid.into(), (self.generation()? as i64).into()],
                    )?
                    .one::<NumberRow>()?
                    .n
                    == 0))
        {
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
                if protocol != PROTOCOL {
                    return Err(refused(ErrorCode::Protocol, "Different app or protocol"));
                }
                let expected_app = self.document_state()?.and_then(|s| s.app).ok_or_else(|| fail("Missing app"))?;
                if expected_app != hex(app_digest.as_bytes()) {
                    return Err(refused(ErrorCode::Protocol, "Different app or protocol"));
                }
                self.hello(ws, protocol, &app_digest, version)?;
                Ok(())
            }
            Frame::Push { id, records } => {
                if !ws.deserialize_attachment::<SocketState>()?.is_some_and(|s| s.joined) {
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
        let generation = self.generation()?;
        if self.phase()? != Some(RoomPhase::Live) || self.role_for_uid(uid)?.is_none() {
            return Refusal::new(403, "denied", "Access revoked").response();
        }
        let pair = WebSocketPair::new()?;
        self.state.accept_web_socket(&pair.server);
        pair.server.serialize_attachment(SocketState {
            joined: false,
            owner,
            generation,
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
