//! HTTP metadata only. Document state travels as SQLite seeds and opaque updates.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum RoomPhase {
    Uploading,
    Live,
    Ended,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreateRoom {
    pub document: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreatedRoom {
    pub room: String,
    pub capability: String,
    pub phase: RoomPhase,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RoomInit {
    pub owner_uid: String,
    pub document: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RoomInfo {
    pub document: String,
    pub phase: RoomPhase,
    pub seed_hash: Option<String>,
    pub seed_size: Option<u32>,
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub capability: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub compact: bool,
}
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub struct ConnectionPlan {
    pub document: String,
    pub size: u32,
    pub url: String,
    pub headers: std::collections::BTreeMap<String, String>,
    pub socket: String,
    pub protocols: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum SharingControl {
    JoinCredential { credential: String },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum SharingEvent {
    ResourcesReady,
    Files {
        status: String,
    },
    /// `status` is for people. `phase` is what a host routes on; older workers omit it.
    Sync {
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        phase: Option<SyncPhase>,
    },
}
/// Where a replica stands with its room, apart from the words shown for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum SyncPhase {
    #[default]
    Offline,
    Syncing,
    Synced,
    /// Stopped until a person acts; the reason travels with the pause.
    Paused,
    /// The room no longer exists; the local copy is all that remains.
    Ended,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ResourceGrant {
    pub app: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ApiFailure {
    pub code: String,
    pub message: String,
}

impl ApiFailure {
    /// Public v1 refusal codes are stable; clients may also receive unfamiliar strings.
    pub fn refused(status: u16) -> Self {
        let code = match status {
            401 => "sign_in_required",
            400 | 413 | 422 => "invalid_request",
            403 => "denied",
            404 => "not_found",
            409 => "conflict",
            410 => "sharing_ended",
            429 => "limited",
            _ => "unavailable",
        };
        Self { code: code.into(), message: "The sharing service refused this request".into() }
    }
}

/// A bearer capability stays in the fragment of a pinned-origin link.
#[derive(Clone)]
pub struct Invitation {
    pub room: String,
    pub capability: String,
}
impl Invitation {
    pub fn validate(&self) -> Result<(), String> {
        if !crate::valid_room(&self.room) || !crate::resources::hash_id(&self.capability) {
            return Err("Sharing link is no longer valid".into());
        }
        Ok(())
    }
    pub fn link(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("{}/join#{}.{}", crate::TRUSTED_ORIGIN, self.room, self.capability))
    }
    pub fn parse(link: &str) -> Result<Self, String> {
        let prefix = format!("{}/join#", crate::TRUSTED_ORIGIN);
        let link = link.trim();
        if link.len() != prefix.len() + 36 + 1 + 64 {
            return Err("Sharing link is no longer valid".into());
        }
        let fragment = link.strip_prefix(&prefix).ok_or("Sharing link is no longer valid")?;
        let (room, capability) = fragment.split_once('.').ok_or("Sharing link is no longer valid")?;
        let invitation = Self { room: room.into(), capability: capability.into() };
        invitation.validate()?;
        Ok(invitation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invitations_pin_the_host_identity_limits_and_keep_capabilities_in_the_fragment() {
        let key = Invitation { room: "00000000-0000-4000-8000-000000000001".into(), capability: "b".repeat(64) };
        let link = key.link().unwrap();
        assert_eq!(link.split('#').next(), Some("https://share.dotpop.click/join"));
        assert_eq!(Invitation::parse(&link).unwrap().capability, key.capability);
        for invalid in [
            link.replace("share.dotpop.click", "share.dotpop.click.attacker.example"),
            link.clone() + "/trailing",
            link.to_uppercase(),
            link.replace("https://", "http://"),
            format!("{}/join#{}", crate::TRUSTED_ORIGIN, "a".repeat(100)),
        ] {
            assert!(Invitation::parse(&invalid).is_err());
        }
    }
    #[test]
    fn authored_origins_require_one_exact_canonical_copy_uuid() {
        let copy = "00000000-0000-4000-8000-000000000001";
        assert!(crate::is_authored_origin(&crate::authored_origin(copy).unwrap()));
        for origin in [
            "https://dotslop.com",
            "https://other.dotslop.com",
            "https://00000000-0000-4000-8000-000000000001.dotslop.com.attacker.example",
            "http://00000000-0000-4000-8000-000000000001.dotslop.com",
        ] {
            assert!(!crate::is_authored_origin(origin));
        }
    }
}

#[cfg(feature = "openapi")]
pub fn openapi() -> utoipa::openapi::OpenApi {
    use utoipa::OpenApi;
    #[derive(OpenApi)]
    #[openapi(components(schemas(
        CreateRoom,
        CreatedRoom,
        RoomInfo,
        RoomPhase,
        ResourceGrant,
        ApiFailure,
        crate::document::DocumentEntry,
        crate::document::DocumentRole,
        crate::document::AccessState,
        crate::document::JoinDocument,
        crate::document::UpdateDocument,
        crate::document::RegisteredDocument,
        crate::document::RegisteredDocuments,
        crate::document::DocumentMetadata,
        crate::resources::Manifest,
        crate::resources::Resource,
        crate::lifecycle::EpochBase,
        crate::lifecycle::EpochState,
        crate::lifecycle::StagedSeed,
        crate::lifecycle::PublishEpoch
    )))]
    struct Description;
    let mut value = serde_json::to_value(Description::openapi()).expect("OpenAPI components");
    value["info"] = serde_json::json!({"title":"pop persistent document service","version":"1"});
    let mut paths = serde_json::Map::new();
    for (path, methods) in [
        ("/v1/documents", vec![("post", Some("CreateRoom"), Some("CreatedRoom"))]),
        ("/v1/account", vec![("delete", None, None)]),
        ("/v1/account/documents", vec![("get", None, Some("RegisteredDocuments"))]),
        ("/v1/documents/{document}/metadata", vec![("get", None, Some("DocumentMetadata"))]),
        ("/v1/account/documents/{document}", vec![("get", None, Some("AccessState"))]),
        (
            "/v1/documents/{document}",
            vec![
                ("get", None, Some("RoomInfo")),
                ("patch", Some("UpdateDocument"), Some("AccessState")),
                ("delete", None, None),
            ],
        ),
        (
            "/v1/documents/{document}/sharing",
            vec![
                ("get", None, Some("AccessState")),
                ("post", None, Some("AccessState")),
                ("delete", None, Some("AccessState")),
            ],
        ),
        ("/v1/documents/{document}/members", vec![("post", Some("JoinDocument"), Some("AccessState"))]),
        ("/v1/documents/{document}/members/me", vec![("delete", None, None)]),
        ("/v1/documents/{document}/open", vec![("post", None, Some("RoomInfo"))]),
        (
            "/v1/documents/{document}/epoch",
            vec![("get", None, Some("EpochState")), ("post", Some("PublishEpoch"), Some("EpochState"))],
        ),
        ("/v1/documents/{document}/epochs/{hash}", vec![("put", None, Some("StagedSeed")), ("delete", None, None)]),
        ("/v1/documents/{document}/epochs/{seed}/blobs/{hash}", vec![("put", None, None)]),
        ("/v1/documents/{document}/seed", vec![("get", None, None), ("put", None, None)]),
        ("/v1/documents/{document}/resources", vec![("get", None, Some("Manifest")), ("post", Some("Manifest"), None)]),
        ("/v1/documents/{document}/asset/{hash}", vec![("put", None, None)]),
        ("/v1/documents/{document}/blob/{hash}", vec![("get", None, None), ("put", None, None)]),
        ("/v1/documents/{document}/blobs", vec![("get", None, None)]),
        ("/v1/documents/{document}/compact", vec![("post", None, None)]),
    ] {
        let mut operations = serde_json::Map::new();
        for (method, input, output) in methods {
            let mut op = serde_json::json!({"responses":{"200":{"description":"Accepted"},"403":{"description":"Permission denied; local copy retained"},"409":{"description":"Conditional conflict"},"410":{"description":"Document deleted"}},"security":[{"firebaseIdentity":[]},{"capability":[]}]});
            if let Some(input) = input {
                op["requestBody"] = serde_json::json!({"required":true,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{input}")}}}});
            }
            if let Some(output) = output {
                op["responses"]["200"]["content"] = serde_json::json!({"application/json":{"schema":{"$ref":format!("#/components/schemas/{output}")}}});
            }
            if path.contains("{document}") {
                op["parameters"] = serde_json::json!([{"name":"document","in":"path","required":true,"schema":{"type":"string","format":"uuid"}}]);
            }
            operations.insert(method.into(), op);
        }
        paths.insert(path.into(), operations.into());
    }
    value["paths"] = paths.into();
    value["components"]["securitySchemes"] = serde_json::json!({"firebaseIdentity":{"type":"http","scheme":"bearer","bearerFormat":"JWT"},"capability":{"type":"http","scheme":"bearer"}});
    serde_json::from_value(value).expect("document OpenAPI")
}
