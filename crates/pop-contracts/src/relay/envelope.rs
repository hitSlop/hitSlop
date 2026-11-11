//! Socket transport: the shared Frame codec, one update per push, within `MAX_FRAME`.
use super::frame::{Frame, MAX_FRAME};

fn transport_frame(frame: &Frame) -> Result<(), String> {
    if matches!(frame, Frame::Push { records, .. } if records.len() != 1) {
        return Err("One update per push".into());
    }
    Ok(())
}
pub fn encode(frame: Frame) -> Result<Vec<u8>, String> {
    transport_frame(&frame)?;
    let bytes = frame.encode();
    if bytes.len() > MAX_FRAME {
        return Err("Frame too large".into());
    }
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<Frame, String> {
    let frame = Frame::decode(bytes).map_err(|error| error.to_string())?;
    transport_frame(&frame)?;
    Ok(frame)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::frame::{Ack, Permission, Record, Span};
    #[test]
    fn transport_uses_the_shared_frame_codec() {
        let record = Record { spans: vec![Span { peer: 7, start: 1, end: 2 }], bytes: vec![12, 13] };
        let frames = [
            Frame::Hello {
                protocol: 1,
                app_digest: crate::AppDigest::from_bytes([3; 32]),
                version: [(7, 2)].into_iter().collect(),
            },
            Frame::Welcome { permission: Permission::Write, version: [(7, 2)].into_iter().collect() },
            Frame::Records(vec![record.clone()]),
            Frame::CaughtUp,
            Frame::Push { id: 9, records: vec![record] },
            Frame::Ack { id: 9, status: Ack::Stored },
            Frame::Error { code: 255, message: "opaque".into() },
        ];
        for frame in frames {
            let bytes = frame.encode();
            assert_eq!(encode(frame.clone()).unwrap(), bytes);
            assert_eq!(decode(&bytes).unwrap(), frame);
        }
    }
    #[test]
    fn transport_keeps_one_update_per_push_and_frame_bounds() {
        let record = Record { spans: vec![Span { peer: 7, start: 1, end: 2 }], bytes: vec![12] };
        for records in [vec![], vec![record.clone(), record]] {
            let frame = Frame::Push { id: 1, records };
            assert!(decode(&frame.encode()).is_err());
            assert!(encode(frame).is_err());
        }
        assert!(decode(&vec![0; MAX_FRAME + 1]).is_err());
        let invalid =
            Frame::Records(vec![Record { spans: vec![Span { peer: 7, start: 0, end: u32::MAX }], bytes: vec![12] }]);
        assert!(decode(&invalid.encode()).is_err());
    }
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
