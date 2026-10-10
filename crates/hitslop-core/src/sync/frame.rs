//! The relay wire: binary frames, one room per socket. The relay reads only the span
//! headers; the Loro bytes they index are opaque to it (plans/sync-simplification.md §7.1).
//!
//! ```text
//! vv      := varuint n, n × (peer u64 BE, counter varuint)       sorted by peer
//! span    := peer u64 BE, start varuint, end varuint               end > start
//! record  := varuint n, n × span, varBytes loro_update_bytes
//! 0x01 Hello    { protocol u8, app_digest [32], vv }              client → relay
//! 0x02 Welcome  { permission u8, relay_vv vv }                    relay → client
//! 0x03 Records  { varuint n, n × record }                         backfill and broadcast
//! 0x04 CaughtUp {}                                                backfill finished
//! 0x05 Push     { id u64 BE, varuint n, n × record }              client → relay
//! 0x06 Ack      { id u64 BE, status u8 }                          relay → client
//! 0x07 Error    { code u8, message varString }                    then the relay closes
//! ```
//!
//! `tests/sync-frames.json` holds golden frames the relay's own codec is tested against.
use crate::{Code, Result, err};
use std::collections::BTreeMap;

/// The wire's version, sent in every Hello. A relay refuses one it does not speak.
pub const PROTOCOL: u8 = 1;
/// The largest frame either side sends or accepts: a Cloudflare WebSocket message's limit.
pub const MAX_FRAME: usize = 32 << 20;

/// One peer's operations `start..end`: what a record's bytes contain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub peer: u64,
    pub start: u32,
    pub end: u32,
}
/// Loro update bytes and the spans they hold, which the relay indexes them by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub spans: Vec<Span>,
    pub bytes: Vec<u8>,
}
/// A version vector: each peer's exclusive end counter.
pub type Version = BTreeMap<u64, u32>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Read,
    Write,
}
/// Whether the relay stored a push. A refusal other than `Gap` pauses sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ack {
    Stored,
    /// A span starts past what the relay holds for its peer: the client's idea of the
    /// relay is wrong, and a new Hello corrects it.
    Gap,
    TooLarge,
    Denied,
    /// The room is full.
    Quota,
}
impl Ack {
    fn code(self) -> u8 {
        self as u8
    }
    fn of(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::Stored,
            1 => Self::Gap,
            2 => Self::TooLarge,
            3 => Self::Denied,
            4 => Self::Quota,
            _ => return Err(invalid("unknown acknowledgement")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    Hello { protocol: u8, app_digest: [u8; 32], version: Version },
    Welcome { permission: Permission, version: Version },
    Records(Vec<Record>),
    CaughtUp,
    Push { id: u64, records: Vec<Record> },
    Ack { id: u64, status: Ack },
    Error { code: u8, message: String },
}

fn invalid(message: &str) -> crate::Error {
    err(Code::InvalidBytes, format!("Invalid sync frame: {message}"))
}
fn put_uint(out: &mut Vec<u8>, mut n: u64) {
    while n >= 0x80 {
        out.push(n as u8 | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}
fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_uint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}
fn put_version(out: &mut Vec<u8>, version: &Version) {
    put_uint(out, version.len() as u64);
    for (peer, end) in version {
        out.extend_from_slice(&peer.to_be_bytes());
        put_uint(out, u64::from(*end));
    }
}
fn put_records(out: &mut Vec<u8>, records: &[Record]) {
    put_uint(out, records.len() as u64);
    for record in records {
        put_uint(out, record.spans.len() as u64);
        for span in &record.spans {
            out.extend_from_slice(&span.peer.to_be_bytes());
            put_uint(out, u64::from(span.start));
            put_uint(out, u64::from(span.end));
        }
        put_bytes(out, &record.bytes);
    }
}

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![];
        match self {
            Self::Hello { protocol, app_digest, version } => {
                out.push(1);
                out.push(*protocol);
                out.extend_from_slice(app_digest);
                put_version(&mut out, version);
            }
            Self::Welcome { permission, version } => {
                out.push(2);
                out.push(matches!(permission, Permission::Write) as u8);
                put_version(&mut out, version);
            }
            Self::Records(records) => {
                out.push(3);
                put_records(&mut out, records);
            }
            Self::CaughtUp => out.push(4),
            Self::Push { id, records } => {
                out.push(5);
                out.extend_from_slice(&id.to_be_bytes());
                put_records(&mut out, records);
            }
            Self::Ack { id, status } => {
                out.push(6);
                out.extend_from_slice(&id.to_be_bytes());
                out.push(status.code());
            }
            Self::Error { code, message } => {
                out.push(7);
                out.push(*code);
                put_bytes(&mut out, message.as_bytes());
            }
        }
        out
    }
    /// One whole frame: anything malformed, truncated, followed by more bytes or larger
    /// than `MAX_FRAME` is refused.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_FRAME {
            return Err(err(Code::TooLarge, "Sync frame exceeds its size limit"));
        }
        let mut reader = Reader(bytes);
        let frame = reader.frame()?;
        if !reader.0.is_empty() {
            return Err(invalid("trailing bytes"));
        }
        Ok(frame)
    }
}

struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(invalid("truncated"));
        }
        let (taken, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(taken)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().expect("eight bytes")))
    }
    fn uint(&mut self) -> Result<u64> {
        let mut n = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            n |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(n);
            }
        }
        Err(invalid("overlong integer"))
    }
    fn counter(&mut self) -> Result<u32> {
        // Loro counters are non-negative i32s.
        u32::try_from(self.uint()?).ok().filter(|n| *n <= i32::MAX as u32).ok_or_else(|| invalid("counter"))
    }
    /// A count of items that each take at least one byte, so a count cannot claim more
    /// than the frame holds.
    fn count(&mut self) -> Result<usize> {
        let n = self.uint()?;
        usize::try_from(n).ok().filter(|n| *n <= self.0.len()).ok_or_else(|| invalid("count"))
    }
    fn version(&mut self) -> Result<Version> {
        let mut version = Version::new();
        let mut last = None;
        for _ in 0..self.count()? {
            let peer = self.u64()?;
            if last.is_some_and(|last| last >= peer) {
                return Err(invalid("version peers out of order"));
            }
            last = Some(peer);
            version.insert(peer, self.counter()?);
        }
        Ok(version)
    }
    fn records(&mut self) -> Result<Vec<Record>> {
        let mut records = vec![];
        for _ in 0..self.count()? {
            let mut spans = vec![];
            for _ in 0..self.count()? {
                let span = Span { peer: self.u64()?, start: self.counter()?, end: self.counter()? };
                if span.end <= span.start {
                    return Err(invalid("empty span"));
                }
                spans.push(span);
            }
            if spans.is_empty() {
                return Err(invalid("a record without spans"));
            }
            let n = self.count()?;
            records.push(Record { spans, bytes: self.take(n)?.to_vec() });
        }
        Ok(records)
    }
    fn frame(&mut self) -> Result<Frame> {
        Ok(match self.byte()? {
            1 => Frame::Hello {
                protocol: self.byte()?,
                app_digest: self.take(32)?.try_into().expect("32 bytes"),
                version: self.version()?,
            },
            2 => Frame::Welcome {
                permission: match self.byte()? {
                    0 => Permission::Read,
                    1 => Permission::Write,
                    _ => return Err(invalid("unknown permission")),
                },
                version: self.version()?,
            },
            3 => Frame::Records(self.records()?),
            4 => Frame::CaughtUp,
            5 => Frame::Push { id: self.u64()?, records: self.records()? },
            6 => Frame::Ack { id: self.u64()?, status: Ack::of(self.byte()?)? },
            7 => {
                let code = self.byte()?;
                let n = self.count()?;
                let message = String::from_utf8(self.take(n)?.to_vec()).map_err(|_| invalid("message"))?;
                Frame::Error { code, message }
            }
            _ => return Err(invalid("unknown type")),
        })
    }
}
