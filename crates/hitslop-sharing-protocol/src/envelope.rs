//! Version-one serde/postcard envelope: opaque payloads, one update per push.
use crate::frame::{Ack, Frame, MAX_FRAME, Permission, Record, Span, Version};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Wire {
    Hello { protocol: u8, app: [u8; 32], vv: Vec<(u64, u32)> },
    Welcome { write: bool, vv: Vec<(u64, u32)> },
    Records(Vec<Update>),
    CaughtUp,
    Push { id: u64, update: Update },
    Ack { id: u64, status: Outcome },
    Error { code: u8, message: String },
}
#[derive(Serialize, Deserialize)]
struct Update {
    spans: Vec<(u64, u32, u32)>,
    #[serde(with = "serde_bytes")]
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
enum Outcome {
    Stored,
    Gap,
    TooLarge,
    Denied,
    Quota,
}
fn version(v: Vec<(u64, u32)>) -> Result<Version, String> {
    if v.windows(2).any(|w| w[0].0 >= w[1].0) || v.iter().any(|(_, n)| *n > i32::MAX as u32) {
        return Err("Invalid version".into());
    }
    Ok(v.into_iter().collect())
}
impl From<Record> for Update {
    fn from(r: Record) -> Self {
        Self { spans: r.spans.into_iter().map(|s| (s.peer, s.start, s.end)).collect(), bytes: r.bytes }
    }
}
impl Update {
    fn record(self) -> Result<Record, String> {
        if self.spans.is_empty() || self.spans.iter().any(|(_, s, e)| s >= e || *e > i32::MAX as u32) {
            return Err("Invalid spans".into());
        }
        Ok(Record {
            spans: self.spans.into_iter().map(|(peer, start, end)| Span { peer, start, end }).collect(),
            bytes: self.bytes,
        })
    }
}
pub fn encode(frame: Frame) -> Result<Vec<u8>, String> {
    let vv = |v: Version| v.into_iter().collect();
    let wire = match frame {
        Frame::Hello { protocol, app_digest, version } => Wire::Hello { protocol, app: app_digest, vv: vv(version) },
        Frame::Welcome { permission, version } => {
            Wire::Welcome { write: permission == Permission::Write, vv: vv(version) }
        }
        Frame::Records(records) => Wire::Records(records.into_iter().map(Into::into).collect()),
        Frame::CaughtUp => Wire::CaughtUp,
        Frame::Push { id, mut records } => {
            if records.len() != 1 {
                return Err("One update per push".into());
            }
            Wire::Push { id, update: records.remove(0).into() }
        }
        Frame::Ack { id, status } => Wire::Ack {
            id,
            status: match status {
                Ack::Stored => Outcome::Stored,
                Ack::Gap => Outcome::Gap,
                Ack::TooLarge => Outcome::TooLarge,
                Ack::Denied => Outcome::Denied,
                Ack::Quota => Outcome::Quota,
            },
        },
        Frame::Error { code, message } => Wire::Error { code, message },
    };
    let bytes = postcard::to_allocvec(&wire).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME {
        return Err("Frame too large".into());
    }
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<Frame, String> {
    if bytes.len() > MAX_FRAME {
        return Err("Frame too large".into());
    }
    let (wire, rest) = postcard::take_from_bytes::<Wire>(bytes).map_err(|e| e.to_string())?;
    if !rest.is_empty() {
        return Err("Trailing bytes".into());
    }
    Ok(match wire {
        Wire::Hello { protocol, app, vv } => Frame::Hello { protocol, app_digest: app, version: version(vv)? },
        Wire::Welcome { write, vv } => Frame::Welcome {
            permission: if write { Permission::Write } else { Permission::Read },
            version: version(vv)?,
        },
        Wire::Records(records) => Frame::Records(records.into_iter().map(Update::record).collect::<Result<_, _>>()?),
        Wire::CaughtUp => Frame::CaughtUp,
        Wire::Push { id, update } => Frame::Push { id, records: vec![update.record()?] },
        Wire::Ack { id, status } => Frame::Ack {
            id,
            status: match status {
                Outcome::Stored => Ack::Stored,
                Outcome::Gap => Ack::Gap,
                Outcome::TooLarge => Ack::TooLarge,
                Outcome::Denied => Ack::Denied,
                Outcome::Quota => Ack::Quota,
            },
        },
        Wire::Error { code, message } => Frame::Error { code, message },
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_lengths_and_extra_bytes_are_refused() {
        let valid = encode(Frame::Error { code: 1, message: String::new() }).unwrap();
        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_err());
        let mut overflow = valid[..2].to_vec();
        overflow.extend([0x80; 9]);
        overflow.push(2);
        assert!(decode(&overflow).is_err());
        for end in 0..valid.len() {
            assert!(decode(&valid[..end]).is_err());
        }
        let huge_count = [2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1];
        assert!(std::panic::catch_unwind(|| decode(&huge_count)).unwrap().is_err());
    }
}
