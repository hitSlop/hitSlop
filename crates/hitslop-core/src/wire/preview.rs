//! Development-only pipe framing. The parent selects the file; pages never send paths.
use super::page::PageRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "preview.generated.ts"))]
pub enum PreviewRequest {
    Page { id: u32, request: PageRequest },
    Resource { id: u32, attachment_id: String, offset: u64, length: u64 },
}
