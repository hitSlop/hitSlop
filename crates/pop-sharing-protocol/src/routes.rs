//! The one table of public service routes. The Worker's dispatch, its rate-limit
//! classification and the generated OpenAPI all read `ROUTES`; a request that matches
//! no row is a 404 before any handler or limiter is reached.
//!
//! The authored-domain resource host (`/assets`, `/attachments`, `/_resources`) is a
//! separate surface with its own cookie authorization and is not described here.

/// One operation per (method, path). Handlers match on this, never on path text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Health,
    CreateDocument,
    DeleteAccount,
    ListAccountDocuments,
    GetDocument,
    PatchDocument,
    DeleteDocument,
    GetMetadata,
    GetSharing,
    EnableSharing,
    DisableSharing,
    Join,
    Leave,
    Open,
    GetSeed,
    PutSeed,
    GetResources,
    PutResources,
    PutAsset,
    ListBlobs,
    GetBlob,
    PutBlob,
    Compact,
    Sync,
}

/// Which Durable Object answers: none, the caller's account, or the addressed document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Service,
    Account,
    Document,
}

/// Who may call. The owning handler enforces it against current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Public,
    /// Any verified Firebase identity, including anonymous.
    Identity,
    /// A named (non-anonymous) Firebase identity.
    Named,
    /// The document owner.
    Owner,
    /// The owner or an editor admitted by the current invitation generation.
    Member,
}

/// Per-IP rate-limit bucket (a Worker `ratelimits` binding), or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    None,
    Metadata,
    Seed,
    Connect,
    Blob,
    Compact,
}
impl Limit {
    pub fn binding(self) -> Option<&'static str> {
        Some(match self {
            Self::None => return None,
            Self::Metadata => "GUEST_METADATA_LIMIT",
            Self::Seed => "GUEST_SEED_LIMIT",
            Self::Connect => "GUEST_CONNECT_LIMIT",
            Self::Blob => "GUEST_BLOB_LIMIT",
            Self::Compact => "GUEST_COMPACT_LIMIT",
        })
    }
}

/// A request or response body, by schema name in the generated components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Payload {
    /// No body the client reads; a reply is 200 with a short plain-text status.
    None,
    /// A 204 reply with no body.
    Empty,
    Json(&'static str),
    JsonArray(&'static str),
    Bytes,
}

#[derive(Debug)]
pub struct Route {
    pub op: Op,
    pub method: &'static str,
    /// `{document}` is a canonical UUID; `{hash}` is a single non-empty segment.
    pub path: &'static str,
    pub scope: Scope,
    pub actor: Actor,
    pub limit: Limit,
    pub request: Payload,
    pub response: Payload,
    pub summary: &'static str,
    /// Optional query parameters the handler reads. They never take part in matching.
    pub query: &'static [&'static str],
}
impl Route {
    const fn with_query(self, query: &'static [&'static str]) -> Self {
        Route { query, ..self }
    }
}

#[allow(clippy::too_many_arguments, reason = "one flat table row; named fields would triple the table")]
const fn route(
    op: Op,
    method: &'static str,
    path: &'static str,
    scope: Scope,
    actor: Actor,
    limit: Limit,
    request: Payload,
    response: Payload,
    summary: &'static str,
) -> Route {
    Route { op, method, path, scope, actor, limit, request, response, summary, query: &[] }
}

use Actor::*;
use Limit as L;
use Payload::{Bytes, Json, JsonArray};
use Scope::*;
const NO: Payload = Payload::None;

