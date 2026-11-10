//! HTTP metadata only. Document state travels as SQLite seeds and opaque updates.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum RoomPhase {
    Uploading,
    Live,
    Ended,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub struct CreateRoom {
    pub document: String,
}

/// The seed an upload makes live. The relay refuses (409) one that is not the seed it
/// stores, so an older attempt cannot publish a seed uploaded after it. The relay does not
/// check attachments: the creator uploads every attachment the seed references first.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub struct OpenRoom {
    /// Lowercase hex SHA-256 of the uploaded seed bytes.
    pub seed_sha256: String,
}

/// A document's seed and app, read by any member. Access and the invitation secret are
/// only in `AccessState`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub struct RoomInfo {
    pub document: String,
    pub phase: RoomPhase,
    pub seed_hash: Option<String>,
    pub seed_size: Option<u32>,
    pub app: Option<String>,
    /// Whether retained updates have reached the size at which a member may compact.
    pub compact: bool,
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
        if !crate::valid_document_id(&self.room) || !crate::resources::hash_id(&self.capability) {
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
}
