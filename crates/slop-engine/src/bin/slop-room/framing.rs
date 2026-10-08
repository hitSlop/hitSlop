//! Private loopback framing. A payload is bounded before allocation, hashed before
//! installation, and transferred in bounded chunks instead of JSON/base64 copies.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

pub const MAX_PAYLOAD: usize = 32 * 1024 * 1024;
pub const MAX_HEADER: usize = 64 * 1024;
pub const CHUNK: usize = 64 * 1024;
const MAGIC: &[u8; 8] = b"SLOPDEV1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header<T> {
    message: T,
    length: usize,
    sha256: String,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
pub fn write<T: Serialize>(stream: &mut impl Write, message: &T, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_PAYLOAD {
        return Err(invalid("Development sync payload exceeds 32 MiB"));
    }
    let header = serde_json::to_vec(&Header {
        message,
        length: payload.len(),
        sha256: data_encoding::HEXLOWER.encode(&Sha256::digest(payload)),
    })?;
    if header.len() > MAX_HEADER {
        return Err(invalid("Development sync header exceeds 64 KiB"));
    }
    stream.write_all(MAGIC)?;
    stream.write_all(&(header.len() as u32).to_be_bytes())?;
    stream.write_all(&header)?;
    for chunk in payload.chunks(CHUNK) {
        stream.write_all(&(chunk.len() as u32).to_be_bytes())?;
        stream.write_all(chunk)?;
    }
    stream.flush()
}
pub fn read<T: serde::de::DeserializeOwned>(stream: &mut impl Read) -> io::Result<(T, Vec<u8>)> {
    let mut magic = [0; 8];
    stream.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(invalid("Not a development sync frame"));
    }
    let mut word = [0; 4];
    stream.read_exact(&mut word)?;
    let length = u32::from_be_bytes(word) as usize;
    if length == 0 || length > MAX_HEADER {
        return Err(invalid("Invalid development sync header length"));
    }
    let mut header = vec![0; length];
    stream.read_exact(&mut header)?;
    let header: Header<T> = serde_json::from_slice(&header)?;
    if header.length > MAX_PAYLOAD {
        return Err(invalid("Development sync payload exceeds 32 MiB"));
    }
    let mut payload = Vec::with_capacity(header.length);
    while payload.len() < header.length {
        stream.read_exact(&mut word)?;
        let length = u32::from_be_bytes(word) as usize;
        if length == 0 || length > CHUNK || length > header.length - payload.len() {
            return Err(invalid("Invalid development sync chunk length"));
        }
        let start = payload.len();
        payload.resize(start + length, 0);
        stream.read_exact(&mut payload[start..])?;
    }
    if data_encoding::HEXLOWER.encode(&Sha256::digest(&payload)) != header.sha256 {
        return Err(invalid("Development sync payload digest mismatch"));
    }
    Ok((header.message, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximum_checkpoint_round_trips_without_a_giant_frame() {
        let payload = vec![0xa7; MAX_PAYLOAD];
        let mut bytes = vec![];
        write(&mut bytes, &"snapshot", &payload).unwrap();
        let (message, received): (String, _) = read(&mut bytes.as_slice()).unwrap();
        assert_eq!(message, "snapshot");
        assert_eq!(received, payload);
    }
    #[test]
    fn refuses_truncation_corruption_and_oversized_headers_before_install() {
        let mut bytes = vec![];
        write(&mut bytes, &"update", b"abc").unwrap();
        for length in [0, 8, 12, bytes.len() - 1] {
            assert!(read::<String>(&mut &bytes[..length]).is_err());
        }
        *bytes.last_mut().unwrap() ^= 1;
        assert!(read::<String>(&mut bytes.as_slice()).is_err());
        let mut invalid = MAGIC.to_vec();
        invalid.extend_from_slice(&((MAX_HEADER + 1) as u32).to_be_bytes());
        assert!(read::<String>(&mut invalid.as_slice()).is_err());
    }
    #[test]
    fn refuses_payload_length_before_allocation() {
        let header =
            serde_json::to_vec(&Header { message: "snapshot", length: MAX_PAYLOAD + 1, sha256: String::new() })
                .unwrap();
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&(header.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&header);
        assert!(read::<String>(&mut bytes.as_slice()).is_err());
    }
}
