//! Reservation cannot publish a live room. Activation requires matching catalog
//! Preparing authority, committed by the host's CloudKit change-tag CAS.
use super::*;
use pop_library::CatalogAuthority;

pub struct PreparedRoom {
    document: String,
    credential: Credential,
    phase: RoomPhase,
}
pub struct ActivatedRoom {
    pub room: String,
    pub transition: String,
    pub seed_hash: String,
    pub credential: String,
    pub recovery_path: Option<String>,
}
impl PreparedRoom {
    pub fn document(&self) -> &str {
        &self.document
    }
    pub fn room(&self) -> Result<String> {
        Ok(self.credential.runtime()?.room)
    }
    pub fn phase(&self) -> Phase {
        if self.phase == RoomPhase::Uploading { Phase::Preparing } else { Phase::Offline }
    }
    pub fn activate(
        &self,
        owner: &Owner,
        token: &str,
        authority: Option<&CatalogAuthority>,
        state_directory: &Path,
    ) -> Result<ActivatedRoom> {
        if let Some(authority) = authority {
            authority.validate().map_err(fail)?;
        }
        let runtime = self.credential.runtime()?;
        let (transition, expected_seed) = match authority {
            None => ("", None),
            Some(CatalogAuthority::Preparing { transition, room, seed_hash, .. }) if room == &runtime.room => {
                (transition.as_str(), seed_hash.as_deref())
            }
            Some(CatalogAuthority::Relay { transition, room, seed_hash }) if room == &runtime.room => {
                (transition.as_str(), Some(seed_hash.as_str()))
            }
            _ => return Err("A matching committed catalog preparation is required before activation".into()),
        };
        if owner.app().document_uuid.as_deref() != Some(self.document.as_str()) {
            return Err("Different sharing document".into());
        }
        let journal = Arc::new(journal::Journal::new(state_directory, &runtime, &self.document)?);
        let mut recovery_path = None;
        let info = room_info(&runtime, Some(token))?;
        if info.phase == RoomPhase::Uploading {
            if !matches!(authority, None | Some(CatalogAuthority::Preparing { .. })) {
                return Err("A catalog relay cannot activate an unfinished room".into());
            }
            let seed = stage(owner)?;
            let SeedBackup { path, version, app, hash, .. } = &seed;
            let result: Result<()> = (|| {
                if expected_seed.is_some_and(|expected| expected != hash.as_str()) {
                    return Err("The preparing seed changed; reconcile the catalog before activation".into());
                }
                let bytes = std::fs::read(path).map_err(fail)?;
                let version = serde_json::to_string(&version).map_err(fail)?;
                transport::http(
                    &runtime,
                    "PUT",
                    "seed",
                    Some(token),
                    &bytes,
                    &[("X-Sharing-Version", &version), ("X-Sharing-App", app), ("X-Sharing-SHA256", hash)],
                )?;
                let store = Store::open(path, Mode::Snapshot).map_err(fail)?;
                resources::upload_app(&runtime, &store, path, token)?;
                resources::upload_initial_blobs(&runtime, &store, token)?;
                transport::http(&runtime, "POST", "open", Some(token), &[], &[])?;
                store.close().map_err(fail)?;
                let published = resolution::epoch(&runtime, token)?;
                if published.seed_hash != *hash {
                    return Err("The relay epoch changed before activation was confirmed".into());
                }
                recovery_path = epochs::install_prepared(owner, &seed, &published, &journal)?
                    .map(|path| path.to_string_lossy().into_owned());
                Ok(())
            })();
            let _ = std::fs::remove_file(path);
            if let Some(directory) = path.parent() {
                let _ = std::fs::remove_dir(directory);
            }
            result?;
        } else if info.phase == RoomPhase::Live {
            epochs::bootstrap(
                owner,
                &runtime,
                &epochs::Access { journal: journal.clone(), identity: Some(token.into()) },
            )?;
            recovery_path = journal
                .transaction()?
                .current()?
                .and_then(|receipt| receipt.recovery)
                .map(|path| path.to_string_lossy().into_owned());
        } else {
            return Err("Sharing ended".into());
        }
        let epoch = resolution::epoch(&runtime, token).map_err(String::from)?;
        if epoch.handoff.is_some() {
            return Err("A frozen sharing handoff must finish before activation".into());
        }
        bind_owner(owner, &runtime, &self.document)?;
        Ok(ActivatedRoom {
            room: runtime.room,
            transition: transition.to_string(),
            seed_hash: epoch.seed_hash,
            credential: self.credential.encode()?,
            recovery_path,
        })
    }
}
pub fn reserve(document: &str, token: &str) -> Result<PreparedRoom> {
    if !pop_sharing_protocol::valid_copy(document) {
        return Err("Invalid document identity".into());
    }
    let room: CreatedRoom = account_request(
        "POST",
        "rooms",
        token,
        &serde_json::to_vec(&CreateRoom { document: document.into() }).map_err(fail)?,
    )?;
    if !matches!(room.phase, RoomPhase::Uploading | RoomPhase::Live) {
        return Err("Sharing ended".into());
    }
    let credential =
        Credential { link: Invitation { room: room.room, capability: room.capability }.link()?, ended: false };
    Ok(PreparedRoom { document: document.into(), credential, phase: room.phase })
}

/// Cancel only an initial, unpublished reservation. A concurrent activation is
/// refused by the relay's atomic Uploading predicate and must use handoff instead.
pub fn cancel_preparation(document: &str, room: Option<&str>, token: &str) -> Result<Option<String>> {
    if !pop_sharing_protocol::valid_copy(document) || room.is_some_and(|room| !pop_sharing_protocol::valid_room(room)) {
        return Err("Invalid sharing reservation identity".into());
    }
    let room = match room {
        Some(room) => room.to_owned(),
        None => match access(document, token)? {
            Some(room) => room.room,
            None => return Ok(None),
        },
    };
    let Some((credential, info)) = creator_credential_at(document, &room, token, pop_sharing_protocol::TRUSTED_ORIGIN)?
    else {
        return Ok(Some(room));
    };
    if info.phase != RoomPhase::Uploading {
        return Ok(None);
    }
    match transport::http_typed(&credential.runtime()?, "DELETE", "?phase=uploading", Some(token), &[], &[]) {
        Ok(_) | Err(transport::HttpFailure::Ended) => Ok(Some(room)),
        Err(transport::HttpFailure::Conflict) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
