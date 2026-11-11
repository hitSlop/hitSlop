//! Bounded untrusted request bodies; stored JSON keeps its separate failure path.
//! Handlers match `body(..)` with `Err(error) => return error.response()`.
use crate::refusal::Refusal;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use worker::{Request, Response, console_error};

#[derive(Debug)]
pub(crate) enum InputError {
    TooLarge,
    InvalidJson,
    Read(worker::Error),
}
impl InputError {
    /// A client mistake, never a 500. Only a fixed kind is logged, never the body.
    pub(crate) fn response(self) -> worker::Result<Response> {
        let (kind, message, status, code) = match self {
            Self::TooLarge => ("too_large", "Request exceeds its byte limit", 413, "invalid_request"),
            Self::InvalidJson => ("invalid_json", "Invalid JSON request", 400, "invalid_request"),
            Self::Read(_) => ("unreadable", "Request body could not be read", 400, "invalid_request"),
        };
        console_error!("Sharing request rejected ({})", kind);
        Refusal::new(status, code, message).response()
    }
}
impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "request exceeds its byte limit",
            Self::InvalidJson => "invalid JSON request",
            Self::Read(_) => "request body could not be read",
        })
    }
}
impl std::error::Error for InputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            _ => None,
        }
    }
}
/// Metadata refusals also consume a bounded body so a normal upload can receive them.
pub(crate) async fn discard(req: &mut Request, limit: usize) {
    let Ok(mut stream) = req.stream() else {
        return;
    };
    let mut remaining = limit.saturating_add(crate::limits::REQUEST_DRAIN_BYTES);
    while let Some(Ok(chunk)) = stream.next().await {
        remaining = remaining.saturating_sub(chunk.len());
        if remaining == 0 {
            break;
        }
    }
}

pub(crate) async fn bytes(req: &mut Request, limit: usize) -> Result<Vec<u8>, InputError> {
    let mut stream = req.stream().map_err(InputError::Read)?;
    let mut body = Vec::new();
    let mut received = 0usize;
    let mut oversized = false;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(_) if oversized => return Err(InputError::TooLarge),
            Err(error) => return Err(InputError::Read(error)),
        };
        received = received.saturating_add(chunk.len());
        if received > limit {
            oversized = true;
            body.clear();
            // A bounded drain lets ordinary excess bodies finish before the 413 closes
            // the connection, without buffering or consuming an arbitrary upload.
            if received >= limit.saturating_add(crate::limits::REQUEST_DRAIN_BYTES) {
                return Err(InputError::TooLarge);
            }
        } else {
            body.extend(chunk);
        }
    }
    if oversized { Err(InputError::TooLarge) } else { Ok(body) }
}
/// The one JSON request reader: at most `limit` bytes, 413 over it, 400 when not a `T`.
pub(crate) async fn body<T: DeserializeOwned>(req: &mut Request, limit: usize) -> Result<T, InputError> {
    serde_json::from_slice(&bytes(req, limit).await?).map_err(|_| InputError::InvalidJson)
}
