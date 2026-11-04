//! Explicit canonical epoch adoption, fenced by durable device-local provenance.
use super::*;
use pop_sharing_protocol::lifecycle::EpochState;
use std::path::PathBuf;

pub(crate) struct Access {
    pub journal: Arc<journal::Journal>,
    pub identity: Option<String>,
}
pub(crate) fn local(error: impl std::fmt::Display) -> transport::HttpFailure {
    transport::HttpFailure::Paused(pop_core::sync::Pause { kind: PauseKind::Refused, reason: error.to_string() })
}
pub(crate) fn retry_busy(
    owner: &Owner,
    build: impl Fn() -> Request,
) -> std::result::Result<Reply, transport::HttpFailure> {
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match request(owner, build()) {
            Err(transport::HttpFailure::Busy(_)) if Instant::now() < deadline => {
                transport::pause(std::time::Duration::from_millis(25))?
            }
            result => return result,
        }
    }
}
pub(crate) fn install_prepared(
    owner: &Owner,
    seed: &SeedBackup,
    epoch: &EpochState,
    journal: &journal::Journal,
) -> std::result::Result<Option<PathBuf>, transport::HttpFailure> {
    let transaction = journal.transaction().map_err(local)?;
    let Reply::State { reading, .. } = request(owner, Request::State)? else {
        return Err(local("Owner state unavailable"));
    };
    if let Some(pending) = transaction.pending().map_err(local)?
        && pending.epoch.seed_hash == epoch.seed_hash
        && pending.owner_version == seed.epoch_version
        && reading.version == seed.epoch_version
    {
        if pending.recovery.as_ref().is_some_and(|path| !recovery::accepted(path, owner.app().document_uuid.as_deref()))
        {
            return Err(local("The pending recovery file is not an accepted independent document"));
        }
        transaction.promote(&pending).map_err(local)?;
        return Ok(pending.recovery);
    }
    let recovery = if reading.version != seed.source_version {
        Some(journal.recovery_destination(&reading.version).map_err(local)?)
    } else {
        None
    };
    let receipt = journal.receipt(epoch.clone(), seed.epoch_version.clone(), recovery.clone());
    transaction.prepare(&receipt).map_err(local)?;
    let Reply::EpochAdopted { recovery_path } = retry_busy(owner, || Request::AdoptEpoch {
        source: seed.path.clone().into(),
        expected_version: seed.source_version.clone(),
        recovery_destination: recovery.clone().map(Into::into),
    })?
    else {
        return Err(local("Owner did not acknowledge prepared epoch installation"));
    };
    transaction.promote(&receipt).map_err(local)?;
    Ok(recovery_path.map(|path| path.native_path().to_path_buf()))
}
pub(crate) fn bootstrap(
    owner: &Owner,
    key: &RoomKey,
    access: &Access,
) -> std::result::Result<(), transport::HttpFailure> {
    let token = access.identity.as_deref().unwrap_or(&key.capability);
    let info = room_info(key, Some(token))?;
    if info.phase != RoomPhase::Live {
        return Err(local("Sharing is not live"));
    }
    if owner.app().document_uuid.as_deref() != Some(info.document.as_str()) {
        return Err(local("Different sharing document"));
    }
    let epoch = resolution::epoch(key, token)?;
    {
        let transaction = access.journal.transaction().map_err(local)?;
        if transaction.current().map_err(local)?.is_some_and(|receipt| receipt.epoch.seed_hash == epoch.seed_hash) {
            let saved = Store::open(owner.path(), Mode::Snapshot).map_err(transport::HttpFailure::from)?;
            let included = saved
                .document()
                .map_err(transport::HttpFailure::from)?
                .covers_wire_history(&epoch.root)
                .map_err(transport::HttpFailure::from)?;
            saved.close().map_err(transport::HttpFailure::from)?;
            if included {
                return Ok(());
            }
        }
    }
    let credential = Credential {
        link: Invitation { room: key.room.clone(), capability: key.capability.clone() }.link()?,
        ended: false,
    }
    .encode()?;
    let resolved = resolution::download(key.clone(), info, epoch, credential, token)?;
    adopt(owner, &resolved, &access.journal)?;
    Ok(())
}
pub(crate) fn adopt(
    owner: &Owner,
    resolved: &ResolvedRoom,
    journal: &journal::Journal,
) -> std::result::Result<Option<PathBuf>, transport::HttpFailure> {
    let transaction = journal.transaction().map_err(local)?;
    let incoming = Store::open(&resolved.seed.path, Mode::Snapshot).map_err(transport::HttpFailure::from)?;
    let incoming_version = incoming.document().map_err(transport::HttpFailure::from)?.version();
    incoming.close().map_err(transport::HttpFailure::from)?;
    let Reply::State { reading, .. } = request(owner, Request::State)? else {
        return Err(local("Owner state unavailable"));
    };
    if let Some(pending) = transaction.pending().map_err(local)?
        && pending.epoch.seed_hash == resolved.epoch.seed_hash
        && pending.owner_version == incoming_version
        && reading.version == incoming_version
    {
        if let Some(path) = &pending.recovery {
            let recovery = Store::open(path, Mode::Snapshot).map_err(transport::HttpFailure::from)?;
            if recovery.app().document_uuid.as_deref() == Some(resolved.document())
                || recovery.share().map_err(transport::HttpFailure::from)?.is_some()
            {
                return Err(local("The pending recovery file is not an independent document"));
            }
            recovery.close().map_err(transport::HttpFailure::from)?;
        }
        transaction.promote(&pending).map_err(local)?;
        return Ok(pending.recovery);
    }
    let expected = transaction.current().map_err(local)?.map_or(String::new(), |receipt| receipt.owner_version);
    let recovery = if reading.version != expected {
        Some(journal.recovery_destination(&reading.version).map_err(local)?)
    } else {
        None
    };
    let receipt = journal.receipt(resolved.epoch.clone(), incoming_version, recovery.clone());
    transaction.prepare(&receipt).map_err(local)?;
    let Reply::EpochAdopted { recovery_path } = retry_busy(owner, || Request::AdoptEpoch {
        source: resolved.seed.path.clone().into(),
        expected_version: expected.clone(),
        recovery_destination: recovery.clone().map(Into::into),
    })?
    else {
        return Err(local("Owner did not acknowledge epoch adoption"));
    };
    transaction.promote(&receipt).map_err(local)?;
    Ok(recovery_path.map(|path| path.native_path().to_path_buf()))
}

