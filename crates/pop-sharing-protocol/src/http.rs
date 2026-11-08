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

/// The seed an upload makes live. The relay refuses (409) one that is not the seed it
/// stores, so an older attempt cannot publish a seed uploaded after it. The relay does not
/// check attachments: the creator uploads every attachment the seed references first.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct OpenRoom {
    /// Lowercase hex SHA-256 of the uploaded seed bytes.
    pub seed_sha256: String,
}

/// A document's seed and app, read by any member. Access and the invitation secret are
/// only in `AccessState`.
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
    /// Whether retained updates have reached the size at which a member may compact.
    pub compact: bool,
}
/// What a browser copy joins: the invitation's document, checked against its metadata,
/// and the seed's declared size. Routes are `/v1/documents/{document}/...`.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub struct ConnectionPlan {
    pub document: String,
    pub size: u32,
}
/// How the trusted host opens a browser copy: through its invitation, or as a local copy
/// that no longer syncs.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum SharingControl {
    JoinInvitation { invitation: String },
    OpenLocal,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum SharingEvent {
    IdentityAttention {
        message: String,
    },
    ResourcesReady,
    Files {
        status: String,
    },
    /// `status` is for people. `phase` is what a host routes on.
    Sync {
        status: String,
        phase: SyncPhase,
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

/// An invitation secret stays in the fragment of a pinned-origin link.
/// It grants membership only through the authenticated members exchange.
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

#[cfg(feature = "openapi")]
fn openapi_paths() -> serde_json::Map<String, serde_json::Value> {
    use crate::routes::{Actor, Payload, ROUTES, Scope, parameters};
    use serde_json::json;
    let failure = |description: &str| json!({"description":description,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/ApiFailure"}}}});
    let content = |payload: Payload| -> Option<serde_json::Value> {
        let schema = match payload {
            Payload::None | Payload::Empty => return None,
            Payload::Json(name) => json!({"$ref":format!("#/components/schemas/{name}")}),
            Payload::JsonArray(name) => json!({"type":"array","items":{"$ref":format!("#/components/schemas/{name}")}}),
            Payload::Bytes => {
                return Some(json!({"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}));
            }
        };
        Some(json!({"application/json":{"schema":schema}}))
    };
    let mut paths = serde_json::Map::new();
    for route in ROUTES {
        let mut responses = serde_json::Map::new();
        if route.op == crate::routes::Op::Sync {
            responses.insert("101".into(), json!({"description":"Switching Protocols to opaque update frames"}));
        } else if route.response == Payload::Empty {
            responses.insert("204".into(), json!({"description":"Accepted; no content"}));
        } else {
            let mut ok = json!({"description":"Accepted"});
            if let Some(content) = content(route.response) {
                ok["content"] = content;
            }
            responses.insert("200".into(), ok);
        }
        let mut operation = json!({"summary":route.summary,"operationId":format!("{:?}",route.op)});
        if route.request != Payload::None {
            operation["requestBody"] = json!({"required":true,"content":content(route.request)});
            responses.insert("400".into(), failure("Invalid request"));
            responses.insert("413".into(), failure("Request exceeds its byte limit"));
        }
        if route.actor != Actor::Public {
            responses.insert("401".into(), failure("Sign in required"));
            responses.insert("403".into(), failure("Permission denied; local copy retained"));
            responses.insert("429".into(), failure("Rate or quota limit"));
            operation["security"] = json!([{"firebaseIdentity":[]}]);
        }
        if route.scope == Scope::Document {
            responses.insert("404".into(), failure("Unknown document or route"));
            responses.insert("409".into(), failure("Conditional conflict"));
            responses.insert("410".into(), failure("Document deleted"));
        }
        operation["responses"] = responses.into();
        // `document` names a document and the `after` cursor is the last one listed.
        let schema = |name: &str| match name {
            "document" | "after" => json!({"type":"string","format":"uuid"}),
            _ => json!({"type":"string"}),
        };
        let parameters: Vec<_> = parameters(route.path)
            .map(|name| json!({"name":name,"in":"path","required":true,"schema":schema(name)}))
            .chain(
                route.query.iter().map(|name| json!({"name":name,"in":"query","required":false,"schema":schema(name)})),
            )
            .collect();
        if !parameters.is_empty() {
            operation["parameters"] = parameters.into();
        }
        let entry = paths.entry(route.path.to_string()).or_insert_with(|| json!({}));
        entry[route.method.to_ascii_lowercase()] = operation;
    }
    paths
}

#[cfg(feature = "openapi")]
pub fn openapi() -> utoipa::openapi::OpenApi {
    use utoipa::OpenApi;
    #[derive(OpenApi)]
    #[openapi(components(schemas(
        CreateRoom,
        OpenRoom,
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
        crate::resources::Resource
    )))]
    struct Description;
    let mut value = serde_json::to_value(Description::openapi()).expect("OpenAPI components");
    value["info"] = serde_json::json!({"title":"pop persistent document service","version":"1"});
    value["paths"] = openapi_paths().into();
    value["components"]["securitySchemes"] =
        serde_json::json!({"firebaseIdentity":{"type":"http","scheme":"bearer","bearerFormat":"JWT"}});
    serde_json::from_value(value).expect("document OpenAPI")
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
    #[cfg(feature = "openapi")]
    #[test]
    fn openapi_describes_every_route_with_its_parameters_and_refusals() {
        let value = serde_json::to_value(openapi()).unwrap();
        for route in crate::routes::ROUTES {
            let operation = &value["paths"][route.path][route.method.to_ascii_lowercase()];
            assert!(operation.is_object(), "{} {}", route.method, route.path);
            let declared: Vec<_> = operation["parameters"]
                .as_array()
                .map(|all| all.iter().map(|p| (p["in"].as_str().unwrap(), p["name"].as_str().unwrap())).collect())
                .unwrap_or_default();
            let expected: Vec<_> = crate::routes::parameters(route.path)
                .map(|name| ("path", name))
                .chain(route.query.iter().map(|name| ("query", *name)))
                .collect();
            assert_eq!(declared, expected, "{}", route.path);
        }
        let created = &value["paths"]["/v1/documents"]["post"]["responses"];
        assert!(created["204"].is_object() && created["200"].is_null(), "creation replies with no body");
        let upload = &value["paths"]["/v1/documents/{document}/blob/{hash}"]["put"]["responses"];
        for status in ["200", "401", "413", "429"] {
            assert!(upload[status].is_object(), "{status}");
        }
        assert!(value["paths"]["/v1/documents/{document}/sync"]["get"]["responses"]["101"].is_object());
    }
    #[test]
    fn authored_origins_require_one_exact_canonical_copy_uuid() {
        let copy = "00000000-0000-4000-8000-000000000001";
        assert!(crate::is_authored_origin(&format!("https://{copy}.{}", crate::AUTHORED_DOMAIN)));
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
