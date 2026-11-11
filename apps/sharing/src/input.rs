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
pub(crate) async fn bytes(req: &mut Request, limit: usize) -> Result<Vec<u8>, InputError> {
    let mut stream = req.stream().map_err(InputError::Read)?;
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(InputError::Read)?;
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(InputError::TooLarge);
        }
        body.extend(chunk);
    }
    Ok(body)
}
/// The one JSON request reader: at most `limit` bytes, 413 over it, 400 when not a `T`.
pub(crate) async fn body<T: DeserializeOwned>(req: &mut Request, limit: usize) -> Result<T, InputError> {
    serde_json::from_slice(&bytes(req, limit).await?).map_err(|_| InputError::InvalidJson)
}