pub static ROUTES: &[Route] = &[
    route(Op::Health, "GET", "/health", Service, Public, L::None, NO, NO, "Liveness probe."),
    route(
        Op::CreateDocument,
        "POST",
        "/v1/documents",
        Account,
        Named,
        L::None,
        Json("CreateRoom"),
        Payload::Empty,
        "Idempotently reserve a document UUID. Requires a non-anonymous Firebase identity; anonymous identities join through the members exchange.",
    ),
    route(
        Op::DeleteAccount,
        "DELETE",
        "/v1/account",
        Account,
        Identity,
        L::None,
        NO,
        NO,
        "Fence the account, then delete owned documents and leave joined ones.",
    ),
    route(
        Op::ListAccountDocuments,
        "GET",
        "/v1/account/documents",
        Account,
        Identity,
        L::None,
        NO,
        Json("RegisteredDocuments"),
        "List registered document UUIDs and roles, a hint for repairing a missing catalog reference. Pages in document order: pass a page's `next` as `after` until it is absent.",
    )
    .with_query(&["after"]),
    route(
        Op::GetDocument,
        "GET",
        "/v1/documents/{document}",
        Document,
        Member,
        L::Metadata,
        NO,
        Json("RoomInfo"),
        "Current seed and app metadata.",
    ),
    route(
        Op::PatchDocument,
        "PATCH",
        "/v1/documents/{document}",
        Document,
        Owner,
        L::None,
        Json("UpdateDocument"),
        Json("AccessState"),
        "Store display title and template ID.",
    ),
    route(
        Op::DeleteDocument,
        "DELETE",
        "/v1/documents/{document}",
        Document,
        Owner,
        L::None,
        NO,
        NO,
        "Revoke access, purge payloads and keep a deletion tombstone.",
    ),
    route(
        Op::GetMetadata,
        "GET",
        "/v1/documents/{document}/metadata",
        Document,
        Member,
        L::None,
        NO,
        Json("DocumentMetadata"),
        "Title and template ID retained for catalog repair.",
    ),
    route(
        Op::GetSharing,
        "GET",
        "/v1/documents/{document}/sharing",
        Document,
        Member,
        L::None,
        NO,
        Json("AccessState"),
        "Current role and sharing status. Only the owner receives the capability.",
    ),
    route(
        Op::EnableSharing,
        "POST",
        "/v1/documents/{document}/sharing",
        Document,
        Owner,
        L::None,
        NO,
        Json("AccessState"),
        "Enable an invitation idempotently.",
    ),
    route(
        Op::DisableSharing,
        "DELETE",
        "/v1/documents/{document}/sharing",
        Document,
        Owner,
        L::None,
        NO,
        Json("AccessState"),
        "Revoke the invitation and its memberships; the document is preserved.",
    ),
    route(
        Op::Join,
        "POST",
        "/v1/documents/{document}/members",
        Document,
        Identity,
        L::None,
        Json("JoinDocument"),
        Json("AccessState"),
        "Exchange an invitation secret for membership under the verified Firebase identity, including anonymous identities. The secret is not a bearer credential.",
    ),
    route(
        Op::Leave,
        "DELETE",
        "/v1/documents/{document}/members/me",
        Document,
        Member,
        L::None,
        NO,
        NO,
        "Leave without deleting the document.",
    ),
    route(
        Op::Open,
        "POST",
        "/v1/documents/{document}/open",
        Document,
        Owner,
        L::None,
        Json("OpenRoom"),
        Json("RoomInfo"),
        "Make the named seed live (409 for any other). Attachments are not checked: upload them first.",
    ),
    route(
        Op::GetSeed,
        "GET",
        "/v1/documents/{document}/seed",
        Document,
        Member,
        L::Seed,
        NO,
        Bytes,
        "The canonical SQLite seed.",
    ),
    route(
        Op::PutSeed,
        "PUT",
        "/v1/documents/{document}/seed",
        Document,
        Owner,
        L::None,
        Bytes,
        NO,
        "Upload the seed while the document is uploading.",
    ),
    route(
        Op::GetResources,
        "GET",
        "/v1/documents/{document}/resources",
        Document,
        Member,
        L::None,
        NO,
        Json("Manifest"),
        "The app resource manifest.",
    ),
    route(
        Op::PutResources,
        "POST",
        "/v1/documents/{document}/resources",
        Document,
        Owner,
        L::None,
        Json("Manifest"),
        NO,
        "Declare the app resource manifest.",
    ),
    route(
        Op::PutAsset,
        "PUT",
        "/v1/documents/{document}/asset/{hash}",
        Document,
        Owner,
        L::None,
        Bytes,
        NO,
        "Upload one manifest resource.",
    ),
    route(
        Op::ListBlobs,
        "GET",
        "/v1/documents/{document}/blobs",
        Document,
        Member,
        L::None,
        NO,
        JsonArray("Resource"),
        "The attachment inventory.",
    ),
    route(
        Op::GetBlob,
        "GET",
        "/v1/documents/{document}/blob/{hash}",
        Document,
        Member,
        L::None,
        NO,
        Bytes,
        "An attachment listed in the inventory.",
    ),
    route(
        Op::PutBlob,
        "PUT",
        "/v1/documents/{document}/blob/{hash}",
        Document,
        Member,
        L::Blob,
        Bytes,
        NO,
        "Upload an immutable attachment (owner while uploading).",
    ),
    route(
        Op::Compact,
        "POST",
        "/v1/documents/{document}/compact",
        Document,
        Member,
        L::Compact,
        Bytes,
        Payload::Empty,
        "Replace the retained base with a client-built one.",
    ),
    route(
        Op::Sync,
        "GET",
        "/v1/documents/{document}/sync",
        Document,
        Member,
        L::Connect,
        NO,
        NO,
        "WebSocket upgrade for opaque update frames.",
    ),
];

