//! Cloud catalog authority. These bounded records contain no credentials or document state.
use crate::{Error, Issue, Library, Result, Upload, failed};
use pop_sharing_protocol::lifecycle::Handoff;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

pub const CATALOG_FORMAT: u32 = 2;
const AUTHORITY_BYTES: usize = 32 * 1024;
const SYSTEM_FIELDS_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CatalogAuthority {
    Snapshot {
        receipt: Option<CatalogReceipt>,
    },
    Checkpoint {
        proof: CatalogCheckpointProof,
    },
    Preparing {
        transition: String,
        room: String,
        seed_hash: Option<String>,
        prior_checkpoint: Option<CatalogCheckpointProof>,
    },
    Relay {
        transition: String,
        room: String,
        seed_hash: String,
    },
    Handoff {
        transition: String,
        room: String,
        #[serde(with = "frozen")]
        freeze: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCheckpointProof {
    pub transition: String,
    pub source_version: String,
    pub version: String,
}
impl CatalogCheckpointProof {
    fn validate(&self) -> Result<()> {
        if !nonce(&self.transition)
            || self.source_version.is_empty()
            || self.source_version.len() > 8192
            || self.version.is_empty()
            || self.version.len() > 8192
        {
            return Err(failed("Invalid private checkpoint proof"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRelayBinding {
    pub transition: String,
    pub room: String,
    pub seed_hash: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogReceipt {
    pub transition: String,
    pub room: String,
    #[serde(with = "frozen")]
    pub freeze: String,
    pub snapshot_version: String,
}
// Native code transports opaque frozen frontier JSON; only the shared Rust protocol
// decoder interprets it. CloudKit stores its typed object rather than escaped JSON.
mod frozen {
    use super::*;
    pub fn serialize<S: serde::Serializer>(value: &str, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let value: Handoff = serde_json::from_str(value).map_err(serde::ser::Error::custom)?;
        value.validate().map_err(serde::ser::Error::custom)?;
        value.serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<String, D::Error> {
        let value = Handoff::deserialize(deserializer)?;
        value.validate().map_err(serde::de::Error::custom)?;
        serde_json::to_string(&value).map_err(serde::de::Error::custom)
    }
}
pub(crate) fn nonce(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn room(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Default for CatalogAuthority {
    fn default() -> Self {
        Self::Snapshot { receipt: None }
    }
}
impl CatalogAuthority {
    pub fn validate(&self) -> Result<()> {
        let (transition, room_id) = match self {
            Self::Snapshot { receipt: None } => return Ok(()),
            Self::Snapshot { receipt: Some(receipt) } => {
                if receipt.snapshot_version.len() > 8192 || receipt.snapshot_version.is_empty() {
                    return Err(failed("Invalid snapshot receipt"));
                }
                validate_freeze(&receipt.freeze)?;
                (&receipt.transition, &receipt.room)
            }
            Self::Checkpoint { proof } => return proof.validate(),
            Self::Preparing { transition, room, seed_hash, prior_checkpoint } => {
                if let Some(proof) = prior_checkpoint {
                    proof.validate()?;
                }
                if seed_hash.as_deref().is_some_and(|v| !hash(v)) {
                    return Err(failed("Invalid preparing epoch"));
                }
                (transition, room)
            }
            Self::Relay { transition, room, seed_hash } => {
                if !hash(seed_hash) {
                    return Err(failed("Invalid relay epoch"));
                }
                (transition, room)
            }
            Self::Handoff { transition, room, freeze } => {
                validate_freeze(freeze)?;
                (transition, room)
            }
        };
        if !nonce(transition) || !room(room_id) {
            return Err(failed("Invalid catalog transition"));
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<String> {
        self.validate()?;
        let json = serde_json::to_string(self).map_err(failed)?;
        if json.len() > AUTHORITY_BYTES {
            return Err(failed("Catalog authority exceeds its limit"));
        }
        Ok(json)
    }
    pub fn decode(format: u32, json: Option<&str>) -> Result<Self> {
        if format == 1 {
            return Ok(Self::default());
        }
        if format != CATALOG_FORMAT {
            return Err(Error { issue: Issue::RequiresUpdate, message: "This synced pop needs a newer pop.".into() });
        }
        let json = json.ok_or_else(|| Error {
            issue: Issue::RequiresUpdate,
            message: "This synced pop needs a newer pop.".into(),
        })?;
        if json.len() > AUTHORITY_BYTES {
            return Err(failed("Catalog authority exceeds its limit"));
        }
        let value: Self = serde_json::from_str(json).map_err(failed)?;
        value.validate()?;
        Ok(value)
    }
    pub fn snapshot(&self) -> bool {
        matches!(self, Self::Snapshot { .. } | Self::Checkpoint { .. })
    }
    fn progress(&self) -> u8 {
        match self {
            Self::Snapshot { receipt: None } => 0,
            Self::Preparing { .. } => 1,
            Self::Relay { .. } => 2,
            Self::Handoff { .. } => 3,
            Self::Snapshot { receipt: Some(_) } | Self::Checkpoint { .. } => 4,
        }
    }
    pub fn relay_binding(&self) -> Result<Option<CatalogRelayBinding>> {
        self.validate()?;
        Ok(match self {
            Self::Relay { transition, room, seed_hash } => Some(CatalogRelayBinding {
                transition: transition.clone(),
                room: room.clone(),
                seed_hash: seed_hash.clone(),
            }),
            Self::Handoff { transition, room, freeze } => Some(CatalogRelayBinding {
                transition: transition.clone(),
                room: room.clone(),
                seed_hash: validate_freeze(freeze)?.seed_hash,
            }),
            _ => None,
        })
    }
    pub fn transition(&self) -> Option<&str> {
        match self {
            Self::Snapshot { receipt } => receipt.as_ref().map(|v| v.transition.as_str()),
            Self::Checkpoint { proof } => Some(&proof.transition),
            Self::Preparing { transition, .. } | Self::Relay { transition, .. } | Self::Handoff { transition, .. } => {
                Some(transition)
            }
        }
    }
}
pub(crate) fn validate_freeze(json: &str) -> Result<Handoff> {
    if json.len() > AUTHORITY_BYTES {
        return Err(failed("Frozen frontier exceeds its limit"));
    }
    let freeze: Handoff = serde_json::from_str(json).map_err(failed)?;
    freeze.validate().map_err(failed)?;
    if !nonce(&freeze.token) {
        return Err(failed("Invalid frozen transition token"));
    }
    Ok(freeze)
}
impl Library {
    pub(crate) fn authority_inner(inner: &crate::Inner, id: &str) -> Result<CatalogAuthority> {
        let json: Option<String> =
            inner.db.query_row("SELECT committed FROM catalog WHERE id=?1", [id], |r| r.get(0)).optional()?;
        json.map_or(Ok(CatalogAuthority::default()), |json| CatalogAuthority::decode(CATALOG_FORMAT, Some(&json)))
    }
    pub fn authority(&self, id: &str) -> Result<CatalogAuthority> {
        Self::valid_id(id)?;
        Self::authority_inner(&self.inner.lock().unwrap(), id)
    }
    pub fn pending_authority(&self, id: &str) -> Result<Option<CatalogAuthority>> {
        Self::valid_id(id)?;
        let inner = self.inner.lock().unwrap();
        let json: Option<String> =
            inner.db.query_row("SELECT pending FROM catalog WHERE id=?1", [id], |r| r.get(0)).optional()?.flatten();
        json.map(|json| CatalogAuthority::decode(CATALOG_FORMAT, Some(&json))).transpose()
    }
    /// Persist before the first external side effect. Retries retain the same room and nonce.
    pub fn prepare_authority(&self, id: &str, authority: CatalogAuthority) -> Result<()> {
        Self::valid_id(id)?;
        let encoded = authority.encode()?;
        let inner = self.inner.lock().unwrap();
        let current = Self::authority_inner(&inner, id)?;
        if let Some(pending) = self.pending_authority_unlocked(&inner, id)?
            && pending != authority
            && pending.transition() != authority.transition()
            && !restores_preparing(&current, &authority)
        {
            return Err(failed("A catalog authority transition is already pending"));
        }
        validate_step(&current, &authority)?;
        inner.db.execute("INSERT INTO catalog(id,committed,pending) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET pending=excluded.pending", params![id,current.encode()?,encoded])?;
        Ok(())
    }
    pub(crate) fn pending_authority_unlocked(
        &self,
        inner: &crate::Inner,
        id: &str,
    ) -> Result<Option<CatalogAuthority>> {
        let json: Option<String> =
            inner.db.query_row("SELECT pending FROM catalog WHERE id=?1", [id], |r| r.get(0)).optional()?.flatten();
        json.map(|json| CatalogAuthority::decode(CATALOG_FORMAT, Some(&json))).transpose()
    }
    pub fn admit_authority(
        &self,
        id: &str,
        authority: &CatalogAuthority,
        stamp: Option<&crate::CatalogStamp>,
    ) -> Result<()> {
        Self::valid_id(id)?;
        authority.validate()?;
        let inner = self.inner.lock().unwrap();
        crate::stamp::check(&inner.db, id, stamp)?;
        crate::stamp::authority(&Self::authority_inner(&inner, id)?, authority, stamp)
    }
    pub fn stage_preparing(&self, id: &str, transition: &str) -> Result<Upload> {
        if !matches!(self.pending_authority(id)?, Some(CatalogAuthority::Preparing { transition: current, .. }) if current == transition)
        {
            return Err(failed("No matching catalog preparation"));
        }
        self.stage_snapshot(id)
    }
    /// A server record or successful change-tag CAS is the only committed authority.
    pub fn accept_authority(
        &self,
        id: &str,
        title: &str,
        template: &str,
        authority: CatalogAuthority,
        fields: &[u8],
        stamp: Option<&crate::CatalogStamp>,
    ) -> Result<()> {
        Self::valid_id(id)?;
        let encoded = authority.encode()?;
        if fields.len() > SYSTEM_FIELDS_BYTES {
            return Err(failed("Catalog receipt exceeds its limit"));
        }
        let mut inner = self.inner.lock().unwrap();
        crate::stamp::check(&inner.db, id, stamp)?;
        crate::stamp::authority(&Self::authority_inner(&inner, id)?, &authority, stamp)?;
        let prior_stamp = crate::stamp::read(&inner.db, id)?;
        let supersedes =
            stamp.is_some_and(|stamp| prior_stamp.as_ref().is_none_or(|old| stamp.revision > old.revision));
        let clear_pending = supersedes
            || self.pending_authority_unlocked(&inner, id)?.is_some_and(|pending| {
                pending == authority
                    || restores_preparing(&Self::authority_inner(&inner, id).unwrap_or_default(), &authority)
                    || (pending.transition() == authority.transition()
                        && authority.transition().is_some()
                        && authority.progress() >= pending.progress())
            });
        let tx = inner.db.transaction()?;
        tx.execute("INSERT INTO documents(id,title,template,fields,download_removed) VALUES(?1,?2,?3,?4,1) ON CONFLICT(id) DO UPDATE SET title=excluded.title,template=excluded.template,fields=excluded.fields", params![id,title,template,fields])?;
        tx.execute("INSERT INTO catalog(id,committed) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET committed=excluded.committed,pending=CASE WHEN ?3 THEN NULL ELSE pending END", params![id,encoded,clear_pending])?;
        crate::stamp::commit(&tx, id, stamp)?;
        tx.commit()?;
        Ok(())
    }
    /// The creator resolver installs an already validated relay seed through storage.
    /// Confirm only an accepted local file bound to the same catalog room/epoch.
    pub fn confirm_relay_download(&self, id: &str, room: &str, seed_hash: &str) -> Result<()> {
        Self::valid_id(id)?;
        let mut inner = self.inner.lock().unwrap();
        if !matches!(Self::authority_inner(&inner, id)?, CatalogAuthority::Relay { room: current_room, seed_hash: current_hash, .. } if current_room == room && current_hash == seed_hash)
        {
            return Err(failed("Relay catalog changed during download"));
        }
        let store = pop_storage::store::Store::open(&self.path(id), pop_storage::store::Mode::Snapshot)?;
        if store.app().document_uuid.as_deref() != Some(id) {
            return Err(failed("Downloaded relay document identity does not match the catalog"));
        }
        store.document()?;
        inner.db.execute("UPDATE documents SET download_removed=0,problem=NULL WHERE id=?1", [id])?;
        // A staged loser retains its bytes until canonical relay installation has
        // completed (including any dirty-local recovery), never just on discovery.
        if let Some(filename) = Self::retire_checkpoint(&mut inner, id)? {
            let _ = std::fs::remove_file(self.root.join("candidates").join(filename));
        }
        Ok(())
    }
    /// Final handoff bytes may be staged only for the matching durable frozen transition.
    pub fn stage_handoff(&self, id: &str, transition: &str) -> Result<Upload> {
        let authority = match self.pending_authority(id)? {
            Some(authority @ CatalogAuthority::Handoff { .. }) => authority,
            _ => self.authority(id)?,
        };
        let freeze = match authority {
            CatalogAuthority::Handoff { transition: current, freeze, .. } if current == transition => freeze,
            CatalogAuthority::Snapshot { receipt: Some(receipt) } if receipt.transition == transition => receipt.freeze,
            _ => return Err(failed("No matching frozen catalog handoff")),
        };
        let frozen = validate_freeze(&freeze)?;
        let upload = self.stage_snapshot(id)?;
        let checked = (|| {
            let path = upload.asset_path.as_ref().ok_or_else(|| failed("Missing handoff snapshot"))?;
            let store =
                pop_storage::store::Store::open(std::path::Path::new(path), pop_storage::store::Mode::Snapshot)?;
            if !store.document()?.covers_wire_history(&frozen.heads)? {
                return Err(failed("The saved snapshot has not reached the frozen relay frontier"));
            }
            Ok(())
        })();
        if let Err(error) = checked {
            if let Some(path) = &upload.asset_path {
                let _ = self.release_upload(std::path::Path::new(path));
            }
            return Err(error);
        }
        Ok(upload)
    }
}
fn validate_step(current: &CatalogAuthority, next: &CatalogAuthority) -> Result<()> {
    let valid = match (current, next) {
        (a, b) if a == b => true,
        (CatalogAuthority::Snapshot { receipt: None }, CatalogAuthority::Preparing { prior_checkpoint: None, .. }) => {
            true
        }
        (CatalogAuthority::Checkpoint { proof }, CatalogAuthority::Preparing { prior_checkpoint: Some(prior), .. }) => {
            proof.transition == prior.transition && proof.source_version == prior.source_version
        }
        (
            CatalogAuthority::Snapshot { receipt: None } | CatalogAuthority::Checkpoint { .. },
            CatalogAuthority::Checkpoint { .. },
        ) => true,
        (current, next) if restores_preparing(current, next) => true,
        (CatalogAuthority::Snapshot { receipt: Some(current) }, CatalogAuthority::Snapshot { receipt: Some(next) }) => {
            current.transition == next.transition
                && current.room == next.room
                && validate_freeze(&current.freeze)? == validate_freeze(&next.freeze)?
        }
        (CatalogAuthority::Snapshot { receipt: Some(receipt) }, CatalogAuthority::Checkpoint { proof }) => {
            proof.transition == receipt.transition && proof.source_version == receipt.snapshot_version
        }

        (
            CatalogAuthority::Relay { transition: a, room: ar, .. },
            CatalogAuthority::Relay { transition: b, room: br, .. },
        ) => a == b && ar == br,
        (
            CatalogAuthority::Preparing { transition: a, room: ar, .. },
            CatalogAuthority::Relay { transition: b, room: br, .. },
        ) => a == b && ar == br,
        (
            CatalogAuthority::Relay { transition: a, room: ar, seed_hash },
            CatalogAuthority::Handoff { transition: b, room: br, freeze },
        ) => a == b && ar == br && validate_freeze(freeze)?.seed_hash == *seed_hash,
        (
            CatalogAuthority::Handoff { transition, room, freeze },
            CatalogAuthority::Snapshot { receipt: Some(receipt) },
        ) => {
            transition == &receipt.transition
                && room == &receipt.room
                && validate_freeze(freeze)? == validate_freeze(&receipt.freeze)?
        }
        _ => false,
    };
    if valid { Ok(()) } else { Err(failed("Catalog authority transition does not match the current record")) }
}

pub(crate) fn validate_remote(current: &CatalogAuthority, next: &CatalogAuthority) -> Result<()> {
    next.validate()?;
    if matches!(current, CatalogAuthority::Checkpoint { .. } | CatalogAuthority::Snapshot { receipt: Some(_) })
        && matches!(next, CatalogAuthority::Snapshot { receipt: None })
    {
        return Err(failed("An established epoch cannot lose its checkpoint proof"));
    }
    if current.snapshot() || current == next {
        return Ok(());
    }
    match (current, next) {
        (
            CatalogAuthority::Preparing { transition: a, room: ar, .. },
            CatalogAuthority::Relay { transition: b, room: br, .. }
            | CatalogAuthority::Handoff { transition: b, room: br, .. },
        ) if a == b && ar == br => Ok(()),
        (
            CatalogAuthority::Preparing { transition, room, seed_hash, .. },
            CatalogAuthority::Snapshot { receipt: Some(receipt) },
        ) if transition == &receipt.transition
            && room == &receipt.room
            && seed_hash
                .as_ref()
                .is_none_or(|hash| validate_freeze(&receipt.freeze).is_ok_and(|freeze| freeze.seed_hash == *hash)) =>
        {
            Ok(())
        }
        (
            CatalogAuthority::Relay { transition, room, seed_hash },
            CatalogAuthority::Snapshot { receipt: Some(receipt) },
        ) if transition == &receipt.transition
            && room == &receipt.room
            && validate_freeze(&receipt.freeze)?.seed_hash == *seed_hash =>
        {
            Ok(())
        }
        _ => validate_step(current, next),
    }
}

fn restores_preparing(current: &CatalogAuthority, next: &CatalogAuthority) -> bool {
    match (current, next) {
        (
            CatalogAuthority::Preparing { prior_checkpoint: Some(previous), .. },
            CatalogAuthority::Checkpoint { proof },
        ) => previous == proof,
        (CatalogAuthority::Preparing { prior_checkpoint: None, .. }, CatalogAuthority::Snapshot { receipt: None }) => {
            true
        }
        _ => false,
    }
}
