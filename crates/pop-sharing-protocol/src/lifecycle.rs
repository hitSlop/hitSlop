//! Opaque persistent checkpoint publication. The service validates metadata,
//! hashes and conditional frontiers; only document owners interpret checkpoint bytes.
use crate::resources::{Manifest, Resource, hash_id};
use serde::{Deserialize, Serialize};

/// The existing seed/frame canonical peer representation, not another frame codec.
pub type WireVersion = Vec<(String, u32)>;

#[cfg(feature = "openapi")]
fn wire_version_schema() -> utoipa::openapi::schema::Schema {
    serde_json::from_value(serde_json::json!({
        "type":"array", "description":"Unique peer pairs in ascending canonical peer order",
        "items":{"type":"array","minItems":2,"maxItems":2,"items":false,
            "prefixItems":[{"type":"string","pattern":"^[0-9a-f]{16}$"},
                {"type":"integer","minimum":0,"maximum":2147483647}]}
    }))
    .expect("canonical peer tuple schema")
}

pub fn validate_version(version: &[(String, u32)]) -> Result<(), String> {
    let mut previous: Option<&str> = None;
    for (peer, end) in version {
        if peer.len() != 16
            || !peer.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || *end > i32::MAX as u32
            || previous.is_some_and(|p| p >= peer.as_str())
        {
            return Err("Invalid canonical version".into());
        }
        previous = Some(peer);
    }
    Ok(())
}

fn validate_frontier(root: &WireVersion, heads: &WireVersion) -> Result<(), String> {
    validate_version(root)?;
    validate_version(heads)?;
    let heads: std::collections::BTreeMap<_, _> = heads.iter().map(|(peer, end)| (peer.as_str(), *end)).collect();
    if root.iter().any(|(peer, end)| heads.get(peer.as_str()).is_none_or(|n| n < end)) {
        return Err("Heads precede root".into());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EpochBase {
    pub seed_hash: String,
    #[cfg_attr(feature = "openapi", schema(schema_with = wire_version_schema))]
    pub heads: WireVersion,
}
impl EpochBase {
    pub fn validate(&self) -> Result<(), String> {
        if !hash_id(&self.seed_hash) {
            return Err("Invalid seed hash".into());
        }
        validate_version(&self.heads)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EpochState {
    pub seed_hash: String,
    #[cfg_attr(feature = "openapi", schema(schema_with = wire_version_schema))]
    pub root: WireVersion,
    #[cfg_attr(feature = "openapi", schema(schema_with = wire_version_schema))]
    pub heads: WireVersion,
}

impl EpochState {
    pub fn validate(&self) -> Result<(), String> {
        if !hash_id(&self.seed_hash) {
            return Err("Invalid seed hash".into());
        }
        validate_frontier(&self.root, &self.heads)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct StagedSeed {
    pub seed_hash: String,
    pub size: u32,
    pub expires_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PublishEpoch {
    pub expected: EpochBase,
    pub seed_hash: String,
    #[cfg_attr(feature = "openapi", schema(schema_with = wire_version_schema))]
    pub root: WireVersion,
    pub resources: Manifest,
    pub attachments: Vec<Resource>,
}
impl PublishEpoch {
    pub fn validate(&self) -> Result<(), String> {
        self.expected.validate()?;
        if !hash_id(&self.seed_hash) || !hash_id(&self.resources.app) {
            return Err("Invalid publication hash".into());
        }
        validate_version(&self.root)?;
        crate::resources::validate(&self.resources.resources, false)?;
        crate::resources::validate(&self.attachments, true)?;
        if self.attachments.iter().map(|item| item.size).sum::<u64>() > crate::limits::EPOCH_STAGE_BLOB_BYTES as u64 {
            return Err("Checkpoint attachments exceed the seed bound".into());
        }
        Ok(())
    }
}
