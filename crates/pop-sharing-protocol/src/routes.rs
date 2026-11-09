//! The one table of public service routes. Worker dispatch and rate-limit classification
//! read `ROUTES`; a request that matches no row is a 404 before any handler or limiter.
//!
//! The authored-domain resource host (`/assets`, `/attachments`, `/_resources`) is a
//! separate surface with its own cookie authorization and is not described here.

/// One operation per (method, path). Handlers match on this, never on path text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Health,
    CreateDocument,
    DeleteAccount,
    AccountLibrary,
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

#[derive(Debug)]
pub struct Route {
    pub op: Op,
    pub method: &'static str,
    /// `{document}` is a canonical UUID; `{hash}` is a single non-empty segment.
    pub path: &'static str,
    pub scope: Scope,
    pub limit: Limit,
}
impl Route {
    /// WebSocket protocol accepted by this route, if it supports upgrading.
    pub const fn socket_protocol(&self) -> Option<&'static str> {
        match self.op {
            Op::Sync => Some(crate::SOCKET_PROTOCOL),
            Op::AccountLibrary => Some(crate::library::SOCKET_PROTOCOL),
            _ => None,
        }
    }
}

const fn route(op: Op, method: &'static str, path: &'static str, scope: Scope, limit: Limit) -> Route {
    Route { op, method, path, scope, limit }
}

use Limit as L;
use Scope::*;
pub static ROUTES: &[Route] = &[
    route(Op::Health, "GET", "/health", Service, L::None),
    route(Op::CreateDocument, "POST", "/v1/documents", Account, L::None),
    route(Op::DeleteAccount, "DELETE", "/v1/account", Account, L::None),
    route(Op::AccountLibrary, "GET", "/v1/account/library", Account, L::Connect),
    route(Op::GetDocument, "GET", "/v1/documents/{document}", Document, L::Metadata),
    route(Op::PatchDocument, "PATCH", "/v1/documents/{document}", Document, L::None),
    route(Op::DeleteDocument, "DELETE", "/v1/documents/{document}", Document, L::None),
    route(Op::GetMetadata, "GET", "/v1/documents/{document}/metadata", Document, L::None),
    route(Op::GetSharing, "GET", "/v1/documents/{document}/sharing", Document, L::None),
    route(Op::EnableSharing, "POST", "/v1/documents/{document}/sharing", Document, L::None),
    route(Op::DisableSharing, "DELETE", "/v1/documents/{document}/sharing", Document, L::None),
    route(Op::Join, "POST", "/v1/documents/{document}/members", Document, L::None),
    route(Op::Leave, "DELETE", "/v1/documents/{document}/members/me", Document, L::None),
    route(Op::Open, "POST", "/v1/documents/{document}/open", Document, L::None),
    route(Op::GetSeed, "GET", "/v1/documents/{document}/seed", Document, L::Seed),
    route(Op::PutSeed, "PUT", "/v1/documents/{document}/seed", Document, L::None),
    route(Op::GetResources, "GET", "/v1/documents/{document}/resources", Document, L::None),
    route(Op::PutResources, "POST", "/v1/documents/{document}/resources", Document, L::None),
    route(Op::PutAsset, "PUT", "/v1/documents/{document}/asset/{hash}", Document, L::None),
    route(Op::ListBlobs, "GET", "/v1/documents/{document}/blobs", Document, L::None),
    route(Op::GetBlob, "GET", "/v1/documents/{document}/blob/{hash}", Document, L::None),
    route(Op::PutBlob, "PUT", "/v1/documents/{document}/blob/{hash}", Document, L::Blob),
    route(Op::Compact, "POST", "/v1/documents/{document}/compact", Document, L::Compact),
    route(Op::Sync, "GET", "/v1/documents/{document}/sync", Document, L::Connect),
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
            ("GET", format!("/v1/account/library/{DOCUMENT}").as_str()),
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