/// A request resolved against `ROUTES`.
#[derive(Clone, Copy, Debug)]
pub struct Matched<'a> {
    pub route: &'static Route,
    pub document: Option<&'a str>,
    pub hash: Option<&'a str>,
}

/// Resolve a request. Anything unlisted, including an extra trailing segment, is `None`.
pub fn find<'a>(method: &str, path: &'a str) -> Option<Matched<'a>> {
    ROUTES.iter().filter(|route| route.method == method).find_map(|route| {
        let mut found = Matched { route, document: None, hash: None };
        let mut actual = path.split('/');
        for pattern in route.path.split('/') {
            let segment = actual.next()?;
            match pattern {
                "{document}" if crate::valid_copy(segment) => found.document = Some(segment),
                "{hash}" if !segment.is_empty() => found.hash = Some(segment),
                literal if literal == segment && !literal.starts_with('{') => {}
                _ => return None,
            }
        }
        actual.next().is_none().then_some(found)
    })
}

/// `{name}` placeholders of a pattern, in order.
pub fn parameters(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
}

#[cfg(test)]
mod tests {
    use super::*;
    const DOCUMENT: &str = "00000000-0000-4000-8000-000000000001";
    fn concrete(path: &str) -> String {
        path.replace("{document}", DOCUMENT).replace("{hash}", "hash1")
    }
    #[test]
    fn every_route_resolves_to_itself_and_refuses_any_extra_segment() {
        for route in ROUTES {
            let path = concrete(route.path);
            let found = find(route.method, &path).unwrap_or_else(|| panic!("{} {}", route.method, route.path));
            assert!(std::ptr::eq(found.route, route), "{} {}", route.method, route.path);
            for suffix in ["/extra", "/", "/seed/extra"] {
                assert!(find(route.method, &format!("{path}{suffix}")).is_none(), "{path}{suffix}");
            }
        }
    }
    #[test]
    fn methods_and_paths_are_unique_and_unknown_requests_match_nothing() {
        for (index, a) in ROUTES.iter().enumerate() {
            for b in &ROUTES[index + 1..] {
                assert!(a.op != b.op && (a.method, a.path) != (b.method, b.path), "{:?}", a.op);
            }
        }
        for (method, path) in [
            ("GET", "/v1/documents/invalid"),
            ("GET", "/v1/account"),
            ("GET", format!("/v1/account/documents/{DOCUMENT}").as_str()),
            ("POST", format!("/v1/documents/{DOCUMENT}/members/me").as_str()),
            ("DELETE", format!("/v1/documents/{DOCUMENT}/members").as_str()),
            ("PUT", format!("/v1/documents/{DOCUMENT}/blob/").as_str()),
            ("GET", format!("/v1/documents/{DOCUMENT}/").as_str()),
            ("GET", "/v1/documents//seed"),
        ] {
            assert!(find(method, path).is_none(), "{method} {path}");
        }
    }
    #[test]
    fn parameters_are_captured_and_limits_follow_the_route() {
        let path = format!("/v1/documents/{DOCUMENT}/blob/payload");
        let found = find("PUT", &path).unwrap();
        assert_eq!((found.document, found.hash), (Some(DOCUMENT), Some("payload")));
        assert_eq!(found.route.limit.binding(), Some("GUEST_BLOB_LIMIT"));
        // An invalid hash still meets the limiter; the handler rejects it afterwards.
        assert_eq!(find("PUT", &format!("/v1/documents/{DOCUMENT}/blob/x")).unwrap().route.limit, Limit::Blob);
        assert_eq!(find("GET", &format!("/v1/documents/{DOCUMENT}/sync")).unwrap().route.limit, Limit::Connect);
        assert_eq!(find("DELETE", &format!("/v1/documents/{DOCUMENT}")).unwrap().route.limit, Limit::None);
    }
}
