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
pub struct AccountRoom {
    pub room: String,
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
        if link.len() != prefix.len() + 32 + 1 + 64 {
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
        let key = Invitation { room: "a".repeat(32), capability: "b".repeat(64) };
        let link = key.link().unwrap();
        assert_eq!(link.split('#').next(), Some("https://share.tinys.click/join"));
        assert_eq!(Invitation::parse(&link).unwrap().capability, key.capability);
        for invalid in [
            link.replace("share.tinys.click", "share.tinys.click.attacker.example"),
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
        AccountRoom,
        RoomInfo,
        RoomPhase,
        ResourceGrant,
        ApiFailure,
        crate::resources::Manifest,
        crate::resources::Resource
    )))]
    struct Description;
    let mut doc = Description::openapi();
    doc.info.title = "tinys sharing".into();
    doc.info.version = "1".into();
    // Binary files and the negotiated WebSocket remain transports of their own;
    // all JSON request and response schemas come from these Rust DTOs.
    let mut paths = serde_json::Map::new();
    for (path, methods) in [
        (
            "/v1/rooms",
            vec![("post", Some("CreateRoom"), Some("CreatedRoom"), "Create or recover the document's current room")],
        ),
        (
            "/v1/account",
            vec![(
                "delete",
                None,
                None,
                "End every indexed room and remove the signed-in relay account; retryable and idempotent",
            )],
        ),
        (
            "/v1/account/rooms/{document}",
            vec![("get", None, Some("AccountRoom"), "Look up the signed-in owner's current room, or null")],
        ),
        (
            "/v1/rooms/{room}",
            vec![
                ("get", None, Some("RoomInfo"), "Read room metadata; only its owner receives the capability"),
                ("delete", None, None, "End the room and purge retained files"),
            ],
        ),
        (
            "/v1/rooms/{room}/open",
            vec![("post", None, Some("RoomInfo"), "Open a room after its seed and app resources are uploaded")],
        ),
        (
            "/v1/rooms/{room}/compact",
            vec![("post", None, None, "Replace covered updates with an opaque full-history base")],
        ),
        (
            "/v1/rooms/{room}/seed",
            vec![
                ("get", None, None, "Download the canonical SQLite seed"),
                ("put", None, None, "Replace the SQLite seed while Uploading (owner only)"),
            ],
        ),
        (
            "/v1/rooms/{room}/resources",
            vec![
                ("get", None, Some("Manifest"), "Read accepted app inventory"),
                ("post", Some("Manifest"), None, "Replace app inventory while Uploading (owner only)"),
            ],
        ),
        (
            "/v1/rooms/{room}/asset/{hash}",
            vec![("put", None, None, "Publish an immutable app resource (creator only)")],
        ),
        (
            "/v1/rooms/{room}/blob/{hash}",
            vec![("get", None, None, "Download an attachment"), ("put", None, None, "Publish an immutable attachment")],
        ),
        ("/v1/rooms/{room}/blobs", vec![("get", None, Some("ResourceList"), "Read retained attachment inventory")]),
        (
            "/_resources/{room}",
            vec![(
                "post",
                Some("ResourceGrant"),
                None,
                "Grant a short lived host-bound resource cookie on a UUID.dotslop.com origin",
            )],
        ),
    ] {
        let mut operations = serde_json::Map::new();
        for (method, input, output, summary) in methods {
            let mut parameters = vec![];
            if path.contains("{room}") {
                parameters.push(serde_json::json!({"name":"room","in":"path","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{32}$"}}));
            }
            if path.contains("{hash}") {
                parameters.push(serde_json::json!({"name":"hash","in":"path","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}}));
            }
            if path.contains("{document}") {
                parameters.push(serde_json::json!({"name":"document","in":"path","required":true,"schema":{"type":"string","format":"uuid"}}));
            }
            if method == "put" && path.ends_with("/seed") {
                for header in ["X-Sharing-SHA256", "X-Sharing-App", "X-Sharing-Version"] {
                    parameters.push(
                        serde_json::json!({"name":header,"in":"header","required":true,"schema":{"type":"string"}}),
                    );
                }
            }
            if path.ends_with("/compact") {
                for (header, description) in [
                    ("X-Sharing-SHA256", "Lowercase 64-digit hex SHA-256 digest of the body"),
                    ("X-Sharing-Version", "F: JSON array of [16-digit hex peer, non-negative i32 counter] pairs"),
                    (
                        "X-Sharing-Root",
                        "Seed version in the same encoding; required only without a stored root, otherwise ignored",
                    ),
                ] {
                    parameters.push(serde_json::json!({"name":header,"in":"header","required":header != "X-Sharing-Root","description":description,"schema":{"type":"string"}}));
                }
            }
            let mut operation = serde_json::json!({"summary":summary,"parameters":parameters,"security":[{"capability":[]}],"responses":{
                "200":{"description":"Accepted"},"403":{"$ref":"#/components/responses/Refused"},"409":{"$ref":"#/components/responses/Refused"},"410":{"$ref":"#/components/responses/Refused"},"413":{"$ref":"#/components/responses/Refused"},"429":{"$ref":"#/components/responses/Refused"}
            }});
            if path.ends_with("/compact") {
                operation["responses"].as_object_mut().unwrap().remove("200");
                operation["responses"]["204"] = serde_json::json!({"description":"Base installed"});
                for status in ["400", "404"] {
                    operation["responses"][status] = serde_json::json!({"$ref":"#/components/responses/Refused"});
                }
                operation["requestBody"] = serde_json::json!({"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary","maxLength":crate::BASE_BYTES}}}});
            }
            let creator = path.starts_with("/v1/account/")
                || path == "/v1/rooms"
                || path.ends_with("/open")
                || method == "delete"
                || (method == "put" && (path.ends_with("/seed") || path.contains("/asset/")))
                || (method == "post" && path.ends_with("/resources"));
            if creator {
                operation["security"] = serde_json::json!([{"firebaseIdentity":[]}]);
                operation["responses"]["401"] = serde_json::json!({"$ref":"#/components/responses/Refused"});
            }
            if (method == "get" && path == "/v1/rooms/{room}") || (method == "put" && path.contains("/blob/")) {
                operation["security"] = serde_json::json!([{"firebaseIdentity":[]},{"capability":[]}]);
            }
            if path == "/v1/account" {
                operation["responses"]["200"]["content"] =
                    serde_json::json!({"application/json":{"schema":{"type":"null"}}});
            }
            if let Some(name) = input {
                operation["requestBody"] = serde_json::json!({"required":true,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{name}")}}}});
            }
            if let Some(name) = output {
                operation["responses"]["200"]["content"] = serde_json::json!({"application/json":{"schema":if name=="ResourceList"{serde_json::json!({"type":"array","items":{"$ref":"#/components/schemas/Resource"}})}else{serde_json::json!({"$ref":format!("#/components/schemas/{name}")})}}});
                if path.starts_with("/v1/account/rooms/") {
                    operation["responses"]["200"]["content"]["application/json"]["schema"] =
                        serde_json::json!({"anyOf":[{"type":"null"},{"$ref":"#/components/schemas/AccountRoom"}]});
                }
            }
            operations.insert(method.into(), operation);
        }
        paths.insert(path.into(), operations.into());
    }
    doc.paths = serde_json::from_value(paths.into()).expect("static HTTP paths");
    let mut value = serde_json::to_value(doc).expect("OpenAPI document");
    value["components"]["securitySchemes"] = serde_json::json!({"capability":{"type":"http","scheme":"bearer","description":"Room edit capability for guest data access. Creator administration requires a verified Firebase identity."}});
    value["components"]["securitySchemes"]["firebaseIdentity"] = serde_json::json!({"type":"http","scheme":"bearer","bearerFormat":"JWT","description":"Firebase ID token, not a room capability"});
    value["components"]["responses"] = serde_json::json!({"Refused":{"description":"Request refused; the local document is retained","content":{"application/json":{"schema":{"$ref":"#/components/schemas/ApiFailure"}}}}});
    let doc = serde_json::from_value(value).expect("complete OpenAPI document");
    doc
}
