//! Same-build owner socket messages; protocol refusal precedes payload decoding.
use super::engine::{EngineRequest, check_path};
use crate::{Code, Error, err};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub struct SocketRequest {
    pub protocol: u64,
    pub request: EngineRequest,
}
impl SocketRequest {
    /// Bound the transport, negotiate before interpreting the request, then apply
    /// the same request validation as the engine and the socket's narrower admission.
    pub fn decode(input: &str) -> Result<Self, Error> {
        if input.len() > super::SOCKET_ATTACHMENT {
            return Err(err(Code::InvalidRequest, "Oversized socket request"));
        }
        check_protocol(input)?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Envelope {
            protocol: u64,
            request: Box<serde_json::value::RawValue>,
        }
        let envelope: Envelope = serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))?;
        let request = EngineRequest::parse(envelope.request.get()).map_err(|e| err(Code::InvalidRequest, e))?;
        if !request.is_document() {
            return Err(err(Code::InvalidRequest, "Not a document command"));
        }
        if input.len() > request.socket_limit() {
            return Err(err(Code::InvalidRequest, "Oversized socket request"));
        }
        Ok(Self { protocol: envelope.protocol, request })
    }
}

/// The only fields a discovery reader interprets. Other fields are always ignored,
/// including values a future protocol could not represent in this build.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub document_path: String,
    pub socket: String,
}
impl Discovery {
    pub fn decode(input: &str) -> Result<Self, Error> {
        if input.len() > super::SOCKET_ATTACHMENT {
            return Err(err(Code::InvalidRequest, "Oversized discovery"));
        }
        let value: Self = serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))?;
        check_path(&value.socket)
            .and_then(|()| check_path(&value.document_path))
            .map_err(|e| err(Code::InvalidRequest, e))?;
        Ok(value)
    }
}
#[derive(Deserialize)]
pub(crate) struct ProtocolHeader {
    pub protocol: u64,
}
pub(crate) fn check_protocol(input: &str) -> Result<(), Error> {
    let header: ProtocolHeader = serde_json::from_str(input).map_err(|e| err(Code::InvalidRequest, e))?;
    if let Some(message) = crate::command::protocol_mismatch(header.protocol) {
        return Err(err(Code::RequiresUpdate, message));
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum Hello {
    #[serde(rename = "hello")]
    Hello { protocol: u64 },
}
#[derive(Deserialize)]
#[serde(tag = "method")]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum HelloSuccess {
    #[serde(rename = "hello")]
    Hello {
        #[serde(rename = "ok")]
        _ok: super::engine::True,
    },
}
