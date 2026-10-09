//! Same-build browser host transport. Authored pages only get the page channel;
//! storage controls and evaluation results stay on the trusted host's worker channel.
use serde::{Deserialize, Serialize};

pub const PORT: u16 = 41238;
pub const PROTOCOL: u64 = 1;
pub const CONTAINER_FORMAT: u64 = 1;
pub const TRANSFER_BYTES: usize = 1 << 20;
pub const FILE_BYTES: u64 = 256 << 20;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "browser.generated.ts"))]
pub enum BrowserRequest {
    Open { copy: String, source: Option<String> },
    Page { id: String, request: String },
    Flush { id: String },
    Export { id: String },
    Resource { id: String, route: ResourceRoute, key: String, offset: Option<u32>, length: Option<u32> },
    Evaluated { output: Option<String>, error: Option<String> },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "browser.generated.ts"))]
pub enum ResourceRoute {
    App,
    Attachment,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "browser.generated.ts"))]
pub enum BrowserEvent {
    Ready { title: String, window: super::build::WindowInput },
    Reply { id: String, json: String },
    Publication { json: String },
    Save { status: BrowserSaveStatus, error: Option<String> },
    Evaluate { input: String },
    Exported { id: String, file: String },
    Resource { id: String, size: u64, media_type: String, offset: u32 },
    Error { id: Option<String>, error: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "browser.generated.ts"))]
pub enum BrowserSaveStatus {
    Saved,
    Saving,
    Failed,
}

impl BrowserRequest {
    pub fn decode(json: &str) -> crate::Result<Self> {
        use crate::{Code, err};
        if json.len() > super::SOCKET_ATTACHMENT * 2 {
            return Err(err(Code::TooLarge, "Browser request is too large"));
        }
        let request: Self =
            serde_json::from_str(json).map_err(|_| err(Code::InvalidRequest, "Invalid browser request"))?;
        let valid_id = |id: &str| !id.is_empty() && id.len() <= 128;
        let valid = match &request {
            Self::Open { copy, source } => {
                valid_copy(copy)
                    && source.as_ref().is_none_or(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            }
            Self::Page { id, request } => valid_id(id) && request.len() <= super::SOCKET_ATTACHMENT,
            Self::Flush { id } | Self::Export { id } => valid_id(id),
            Self::Resource { id, key, offset, length, .. } => {
                valid_id(id)
                    && key.len() <= super::ASSET_PATH_BYTES
                    && offset.is_none_or(|n| n as u64 <= FILE_BYTES)
                    && length.is_some_and(|n| n as usize <= TRANSFER_BYTES)
            }
            Self::Evaluated { output, error } => match (output, error) {
                (Some(output), None) => output.len() <= hitslop_runner::OUTPUT,
                (None, Some(error)) => error.len() <= 4096,
                _ => false,
            },
        };
        if !valid {
            return Err(err(Code::InvalidRequest, "Invalid browser request fields"));
        }
        Ok(request)
    }
}
pub fn valid_copy(copy: &str) -> bool {
    copy.len() == 36
        && copy.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) { b == b'-' } else { b.is_ascii_digit() || (b'a'..=b'f').contains(&b) }
        })
}
/// Browser policy is generated beside the native policy. Neither adapter rewrites CSP.
pub fn page_policy() -> String {
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' https: blob:; media-src 'self' https: blob:; frame-src https:; style-src 'self' 'unsafe-inline'; img-src 'self' data: https: blob:; font-src 'self' data:; worker-src 'self'; base-uri 'none'; frame-ancestors http://127.0.0.1:{PORT}"
    )
}
pub fn host_policy() -> String {
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; connect-src 'self'; worker-src 'self'; img-src 'self' blob:; frame-src http://*.localhost:{PORT}; base-uri 'none'; frame-ancestors 'none'"
    )
}
