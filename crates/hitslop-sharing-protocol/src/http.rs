//! HTTP metadata only. Document state travels as SQLite seeds and opaque updates.
use serde::{Deserialize, Serialize};

/// Auth spike response. Never returns a bearer token or any sharing capability.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct VerifiedAccount {
    pub uid: String,
    pub auth_time: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum RoomPhase {
    Pending,
    Published,
    Stopping,
    Ended,
}

/// Private account response. Contains a recoverable bearer secret; never log it.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RoomReservation {
    pub document: String,
    pub generation: u32,
    pub room: String,
    pub capability: Option<String>,
    pub previous_room: Option<String>,
    pub phase: RoomPhase,
    pub invitation: Option<Invitation>,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RoomIdentity {
    pub document: String,
    pub generation: u32,
    pub room: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ReserveRoom {
    pub document: String,
    pub generation: u32,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AccountRoomInit {
    pub identity: RoomIdentity,
    pub init: RoomInit,
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
    Files { status: String },
    Sync { status: String },
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RoomInit {
    pub app: String,
    pub seed: Vec<(String, u32)>,
    pub seed_hash: String,
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

/// Sensitive host-owned data. Never forwarded to an authored page or printed in errors.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Invitation {
    pub document: String,
    pub room: String,
    pub origin: String,
    pub capability: String,
    pub seed_hash: String,
    pub size: u32,
}
impl Invitation {
    pub fn validate(&self) -> Result<(), String> {
        let hex = crate::resources::hash_id;
        if self.origin != crate::TRUSTED_ORIGIN
            || !crate::valid_copy(&self.document)
            || self.room.len() != 32
            || !self.room.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !hex(&self.capability)
            || !hex(&self.seed_hash)
            || self.size < 100
            || self.size as usize > crate::SEED_BYTES
        {
            return Err("Untrusted or malformed sharing link".into());
        }
        Ok(())
    }
    pub fn link(&self) -> Result<String, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| "Cannot encode sharing link")?;
        Ok(format!("{}/join#{}", crate::TRUSTED_ORIGIN, data_encoding::BASE64URL_NOPAD.encode(&bytes)))
    }
    pub fn parse(link: &str) -> Result<Self, String> {
        if link.len() > 4096 {
            return Err("Sharing link is too long".into());
        }
        let prefix = format!("{}/join#", crate::TRUSTED_ORIGIN);
        let encoded = link.trim().strip_prefix(&prefix).ok_or("Untrusted sharing link")?;
        let bytes = data_encoding::BASE64URL_NOPAD.decode(encoded.as_bytes()).map_err(|_| "Invalid sharing link")?;
        let key: Self = serde_json::from_slice(&bytes).map_err(|_| "Invalid sharing link")?;
        key.validate()?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invitations_pin_the_host_identity_limits_and_keep_capabilities_in_the_fragment() {
        let key = Invitation {
            document: "00000000-0000-4000-8000-000000000001".into(),
            room: "a".repeat(32),
            origin: crate::TRUSTED_ORIGIN.into(),
            capability: "b".repeat(64),
            seed_hash: "c".repeat(64),
            size: 4096,
        };
        let link = key.link().unwrap();
        assert_eq!(link.split('#').next(), Some("https://share.hitslop.com/join"));
        assert_eq!(Invitation::parse(&link).unwrap().capability, key.capability);
        assert!(Invitation::parse(&link.replace("share.hitslop.com", "share.hitslop.com.attacker.example")).is_err());
        assert!(Invitation::parse(&(link.clone() + "/trailing")).is_err());
        let mut other = key.clone();
        other.origin = "https://attacker.example".into();
        assert!(other.link().is_err());
        other = key.clone();
        other.size = crate::SEED_BYTES as u32 + 1;
        assert!(other.link().is_err());
        other = key;
        other.document = "..".into();
        assert!(other.link().is_err());
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
        RoomInit,
        AccountRoomInit,
        ReserveRoom,
        RoomIdentity,
        RoomReservation,
        VerifiedAccount,
        ResourceGrant,
        ApiFailure,
        crate::resources::Manifest,
        crate::resources::Resource
    )))]
    struct Description;
    let mut doc = Description::openapi();
    doc.info.title = "hitSlop sharing preview".into();
    doc.info.version = "1".into();
    // Binary files and the negotiated WebSocket remain transports of their own;
    // all JSON request and response schemas come from these Rust DTOs.
    let mut paths = serde_json::Map::new();
    for (path, methods) in [
        (
            "/v1/account/access/{document}",
            vec![("get", None, Some("RoomReservation"), "Recover the signed-in creator's room, or null when absent")],
        ),
        (
            "/v1/account/reserve",
            vec![("post", Some("ReserveRoom"), Some("RoomReservation"), "Reserve one room per document generation")],
        ),
        (
            "/v1/account/publish",
            vec![(
                "post",
                Some("RoomIdentity"),
                Some("RoomReservation"),
                "Finalize publication after all immutable files exist",
            )],
        ),
        (
            "/v1/account/stop",
            vec![(
                "post",
                Some("RoomIdentity"),
                Some("RoomReservation"),
                "Tombstone the room and clear its recoverable capability",
            )],
        ),
        (
            "/v1/auth/probe",
            vec![("get", None, Some("VerifiedAccount"), "Verify Firebase identity (developer auth spike)")],
        ),
        (
            "/v1/rooms/{room}/init",
            vec![(
                "post",
                Some("AccountRoomInit"),
                None,
                "Initialize a reserved room (verified owner identity required)",
            )],
        ),
        (
            "/v1/rooms/{room}/seed",
            vec![
                ("get", None, None, "Download the canonical SQLite seed"),
                ("put", None, None, "Publish the immutable SQLite seed (creator only)"),
            ],
        ),
        (
            "/v1/rooms/{room}/resources",
            vec![
                ("get", None, Some("Manifest"), "Read accepted app inventory"),
                ("post", Some("Manifest"), None, "Publish immutable app inventory (creator only)"),
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
            "/v1/rooms/{room}/stop",
            vec![("delete", None, None, "Tombstone the room and remove retained files (creator only)")],
        ),
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
            let mut operation = serde_json::json!({"summary":summary,"parameters":parameters,"security":[{"capability":[]}],"responses":{
                "200":{"description":"Accepted"},"403":{"$ref":"#/components/responses/Refused"},"409":{"$ref":"#/components/responses/Refused"},"410":{"$ref":"#/components/responses/Refused"},"413":{"$ref":"#/components/responses/Refused"},"429":{"$ref":"#/components/responses/Refused"}
            }});
            let creator = path.starts_with("/v1/account/")
                || path == "/v1/auth/probe"
                || path.ends_with("/init")
                || path.ends_with("/stop")
                || (method == "put" && (path.ends_with("/seed") || path.contains("/asset/")))
                || (method == "post" && path.ends_with("/resources"));
            if creator {
                operation["security"] = serde_json::json!([{"firebaseIdentity":[]}]);
                operation["responses"]["401"] = serde_json::json!({"$ref":"#/components/responses/Refused"});
            }
            if let Some(name) = input {
                operation["requestBody"] = serde_json::json!({"required":true,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{name}")}}}});
            }
            if let Some(name) = output {
                operation["responses"]["200"]["content"] = serde_json::json!({"application/json":{"schema":if name=="ResourceList"{serde_json::json!({"type":"array","items":{"$ref":"#/components/schemas/Resource"}})}else{serde_json::json!({"$ref":format!("#/components/schemas/{name}")})}}});
                if path.starts_with("/v1/account/access/") {
                    operation["responses"]["200"]["content"]["application/json"]["schema"] =
                        serde_json::json!({"anyOf":[{"type":"null"},{"$ref":"#/components/schemas/RoomReservation"}]});
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
