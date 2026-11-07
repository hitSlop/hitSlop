//! Development-only pipe framing. The parent selects the file; pages never send paths.
use super::page::PageRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "preview.generated.ts"))]
pub enum PreviewRequest {
    Page { id: u32, request: PageRequest },
    ResourcePlan { id: u32, attachment_id: String, range: Option<String> },
    Resource { id: u32, attachment_id: String, offset: u64, length: u64 },
}

/// Same-build owner output; authored pages never receive resource or lifecycle controls.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "preview.generated.ts"))]
#[cfg_attr(feature = "ts", ts(concrete(P = super::page::PagePush)))]
pub enum PreviewEvent<P = super::page::PagePush> {
    Ready {
        pid: u32,
    },
    Reply {
        id: u32,
        reply: String,
    },
    Push {
        pushes: Vec<P>,
    },
    Save {
        status: super::browser::BrowserSaveStatus,
        error: Option<String>,
    },
    Resource {
        id: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        plan: Option<crate::resources::ResponsePlan>,
        info: Option<PreviewResource>,
        bytes: Option<String>,
        error: Option<String>,
    },
    Fatal {
        error: super::engine::Failure,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "preview.generated.ts"))]
pub struct PreviewResource {
    pub size: u64,
    pub mime_type: String,
}
