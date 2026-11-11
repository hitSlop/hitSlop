//! Public HTTP refusals are serialized once, at the decision boundary.
#[cfg(target_arch = "wasm32")]
use pop_sharing_protocol::http::ApiFailure;
#[cfg(target_arch = "wasm32")]
use worker::{Response, Result};

#[derive(Debug, PartialEq, Eq)]
pub struct Refusal {
    pub(crate) status: u16,
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}
impl Refusal {
    pub(crate) const fn new(status: u16, code: &'static str, message: &'static str) -> Self {
        Self { status, code, message }
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn response(self) -> Result<Response> {
        Ok(Response::from_json(&ApiFailure { code: self.code.into(), message: self.message.into() })?
            .with_status(self.status))
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for Refusal {}
