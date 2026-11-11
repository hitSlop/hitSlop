//! The one table of public and internal service routes. Worker dispatch and rate-limit classification
//! read `ROUTES`; a request that matches no row is a 404 before any handler or limiter.

/// One operation per (method, path). Handlers match on this, never on path text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Init,
    LibraryDescription,
    InternalDelete,
    InternalLeave,
    Describe,
    Release,
    Register,
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
            Self::Metadata => "METADATA_LIMIT",
            Self::Seed => "SEED_LIMIT",
            Self::Connect => "CONNECT_LIMIT",
            Self::Blob => "BLOB_LIMIT",
            Self::Compact => "COMPACT_LIMIT",
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
    pub access: Access,
    pub phases: Phases,
    pub internal: bool,
}
const fn route(
    op: Op,
    method: &'static str,
    path: &'static str,
    scope: Scope,
    limit: Limit,
    access: Access,
    phases: Phases,
) -> Route {
    Route {
        op,
        method,
        path,
        scope,
        limit,
        access,
        phases,
        internal: matches!(
            access,
            Access::Internal | Access::InternalMember | Access::InternalOwner | Access::InternalSignedIn
        ),
    }
}

use Limit as L;
use Scope::{Account, Document, Service};
pub static ROUTES: &[Route] = &[
    route(Op::Init, "POST", "/internal/documents/{document}/init", Document, L::None, Access::Internal, Phases::Active),
    route(
        Op::LibraryDescription,
        "POST",
        "/internal/documents/{document}/library",
        Document,
        L::None,
        Access::InternalMember,
        Phases::Live,
    ),
    route(
        Op::InternalDelete,
        "DELETE",
        "/internal/documents/{document}/delete",
        Document,
        L::None,
        Access::InternalOwner,
        Phases::Any,
    ),
    route(
        Op::InternalLeave,
        "DELETE",
        "/internal/documents/{document}/leave",
        Document,
        L::None,
        Access::InternalSignedIn,
        Phases::Active,
    ),
    route(Op::Describe, "POST", "/internal/describe", Account, L::None, Access::Internal, Phases::Any),
    route(Op::Release, "POST", "/internal/release", Account, L::None, Access::Internal, Phases::Any),
    route(Op::Register, "POST", "/internal/member", Account, L::None, Access::Internal, Phases::Any),
    route(Op::Health, "GET", "/health", Service, L::None, Access::Public, Phases::Any),
    route(Op::CreateDocument, "POST", "/v1/documents", Account, L::None, Access::SignedIn, Phases::Any),
    route(Op::DeleteAccount, "DELETE", "/v1/account", Account, L::None, Access::SignedIn, Phases::Any),
    route(Op::AccountLibrary, "GET", "/v1/account/library", Account, L::Connect, Access::SignedIn, Phases::Any),
    route(
        Op::GetDocument,
        "GET",
        "/v1/documents/{document}",
        Document,
        L::Metadata,
        Access::Member,
        Phases::MemberLive,
    ),
    route(Op::PatchDocument, "PATCH", "/v1/documents/{document}", Document, L::None, Access::Owner, Phases::Active),
    route(Op::DeleteDocument, "DELETE", "/v1/documents/{document}", Document, L::None, Access::Owner, Phases::Any),
    route(Op::GetMetadata, "GET", "/v1/documents/{document}/metadata", Document, L::None, Access::Member, Phases::Live),
    route(Op::GetSharing, "GET", "/v1/documents/{document}/sharing", Document, L::None, Access::Member, Phases::Active),
    route(
        Op::EnableSharing,
        "POST",
        "/v1/documents/{document}/sharing",
        Document,
        L::None,
        Access::Owner,
        Phases::Active,
    ),
    route(
        Op::DisableSharing,
        "DELETE",
        "/v1/documents/{document}/sharing",
        Document,
        L::None,
        Access::Owner,
        Phases::Active,
    ),
    route(Op::Join, "POST", "/v1/documents/{document}/members", Document, L::None, Access::SignedIn, Phases::Active),
    route(
        Op::Leave,
        "DELETE",
        "/v1/documents/{document}/members/me",
        Document,
        L::None,
        Access::SignedIn,
        Phases::Active,
    ),
    route(Op::Open, "POST", "/v1/documents/{document}/open", Document, L::None, Access::Owner, Phases::Active),
    route(Op::GetSeed, "GET", "/v1/documents/{document}/seed", Document, L::Seed, Access::Member, Phases::MemberLive),
    route(Op::PutSeed, "PUT", "/v1/documents/{document}/seed", Document, L::None, Access::Owner, Phases::Uploading),
    route(
        Op::ListBlobs,
        "GET",
        "/v1/documents/{document}/blobs",
        Document,
        L::None,
        Access::PreparingOwner,
        Phases::Active,
    ),
    route(
        Op::GetBlob,
        "GET",
        "/v1/documents/{document}/blob/{hash}",
        Document,
        L::None,
        Access::Member,
        Phases::MemberLive,
    ),
    route(
        Op::PutBlob,
        "PUT",
        "/v1/documents/{document}/blob/{hash}",
        Document,
        L::Blob,
        Access::PreparingOwner,
        Phases::Active,
    ),
    route(
        Op::Compact,
        "POST",
        "/v1/documents/{document}/compact",
        Document,
        L::Compact,
        Access::Member,
        Phases::MemberLive,
    ),
    route(Op::Sync, "GET", "/v1/documents/{document}/sync", Document, L::Connect, Access::Member, Phases::MemberLive),
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
                "{document}" if pop_contracts::identity::valid_document_id(segment) => found.document = Some(segment),
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
        assert_eq!(found.route.limit.binding(), Some("BLOB_LIMIT"));
        // An invalid hash still meets the limiter; the handler rejects it afterwards.
        assert_eq!(find("PUT", &format!("/v1/documents/{DOCUMENT}/blob/x")).unwrap().route.limit, Limit::Blob);
        assert_eq!(find("GET", &format!("/v1/documents/{DOCUMENT}/sync")).unwrap().route.limit, Limit::Connect);
        assert_eq!(find("DELETE", &format!("/v1/documents/{DOCUMENT}")).unwrap().route.limit, Limit::None);
    }
}

