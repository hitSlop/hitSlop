//! Guest entry point budgets are independent and shared across rooms per IP.
pub(crate) fn guest_limit(method: &str, path: &str) -> Option<&'static str> {
    let parts: Vec<_> = path.strip_prefix("/v1/documents/")?.split('/').collect();
    if !pop_sharing_protocol::valid_room(parts[0]) {
        return None;
    }
    match (method, parts.as_slice()) {
        ("POST", [_, "compact" | "epoch"]) => Some("GUEST_COMPACT_LIMIT"),
        ("PUT" | "DELETE", [_, "epochs", hash]) if pop_sharing_protocol::resources::hash_id(hash) => {
            Some("GUEST_COMPACT_LIMIT")
        }
        ("PUT", [_, "epochs", seed, "blobs", hash])
            if pop_sharing_protocol::resources::hash_id(seed) && pop_sharing_protocol::resources::hash_id(hash) =>
        {
            Some("GUEST_BLOB_LIMIT")
        }
        ("GET", [_, "epoch"]) => Some("GUEST_METADATA_LIMIT"),
        ("GET", [_]) => Some("GUEST_METADATA_LIMIT"),
        ("GET", [_, "seed"]) => Some("GUEST_SEED_LIMIT"),
        ("GET", [_, "sync"]) => Some("GUEST_CONNECT_LIMIT"),
        ("PUT", [_, "blob", hash]) if pop_sharing_protocol::resources::hash_id(hash) => Some("GUEST_BLOB_LIMIT"),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_guest_budgets_cover_only_the_intended_routes() {
        let room = "00000000-0000-4000-8000-000000000001".to_owned();
        for (method, suffix, expected) in [
            ("GET", "", Some("GUEST_METADATA_LIMIT")),
            ("GET", "/seed", Some("GUEST_SEED_LIMIT")),
            ("GET", "/sync", Some("GUEST_CONNECT_LIMIT")),
            ("PUT", "/blob/", None),
            ("PUT", "/seed", None),
            ("DELETE", "", None),
            ("GET", "/seed/extra", None),
        ] {
            assert_eq!(guest_limit(method, &format!("/v1/documents/{room}{suffix}")), expected);
        }
        assert_eq!(
            guest_limit("PUT", &format!("/v1/documents/{room}/blob/{}", "b".repeat(64))),
            Some("GUEST_BLOB_LIMIT")
        );
        assert_eq!(guest_limit("GET", "/v1/account"), None);
        assert_eq!(guest_limit("GET", "/v1/documents/invalid"), None);
    }
}
