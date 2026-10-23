//! What a failed relay request means, decided once for the native client and the browser.
use serde::Serialize;

/// Which relay route a request used, for the statuses whose meaning depends on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Other,
    Compact,
    Blob,
}
impl Route {
    /// The route a room URL addresses.
    pub fn of(url: &str) -> Self {
        let path = url.split(['?', '#']).next().unwrap_or_default();
        if path.ends_with("/compact") {
            Self::Compact
        } else if path.contains("/blob/") {
            Self::Blob
        } else {
            Self::Other
        }
    }
}
/// A failure's meaning. Names match the pause kinds of the four that pause sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// The room is gone.
    Ended,
    NeedsUpdate,
    Refused,
    /// Compaction could not make the room fit.
    Outgrown,
    /// The room's attachment budget is spent.
    FilesFull,
    /// Another writer got there first, or creation is unfinished.
    Conflict,
    /// The relay wants a signed-in sharing account.
    SignIn,
    /// Anything else: shown, and retried where the caller retries.
    Failed,
}
/// `code` is the relay's `ApiFailure` code, when the body carried one.
pub fn classify(status: u16, code: Option<&str>, route: Route) -> Outcome {
    if code == Some("requires_update") {
        return Outcome::NeedsUpdate;
    }
    match (status, route) {
        (410, _) => Outcome::Ended,
        (413, Route::Compact) => Outcome::Outgrown,
        (413, Route::Blob) => Outcome::FilesFull,
        (401, _) => Outcome::SignIn,
        (409, _) => Outcome::Conflict,
        (400 | 403 | 404 | 422, _) => Outcome::Refused,
        _ => Outcome::Failed,
    }
}
impl Outcome {
    /// What to tell a person when the relay gave no message of its own.
    pub fn message(self, status: u16) -> String {
        match self {
            Self::Ended => "Sharing ended".into(),
            Self::NeedsUpdate => "Update hitSlop to keep sharing".into(),
            Self::Outgrown => "This document has outgrown sharing".into(),
            Self::FilesFull => "Attachments are full".into(),
            Self::SignIn => "Sign in to your sharing account first".into(),
            Self::Conflict => "Sharing was busy. Try again.".into(),
            Self::Refused | Self::Failed => format!("Sharing service refused this request ({status})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statuses_mean_the_same_to_every_client() {
        use Outcome::*;
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
    fn routes_follow_the_room_url() {
        let room = "https://share.hitslop.com/v1/rooms/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(Route::of(&format!("{room}/compact")), Route::Compact);
        assert_eq!(Route::of(&format!("{room}/blob/{}", "b".repeat(64))), Route::Blob);
        for other in ["", "/seed", "/blobs", "/resources", "/sync"] {
            assert_eq!(Route::of(&format!("{room}{other}")), Route::Other, "{other}");
        }
    }
    #[test]
    fn conflicts_keep_their_retry_text() {
        assert_eq!(Outcome::Conflict.message(409), "Sharing was busy. Try again.");
        assert_eq!(Outcome::SignIn.message(401), "Sign in to your sharing account first");
        assert_eq!(Outcome::Refused.message(404), "Sharing service refused this request (404)");
    }
}