use crate::refusal::Refusal;
use pop_sharing_protocol::{document::DocumentRole, http::RoomPhase};

/// Permission required by a route, before operation-specific capability validation.
#[derive(Clone, Copy, Debug)]
pub enum Access {
    Public,
    SignedIn,
    Member,
    PreparingOwner,
    Owner,
    Internal,
    InternalSignedIn,
    InternalMember,
    InternalOwner,
}
/// Phase admission, including owner preparation before publication.
#[derive(Clone, Copy, Debug)]
pub enum Phases {
    Any,
    Active,
    Live,
    Uploading,
    MemberLive,
}
/// Trusted process-local caller context.
#[derive(Clone, Copy, Debug)]
pub struct Caller {
    pub internal: bool,
    pub role: Option<DocumentRole>,
}
/// Resolved caller authority, including signed-in invitation exchanges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Owner,
    Member,
    SignedIn,
    Internal,
}
/// Resolve permissions and phase once for a matched operation.
pub fn authorize(operation: &Route, caller: Caller, phase: Option<RoomPhase>) -> Result<Role, Refusal> {
    use Access::*;
    use RoomPhase::*;
    let role = match caller.role {
        Some(DocumentRole::Owner) => Role::Owner,
        Some(_) => Role::Member,
        None => Role::SignedIn,
    };
    let denied = || Refusal::new(403, "denied", "Access revoked");
    if operation.internal && !caller.internal {
        return Err(denied());
    }
    if operation.scope == Scope::Document && phase.is_none() && operation.op != Op::Init {
        return Err(Refusal::new(404, "not_found", "Unknown room"));
    }
    if operation.op == Op::LibraryDescription && phase != Some(Live) {
        return Err(Refusal::new(409, "conflict", "Document not live"));
    }
    // Deletion is retryable only for the owner. Other ended routes retain their refusal.
    if phase == Some(Ended) && !matches!(operation.phases, Phases::Any) {
        return Err(Refusal::new(
            410,
            "sharing_ended",
            if operation.op == Op::Init { "Sharing ended" } else { "Document deleted" },
        ));
    }
    if operation.op == Op::GetMetadata && phase != Some(Live) {
        return Err(Refusal::new(409, "conflict", "Document upload is not ready"));
    }
    let allowed = match operation.access {
        Public | SignedIn | InternalSignedIn => true,
        Member | InternalMember => caller.role.is_some(),
        PreparingOwner => caller.role.is_some() && (phase != Some(Uploading) || role == Role::Owner),
        Owner | InternalOwner => role == Role::Owner,
        Internal => caller.internal,
    };
    if !allowed {
        return Err(denied());
    }
    let phase_allowed = match operation.phases {
        Phases::Any | Phases::Active => true,
        Phases::Live => phase == Some(Live),
        Phases::Uploading => phase == Some(Uploading),
        Phases::MemberLive => role == Role::Owner || phase == Some(Live),
    };
    if !phase_allowed {
        let message = match operation.op {
            Op::PutSeed => "Room is live",
            Op::GetMetadata => "Document upload is not ready",
            Op::LibraryDescription => "Document not live",
            _ => "Publication pending",
        };
        return Err(Refusal::new(409, "conflict", message));
    }
    Ok(if matches!(operation.access, Internal) { Role::Internal } else { role })
}

