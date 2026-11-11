//! What a failed relay request means, decided once for every client.
use pop_contracts::relay::PauseKind;

/// Which relay route a request used, for the statuses whose meaning depends on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Other,
    Compact,
    Blob,
}
/// `code` is the relay's `ApiFailure` code, when the body carried one.
pub fn classify(status: u16, code: Option<&str>, route: Route) -> PauseKind {
    if code == Some("requires_update") {
        return PauseKind::NeedsUpdate;
    }
    match (status, route) {
        (410, _) => PauseKind::Ended,
        (413, Route::Compact) => PauseKind::Outgrown,
        (413, Route::Blob) => PauseKind::FilesFull,
        (401, _) => PauseKind::SignIn,
        (409, _) => PauseKind::Conflict,
        (400 | 403 | 404 | 422, _) => PauseKind::Refused,
        _ => PauseKind::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statuses_mean_the_same_to_every_client() {
        use PauseKind::*;
        use Route::*;
        for (status, code, route, expected) in [
            (400, None, Other, Refused),
            (403, None, Other, Refused),
            (404, None, Blob, Refused),
            (422, None, Compact, Refused),
            (400, Some("requires_update"), Other, NeedsUpdate),
            (426, Some("requires_update"), Compact, NeedsUpdate),
            (400, Some("invalid_request"), Other, Refused),
            (410, None, Other, Ended),
            (410, None, Compact, Ended),
            (413, None, Compact, Outgrown),
            // The relay's refusal of a base over `BASE_BYTES`: sharing pauses as outgrown.
            (413, Some("invalid_request"), Compact, Outgrown),
            (413, None, Blob, FilesFull),
            (413, None, Other, Failed),
            (409, None, Other, Conflict),
            (409, None, Compact, Conflict),
            (401, None, Other, SignIn),
            (401, None, Blob, SignIn),
            (429, None, Other, Failed),
            (500, None, Blob, Failed),
        ] {
            assert_eq!(classify(status, code, route), expected, "{status} {code:?} {route:?}");
        }
    }
    #[test]
    fn conflicts_keep_their_retry_text() {
        assert_eq!(PauseKind::Conflict.message(409), "Sharing was busy. Try again.");
        assert_eq!(PauseKind::SignIn.message(401), "Sign in to your sharing account first");
        assert_eq!(PauseKind::Refused.message(404), "Sharing service refused this request (404)");
    }
}
