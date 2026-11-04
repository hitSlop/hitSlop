//! Creator revocation is ordered after accepted local or catalog durability.
use super::*;
use pop_sharing_protocol::{
    frame::Frame,
    lifecycle::{BeginHandoff, EpochBase, FinalizeHandoff, Handoff},
};

impl Client {
    pub fn freeze(&self, token: &str) -> Result<String> {
        let key = self.key.lock().expect("sharing key").runtime()?;
        self.freeze_at(token, &key)
    }
    pub(super) fn freeze_at(&self, token: &str, key: &RoomKey) -> Result<String> {
        self.pause();
        let epoch = match resolution::epoch(key, token) {
            Ok(epoch) => epoch,
            Err(transport::HttpFailure::Ended) => {
                let transaction = self.access.journal.transaction()?;
                let receipt = transaction.current()?.ok_or("No durable handoff proof is available")?;
                let frozen = receipt.epoch.handoff.as_ref().ok_or("No durable handoff proof is available")?;
                if verify_saved(&self.owner, frozen)? != receipt.owner_version {
                    return Err(
                        "The retained document changed after handoff; reconcile the saved copy before retrying".into(),
                    );
                }
                return serde_json::to_string(frozen).map_err(fail);
            }
            Err(error) => return Err(error.into()),
        };
        let frozen = if let Some(frozen) = epoch.handoff.clone() {
            frozen
        } else {
            let begin = BeginHandoff {
                token: random()?[..32].to_owned(),
                expected: EpochBase { seed_hash: epoch.seed_hash.clone(), heads: epoch.heads.clone() },
            };
            let bytes = transport::http_typed(
                key,
                "POST",
                "handoff/begin",
                Some(token),
                &serde_json::to_vec(&begin).map_err(fail)?,
                &[("Content-Type", "application/json")],
            )?;
            let frozen: Handoff = serde_json::from_slice(&bytes).map_err(fail)?;
            frozen.validate()?;
            frozen
        };
        let current = resolution::epoch(key, token)?;
        if current.handoff.as_ref() != Some(&frozen) {
            return Err("The frozen relay changed".into());
        }
        let access = epochs::Access { journal: self.access.journal.clone(), identity: Some(token.to_owned()) };
        validate_membership(self.owner.share().map_err(fail)?, key)?;
        epochs::bootstrap(&self.owner, key, &access)?;
        let binding = Share { endpoint: key.origin.clone(), room: key.room.clone() };
        if self.owner.share().map_err(fail)?.as_ref() != Some(&binding) {
            epochs::retry_busy(&self.owner, || Request::Share { share: binding.clone() })?;
        }
        resources::download_blobs_authenticated(&self.owner, key, &mut Vec::new(), token)?;
        let mut after = None;
        // Frozen history is finite; cursor advancement and a hard page bound prevent a malicious relay loop.
        for _ in 0..=pop_sharing_protocol::limits::ROOM_RECORDS {
            let (bytes, next, done) = transport::history_page(key, token, after)?;
            let Frame::Records(records) = Frame::decode(&bytes).map_err(fail)? else {
                return Err("Invalid frozen history frame".into());
            };
            if records.len() > pop_sharing_protocol::limits::HANDOFF_PAGE_RECORDS {
                return Err("Frozen history record limit exceeded".into());
            }
            for record in records {
                epochs::retry_busy(&self.owner, || Request::MergeSnapshot { bytes: record.bytes.clone() })?;
            }
            if done {
                epochs::retry_busy(&self.owner, || Request::Flush)?;
                let version = verify_saved(&self.owner, &frozen)?;
                let transaction = self.access.journal.transaction()?;
                let recovery = transaction.current()?.and_then(|receipt| receipt.recovery);
                transaction.confirm(&self.access.journal.receipt(current.clone(), version, recovery))?;
                return serde_json::to_string(&frozen).map_err(fail);
            }
            after = next;
        }
        Err("Frozen history page limit exceeded".into())
    }
    /// A catalog receipt is required for managed documents. Local documents prove
    /// their accepted saved file here; membership clears after a definitive relay ACK.
    pub fn finalize(&self, token: &str, freeze: &str, receipt: Option<&pop_library::CatalogReceipt>) -> Result<String> {
        let key = self.key.lock().expect("sharing key").runtime()?;
        self.finalize_at(token, freeze, receipt, &key)
    }
    pub(super) fn finalize_at(
        &self,
        token: &str,
        freeze: &str,
        receipt: Option<&pop_library::CatalogReceipt>,
        key: &RoomKey,
    ) -> Result<String> {
        self.pause();
        finalize_owner(&self.owner, &key.origin, &key.room, token, freeze, receipt)?;
        self.access.journal.retire_handoff(freeze)?;
        let mut credential = self.key.lock().expect("sharing key").clone();
        credential.ended = true;
        let value = credential.encode()?;
        *self.key.lock().expect("sharing key") = credential;
        Ok(value)
    }
}
/// Finish a durable catalog handoff on a device that never held the link capability.
/// Every relay request uses verified creator identity; no credential is manufactured.
pub fn finalize_catalog_handoff(
    owner: &Owner,
    document: &str,
    token: &str,
    receipt: &pop_library::CatalogReceipt,
) -> Result<()> {
    if owner.app().document_uuid.as_deref() != Some(document) || !pop_sharing_protocol::valid_copy(document) {
        return Err("Different handoff document".into());
    }
    finalize_owner(owner, pop_sharing_protocol::TRUSTED_ORIGIN, &receipt.room, token, &receipt.freeze, Some(receipt))
}
fn finalize_owner(
    owner: &Owner,
    origin: &str,
    room: &str,
    token: &str,
    freeze: &str,
    receipt: Option<&pop_library::CatalogReceipt>,
) -> Result<()> {
    let frozen: Handoff = serde_json::from_str(freeze).map_err(fail)?;
    frozen.validate()?;
    if let Some(receipt) = receipt {
        pop_library::CatalogAuthority::Snapshot { receipt: Some(receipt.clone()) }.validate().map_err(fail)?;
        let recorded: Handoff = serde_json::from_str(&receipt.freeze).map_err(fail)?;
        if receipt.room != room || recorded != frozen {
            return Err("Different catalog handoff receipt".into());
        }
    }
    // The page can accept edits while CloudKit is awaiting its CAS. Persist and
    // inspect the live owner before changing membership or revoking relay bytes.
    epochs::retry_busy(owner, || Request::Flush)?;
    let Reply::State { reading, .. } = request(owner, Request::State)? else {
        return Err("Owner state unavailable".into());
    };
    let saved_version = verify_saved(owner, &frozen)?;
    if reading.version != saved_version {
        return Err("Local edits changed during handoff. Save again and retry.".into());
    }
    if receipt.is_some_and(|receipt| reading.version != receipt.snapshot_version) {
        return Err("The local document changed after the durable catalog snapshot".into());
    }
    let already_ended = match resolution::epoch_at(origin, room, token) {
        Ok(current) if current.handoff.as_ref() == Some(&frozen) => false,
        Err(transport::HttpFailure::Ended) => true,
        Ok(_) => return Err("The frozen relay changed".into()),
        Err(error) => return Err(error.into()),
    };
    if !already_ended {
        match transport::room_control(
            origin,
            room,
            "POST",
            "handoff/finalize",
            token,
            &serde_json::to_vec(&FinalizeHandoff { token: frozen.token }).map_err(fail)?,
        ) {
            Ok(_) | Err(transport::HttpFailure::Ended) => {}
            Err(error) => return Err(error.into()),
        }
    }
    // Keep retained history until the relay definitively ended. An unknown
    // acknowledgement leaves the original binding in place for a safe retry.
    if receipt.is_some() {
        let share = Share {
            endpoint: pop_storage::LIBRARY_SHARE_ENDPOINT.into(),
            room: owner.app().document_uuid.clone().ok_or("Missing document identity")?,
        };
        epochs::retry_busy(owner, || Request::Share { share: share.clone() })?;
    } else {
        epochs::retry_busy(owner, || Request::Unshare)?;
    }
    Ok(())
}

fn verify_saved(owner: &Owner, frozen: &Handoff) -> Result<String> {
    let store = Store::open(owner.path(), Mode::Snapshot).map_err(fail)?;
    let document = store.document().map_err(fail)?;
    if !document.covers_wire_history(&frozen.heads).map_err(fail)? {
        return Err("The saved document does not cover the frozen relay".into());
    }
    let version = document.version();
    store.close().map_err(fail)?;
    Ok(version)
}