#[cfg(target_arch = "wasm32")]
pub(crate) const ROUTE_HEADER: &str = "X-Pop-Resolved-Route";
#[cfg(target_arch = "wasm32")]
impl Matched<'_> {
    pub(crate) fn forward(&self, headers: &mut worker::Headers) -> worker::Result<()> {
        let index = ROUTES.iter().position(|r| std::ptr::eq(r, self.route)).expect("matched route belongs to table");
        headers.set(ROUTE_HEADER, &serde_json::to_string(&(index, self.document, self.hash))?)
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) type Resolved = (&'static Route, Option<String>, Option<String>);
/// Public requests carry the edge's match; internal calls resolve here instead.
#[cfg(target_arch = "wasm32")]
pub(crate) fn resolve(req: &worker::Request) -> worker::Result<Option<Resolved>> {
    if let Some(text) = req.headers().get(ROUTE_HEADER)? {
        let (index, document, hash): (usize, Option<String>, Option<String>) = serde_json::from_str(&text)?;
        return Ok(ROUTES.get(index).map(|route| (route, document, hash)));
    }
    let path = req.path();
    Ok(find(req.method().as_ref(), &path).map(|m| (m.route, m.document.map(str::to_owned), m.hash.map(str::to_owned))))
}

#[cfg(test)]
mod authorization_tests {
    use super::*;
    #[test]
    fn every_access_class_and_phase_is_enforced() {
        for route in ROUTES {
            for role in [None, Some(DocumentRole::Editor), Some(DocumentRole::Owner)] {
                for internal in [false, true] {
                    for phase in [None, Some(RoomPhase::Uploading), Some(RoomPhase::Live), Some(RoomPhase::Ended)] {
                        let result = authorize(route, Caller { internal, role }, phase);
                        if route.internal && !internal {
                            assert!(result.is_err(), "{:?}", route.op);
                        }
                        if matches!(route.access, Access::Owner | Access::InternalOwner)
                            && role != Some(DocumentRole::Owner)
                        {
                            assert!(result.is_err(), "{:?}", route.op);
                        }
                        if matches!(route.access, Access::Member | Access::InternalMember) && role.is_none() {
                            assert!(result.is_err(), "{:?}", route.op);
                        }
                        if phase == Some(RoomPhase::Ended) && !matches!(route.phases, Phases::Any) {
                            assert!(result.is_err(), "{:?}", route.op);
                        }
                    }
                }
            }
            let caller = Caller { internal: route.internal, role: Some(DocumentRole::Owner) };
            assert!(
                authorize(route, caller, Some(RoomPhase::Uploading)).is_ok() || matches!(route.phases, Phases::Live),
                "{:?}",
                route.op
            );
            assert!(
                authorize(route, caller, Some(RoomPhase::Live)).is_ok() || matches!(route.phases, Phases::Uploading),
                "{:?}",
                route.op
            );
        }
    }
    #[test]
    fn signed_in_and_member_authorities_follow_route_requirements() {
        use RoomPhase::{Ended, Live, Uploading};
        for (op, role, phase, expected) in [
            (Op::Join, None, Live, Ok(Role::SignedIn)),
            (Op::Leave, None, Live, Ok(Role::SignedIn)),
            (Op::PatchDocument, None, Live, Err(403)),
            (Op::PatchDocument, Some(DocumentRole::Editor), Live, Err(403)),
            (Op::PatchDocument, Some(DocumentRole::Owner), Uploading, Ok(Role::Owner)),
            (Op::GetDocument, Some(DocumentRole::Editor), Uploading, Err(409)),
            (Op::PutBlob, Some(DocumentRole::Editor), Uploading, Err(403)),
            (Op::PutBlob, Some(DocumentRole::Editor), Live, Ok(Role::Member)),
            (Op::GetSeed, None, Live, Err(403)),
            (Op::GetSeed, Some(DocumentRole::Owner), Ended, Err(410)),
            (Op::DeleteDocument, Some(DocumentRole::Owner), Ended, Ok(Role::Owner)),
            (Op::GetMetadata, None, Uploading, Err(409)),
            (Op::PutSeed, Some(DocumentRole::Owner), Live, Err(409)),
        ] {
            let row = ROUTES.iter().find(|r| r.op == op).unwrap();
            assert_eq!(
                authorize(row, Caller { internal: false, role }, Some(phase)).map_err(|e| e.status),
                expected,
                "{op:?} {role:?} {phase:?}"
            );
        }
        for op in [Op::Init, Op::Describe, Op::Register, Op::Release, Op::InternalLeave] {
            let row = ROUTES.iter().find(|r| r.op == op).unwrap();
            assert!(authorize(row, Caller { internal: true, role: None }, Some(Live)).is_ok(), "{op:?}");
            assert!(
                authorize(row, Caller { internal: false, role: Some(DocumentRole::Owner) }, Some(Live)).is_err(),
                "{op:?}"
            );
        }
    }
}
