//! Persistent cloud document control, independent from invitation access.
use super::*;
use pop_sharing_protocol::{
    document::{AccessState, JoinDocument, UpdateDocument},
    http::{CreateRoom, CreatedRoom},
};
pub(crate) fn control(method: &str, route: &str, token: &str, body: &[u8]) -> Result<Vec<u8>> {
    if token.is_empty() || token.len() > pop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES {
        return Err("Sign in to your account".into());
    }
    transport::http_request(
        method,
        &format!("{}/v1/{route}", pop_sharing_protocol::TRUSTED_ORIGIN),
        Some(token),
        body,
        &[("Content-Type", "application/json")],
        pop_sharing_protocol::CONTROL_BYTES as u64,
    )
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(fail)
}
pub fn access(document: &str, token: &str) -> Result<AccessState> {
    RoomKey::identity(document, token)?;
    decode(&control("GET", &format!("account/documents/{document}"), token, &[])?)
}
pub fn registered_documents(token: &str) -> Result<pop_sharing_protocol::document::RegisteredDocuments> {
    decode(&control("GET", "account/documents", token, &[])?)
}
pub fn document_metadata(document: &str, token: &str) -> Result<pop_sharing_protocol::document::DocumentMetadata> {
    ready_document_metadata(document, token)?.ok_or_else(|| "Cloud document has not finished uploading".into())
}
pub fn ready_document_metadata(
    document: &str,
    token: &str,
) -> Result<Option<pop_sharing_protocol::document::DocumentMetadata>> {
    RoomKey::identity(document, token)?;
    match transport::room_control(pop_sharing_protocol::TRUSTED_ORIGIN, document, "GET", "metadata", token, &[]) {
        Ok(bytes) => decode(&bytes).map(Some),
        Err(transport::HttpFailure::Conflict) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
/// Only a definite permission refusal/deletion removes a discovery reference;
/// expired credentials and network failures remain retryable account errors.
pub fn authorized_access(document: &str, token: &str) -> Result<Option<AccessState>> {
    RoomKey::identity(document, token)?;
    match transport::room_control(pop_sharing_protocol::TRUSTED_ORIGIN, document, "GET", "sharing", token, &[]) {
        Ok(bytes) => decode(&bytes).map(Some),
        Err(transport::HttpFailure::Ended) => Ok(None),
        Err(transport::HttpFailure::Paused(pause)) if pause.kind == pop_core::sync::PauseKind::Refused => Ok(None),
        Err(error) => Err(error.into()),
    }
}
pub fn set_sharing(document: &str, enabled: bool, token: &str) -> Result<AccessState> {
    RoomKey::identity(document, token)?;
    decode(&control(if enabled { "POST" } else { "DELETE" }, &format!("documents/{document}/sharing"), token, &[])?)
}
pub fn sharing_link(access: &AccessState) -> Result<Option<String>> {
    if !access.sharing {
        return Ok(None);
    }
    let capability = access.capability.as_deref().ok_or("Only the owner can read the invitation")?;
    Invitation { room: access.document.clone(), capability: capability.into() }.link().map(Some)
}
pub fn join_member(link: &str, token: &str) -> Result<AccessState> {
    let invitation = Invitation::parse(link)?;
    decode(&control(
        "POST",
        &format!("documents/{}/members", invitation.room),
        token,
        &serde_json::to_vec(&JoinDocument { capability: invitation.capability }).map_err(fail)?,
    )?)
}
pub fn leave(document: &str, token: &str) -> Result<()> {
    RoomKey::identity(document, token)?;
    control("DELETE", &format!("documents/{document}/members/me"), token, &[]).map(|_| ())
}
pub fn delete_document(document: &str, token: &str) -> Result<()> {
    RoomKey::identity(document, token)?;
    control("DELETE", &format!("documents/{document}"), token, &[]).map(|_| ())
}
pub fn update_document(document: &str, title: &str, template_id: &str, token: &str) -> Result<()> {
    RoomKey::identity(document, token)?;
    control(
        "PATCH",
        &format!("documents/{document}"),
        token,
        &serde_json::to_vec(&UpdateDocument { title: title.into(), template_id: template_id.into() }).map_err(fail)?,
    )
    .map(|_| ())
}
/// Enable synchronization without replacing the live document or its UUID.
pub fn bootstrap(owner: &Owner, token: &str, state_directory: &Path) -> Result<()> {
    let document = owner.app().document_uuid.as_deref().ok_or("Only documents synchronize")?;
    let _created: CreatedRoom = decode(&control(
        "POST",
        "documents",
        token,
        &serde_json::to_vec(&CreateRoom { document: document.into() }).map_err(fail)?,
    )?)?;
    let key = RoomKey::identity(document, token)?;
    let journal = Arc::new(journal::Journal::new(state_directory, &key, document)?);
    let info = room_info(&key, Some(token))?;
    if info.phase == RoomPhase::Uploading {
        let seed = stage(owner)?;
        let result: Result<()> = (|| {
            let bytes = std::fs::read(&seed.path).map_err(fail)?;
            transport::http(
                &key,
                "PUT",
                "seed",
                Some(token),
                &bytes,
                &[
                    ("X-Sharing-Version", &serde_json::to_string(&seed.version).map_err(fail)?),
                    ("X-Sharing-App", &seed.app),
                    ("X-Sharing-SHA256", &seed.hash),
                ],
            )?;
            let store = Store::open(&seed.path, Mode::Snapshot).map_err(fail)?;
            resources::upload_app(&key, &store, &seed.path, token)?;
            resources::upload_initial_blobs(&key, &store, token)?;
            transport::http(&key, "POST", "open", Some(token), &[], &[])?;
            store.close().map_err(fail)?;
            let epoch = resolution::epoch(&key, token)?;
            epochs::install_prepared(owner, &seed, &epoch, &journal)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(&seed.path);
        if let Some(parent) = seed.path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
        result?;
    } else if info.phase == RoomPhase::Live {
        epochs::bootstrap(owner, &key, &epochs::Access { journal, identity: Some(token.into()) })?;
    } else {
        return Err("Cloud document was deleted".into());
    }
    bind_owner(owner, &key, document)
}