pub(crate) fn confirm(
    owner: &Owner,
    connection: &pop_runtime::owner::SyncConnection,
    access: &Access,
    epoch: &mut EpochState,
) -> std::result::Result<bool, transport::HttpFailure> {
    let (send, receive) = std::sync::mpsc::channel();
    connection.confirmed_version(move |confirmed| {
        let _ = send.send(confirmed);
    });
    let Some(confirmed) = transport::wait_reply(&receive, std::time::Duration::from_secs(20))? else {
        return Ok(false);
    };
    // A failed save cannot advance the authority proof. Native SaveStatus still
    // owns its retry UI; leave the old receipt and allow epoch recovery/maintenance.
    if request(owner, Request::Flush).is_err() {
        return Ok(false);
    }
    let Reply::State { reading, .. } = request(owner, Request::State)? else {
        return Err(local("Owner state unavailable"));
    };
    if reading.version != confirmed.version {
        return Ok(false);
    }
    epoch.heads = confirmed.heads;
    let transaction = access.journal.transaction().map_err(local)?;
    let recovery = transaction.current().map_err(local)?.and_then(|receipt| receipt.recovery);
    let receipt = access.journal.receipt(epoch.clone(), confirmed.version, recovery);
    transaction.confirm(&receipt).map_err(local)?;
    Ok(true)
}

/// Publish an accepted fresh layout only against the exact relay frontier used to prepare it.
pub(crate) fn publish(
    owner: &Owner,
    key: &RoomKey,
    access: &Access,
    expected: pop_sharing_protocol::lifecycle::EpochBase,
) -> std::result::Result<(), transport::HttpFailure> {
    use pop_sharing_protocol::{lifecycle::PublishEpoch, resources::Resource};
    expected.validate().map_err(local)?;
    let seed = stage_required(owner, Some(expected.heads.clone()))?;
    let result = (|| {
        let store = Store::open(&seed.path, Mode::Snapshot).map_err(transport::HttpFailure::from)?;
        let attachments = store.attachments().map_err(transport::HttpFailure::from)?;
        let publication = PublishEpoch {
            expected,
            seed_hash: seed.hash.clone(),
            root: seed.version.clone(),
            resources: store.sharing_manifest().map_err(transport::HttpFailure::from)?,
            attachments: attachments
                .iter()
                .map(|item| Resource {
                    key: item.id.clone(),
                    hash: item.id.clone(),
                    size: item.bytes,
                    media_type: item.media_type.clone(),
                })
                .collect(),
        };
        publication.validate().map_err(local)?;
        let token = access.identity.as_deref().unwrap_or(&key.capability);
        let bytes = std::fs::read(&seed.path).map_err(local)?;
        transport::http_typed(
            key,
            "PUT",
            &format!("epochs/{}", seed.hash),
            Some(token),
            &bytes,
            &[("Content-Type", "application/octet-stream")],
        )?;
        for item in &attachments {
            let bytes = store.attachment(&item.id).map_err(transport::HttpFailure::from)?;
            transport::http_typed(
                key,
                "PUT",
                &format!("epochs/{}/blobs/{}", seed.hash, item.id),
                Some(token),
                &bytes,
                &[("Content-Type", &item.media_type)],
            )?;
        }
        store.close().map_err(transport::HttpFailure::from)?;
        // Persist the candidate before the server can make it authoritative. A lost
        // acknowledgement can then be reconciled by its immutable seed identity.
        {
            let transaction = access.journal.transaction().map_err(local)?;
            let receipt = access.journal.receipt(
                EpochState {
                    seed_hash: seed.hash.clone(),
                    root: seed.version.clone(),
                    heads: seed.version.clone(),
                    handoff: None,
                },
                seed.epoch_version.clone(),
                None,
            );
            transaction.prepare(&receipt).map_err(local)?;
        }
        transport::http_typed(
            key,
            "POST",
            "epoch",
            Some(token),
            &serde_json::to_vec(&publication).map_err(local)?,
            &[("Content-Type", "application/json")],
        )?;
        let epoch = resolution::epoch(key, token)?;
        if epoch.seed_hash != seed.hash {
            return Err(transport::HttpFailure::Conflict);
        }
        install_prepared(owner, &seed, &epoch, &access.journal)?;
        Ok(())
    })();
    // Releasing a committed stage is refused with 409; an uncertain publication
    // therefore cannot delete its authoritative seed. Losing candidates free leases.
    let token = access.identity.as_deref().unwrap_or(&key.capability);
    let _ = transport::http_typed(key, "DELETE", &format!("epochs/{}", seed.hash), Some(token), &[], &[]);
    let _ = std::fs::remove_file(&seed.path);
    if let Some(directory) = seed.path.parent() {
        let _ = std::fs::remove_dir(directory);
    }
    result
}
