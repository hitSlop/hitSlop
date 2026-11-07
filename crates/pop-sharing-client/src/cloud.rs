//! Persistent cloud document control, independent from invitation access.
use super::*;
use pop_sharing_protocol::{
    document::{AccessState, JoinDocument, RegisteredDocument, RegisteredDocuments, UpdateDocument},
    http::CreateRoom,
};
pub(crate) fn control(method: &str, route: &str, token: &str, body: &[u8]) -> Result<Vec<u8>> {
    control_at(pop_sharing_protocol::TRUSTED_ORIGIN, method, route, token, body)
}
fn control_at(origin: &str, method: &str, route: &str, token: &str, body: &[u8]) -> Result<Vec<u8>> {
    if token.is_empty() || token.len() > pop_sharing_protocol::limits::IDENTITY_TOKEN_BYTES {
        return Err("Sign in to your account".into());
    }
    transport::http_request(
        method,
        &format!("{origin}/v1/{route}"),
        Some(token),
        body,
        &[("Content-Type", "application/json")],
        pop_sharing_protocol::CONTROL_BYTES as u64,
    )
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(fail)
}
/// The caller's current access, read from the document. Any refusal is an error; see
/// `authorized_access` for the reading that distinguishes a removed permission.
pub fn access(document: &str, token: &str) -> Result<AccessState> {
    RoomKey::identity(document, token)?;
    decode(&control("GET", &format!("documents/{document}/sharing"), token, &[])?)
}
/// Bounds one repair's reads, not the account: a million registrations.
const ACCOUNT_DOCUMENT_PAGES: usize = 1_000;
/// Every registration on the account, read page by page in document order.
pub fn registered_documents(token: &str) -> Result<Vec<RegisteredDocument>> {
    registered_documents_at(pop_sharing_protocol::TRUSTED_ORIGIN, token)
}
fn registered_documents_at(origin: &str, token: &str) -> Result<Vec<RegisteredDocument>> {
    let mut documents = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..ACCOUNT_DOCUMENT_PAGES {
        let route = match &after {
            Some(after) => format!("account/documents?after={after}"),
            None => "account/documents".into(),
        };
        let page: RegisteredDocuments = decode(&control_at(origin, "GET", &route, token, &[])?)?;
        documents.extend(page.documents);
        let Some(next) = page.next else { return Ok(documents) };
        // The cursor only moves forward, so a faulty reply cannot repeat pages.
        if !pop_sharing_protocol::valid_copy(&next) || after.is_some_and(|after| next <= after) {
            return Err("Invalid account document page".into());
        }
        after = Some(next);
    }
    Err("The account lists too many documents to read".into())
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
    let url = format!("{}/v1/documents/{document}/sharing", pop_sharing_protocol::TRUSTED_ORIGIN);
    transport::access_request(&url, token).map_err(String::from)?.map(|bytes| decode(&bytes)).transpose()
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
/// Enable synchronization without replacing the live document, its history or its UUID.
/// The uploaded seed is this document as saved; later edits sync on top of it.
pub fn bootstrap(owner: &Owner, token: &str, state_directory: &Path) -> Result<()> {
    let document = owner.app().document_uuid.as_deref().ok_or("Only documents synchronize")?;
    // An empty acknowledgement: the document's phase is read next.
    control("POST", "documents", token, &serde_json::to_vec(&CreateRoom { document: document.into() }).map_err(fail)?)?;
    let key = RoomKey::identity(document, token)?;
    let info = room_info(&key, Some(token))?;
    if info.phase == RoomPhase::Uploading {
        let journal = journal::Journal::new(state_directory, &key.origin, document)?;
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
            journal::confirm_seed(&journal, seed.owner_version.clone())?;
            Ok(())
        })();
        let _ = std::fs::remove_file(&seed.path);
        if let Some(parent) = seed.path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
        result?;
    } else if info.phase != RoomPhase::Live {
        return Err("Cloud document was deleted".into());
    }
    bind_owner(owner, &key, document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Serves `pages` in order, one connection each, and returns the requested targets.
    fn account_pages(pages: Vec<serde_json::Value>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            pages
                .into_iter()
                .map(|page| {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                    let mut request = Vec::new();
                    while !request.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        if stream.read(&mut byte).unwrap() == 0 {
                            break;
                        }
                        request.push(byte[0]);
                    }
                    let body = page.to_string();
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                    String::from_utf8(request).unwrap().split(' ').nth(1).unwrap().to_owned()
                })
                .collect()
        });
        (origin, server)
    }
    fn id(n: u8) -> String {
        format!("00000000-0000-4000-8000-0000000000{n:02}")
    }
    fn page(documents: &[u8], next: Option<u8>) -> serde_json::Value {
        let documents: Vec<_> =
            documents.iter().map(|n| serde_json::json!({"document":id(*n),"role":"owner"})).collect();
        match next {
            Some(next) => serde_json::json!({"documents":documents,"next":id(next)}),
            None => serde_json::json!({"documents":documents}),
        }
    }

    #[test]
    fn account_documents_are_read_across_every_page() {
        let (origin, server) = account_pages(vec![page(&[1, 2], Some(2)), page(&[3, 4], Some(4)), page(&[5], None)]);
        let documents = registered_documents_at(&origin, "identity").unwrap();
        assert_eq!(
            documents.iter().map(|d| d.document.clone()).collect::<Vec<_>>(),
            (1..=5).map(id).collect::<Vec<_>>()
        );
        assert_eq!(
            server.join().unwrap(),
            [
                "/v1/account/documents".to_owned(),
                format!("/v1/account/documents?after={}", id(2)),
                format!("/v1/account/documents?after={}", id(4)),
            ]
        );
        // A cursor that does not move forward would list the same page forever.
        let (origin, server) = account_pages(vec![page(&[1, 2], Some(2)), page(&[1, 2], Some(2))]);
        assert!(registered_documents_at(&origin, "identity").is_err());
        assert_eq!(server.join().unwrap().len(), 2);
    }
}
