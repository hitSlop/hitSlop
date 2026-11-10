//! Persistent cloud document control, independent from invitation access.
use crate::{attachments, client::*, journal, transport};
use pop_runtime::owner::Owner;
use pop_sharing_protocol::{
    document::{AccessState, JoinDocument, UpdateDocument},
    http::{CreateRoom, Invitation, OpenRoom, RoomPhase},
};
use pop_storage::store::{Mode, Store};
use std::path::Path;
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
    bootstrap_at(pop_sharing_protocol::TRUSTED_ORIGIN, owner, token, state_directory)
}
fn bootstrap_at(origin: &str, owner: &Owner, token: &str, state_directory: &Path) -> Result<()> {
    let document = owner.app().document_uuid.as_deref().ok_or("Only documents synchronize")?;
    let key = RoomKey { origin: origin.into(), ..RoomKey::identity(document, token)? };
    // Stage a new room's seed first, so a document too large to share fails before any
    // room exists. An existing room keeps its recovery path: a live one only reconnects,
    // even after its document has grown past the initial limit.
    let room = format!("{origin}/v1/documents/{document}");
    let staged = match transport::lookup(&room, token, &[404]).map_err(String::from)? {
        None => Some(stage(owner)?),
        Some(_) => None,
    };
    // An empty acknowledgement: the document's phase is read next.
    control_at(
        origin,
        "POST",
        "documents",
        token,
        &serde_json::to_vec(&CreateRoom { document: document.into() }).map_err(fail)?,
    )?;
    let info = room_info(&key, Some(token))?;
    if info.phase == RoomPhase::Uploading {
        let seed = match staged {
            Some(seed) => seed,
            None => stage(owner)?,
        };
        upload_seed(&key, &seed, token, state_directory)?;
    } else if info.phase != RoomPhase::Live {
        return Err("Cloud document was deleted".into());
    }
    bind_owner(owner, &key, document)
}
/// Upload one staged seed and its attachments, then make exactly that seed live.
/// The relay does not check attachments at Open: every one the seed references is uploaded
/// first, from the same immutable snapshot, whatever the live document has done since.
/// A failure leaves the room uploading for a retry, which sends only missing blobs.
fn upload_seed(key: &RoomKey, seed: &Seed, token: &str, state_directory: &Path) -> Result<()> {
    let journal = journal::Journal::new(state_directory, &key.origin, &key.room)?;
    transport::http(
        key,
        "PUT",
        "seed",
        Some(token),
        &std::fs::read(&seed.path).map_err(fail)?,
        &[
            ("X-Sharing-Version", &serde_json::to_string(&seed.version).map_err(fail)?),
            ("X-Sharing-App", &seed.app),
            ("X-Sharing-SHA256", &seed.hash),
        ],
    )?;
    let store = Store::open(&seed.snapshot, Mode::Snapshot).map_err(fail)?;
    let attachments = attachments::inventory(store.attachments().map_err(fail)?);
    let mut uploaded = attachments::remote_blobs(key)?;
    attachments::upload(key, &attachments, &mut uploaded, |id| Ok(store.attachment(id)?))?;
    store.close().map_err(fail)?;
    let open = OpenRoom { seed_sha256: seed.hash.clone() };
    transport::http(
        key,
        "POST",
        "open",
        Some(token),
        &serde_json::to_vec(&open).map_err(fail)?,
        &[("Content-Type", "application/json")],
    )?;
    Ok(journal::confirm_seed(&journal, seed.owner_version.clone())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pop_runtime::owner::{Reply, Request};
    use pop_sharing_protocol::{hex, http::RoomInfo};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use pop_sharing_protocol::resources::Resource;
    use std::sync::atomic::{AtomicBool, Ordering};
    /// One document's relay state, as the bootstrap routes change it.
    #[derive(Default)]
    struct Room {
        phase: Option<RoomPhase>,
        seed: Option<String>,
        blobs: Vec<Resource>,
        /// `METHOD /path` of every request answered, in order.
        requests: Vec<String>,
        /// The seed each accepted Open named.
        opened: Vec<String>,
        /// Blob uploads fail (503) once this many blobs are stored.
        blob_limit: Option<usize>,
        /// Runs when the seed arrives, after the client staged it.
        after_seed: Option<Box<dyn FnOnce() + Send>>,
    }
    /// A relay double for one document: the bootstrap routes over real HTTP, one request
    /// per connection.
    struct Relay {
        origin: String,
        room: Arc<Mutex<Room>>,
        stop: Arc<AtomicBool>,
        server: Option<std::thread::JoinHandle<()>>,
    }
    impl Relay {
        fn start(document: &str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let room = Arc::new(Mutex::new(Room::default()));
            let stop = Arc::new(AtomicBool::new(false));
            let (serving, stopping, document) = (room.clone(), stop.clone(), document.to_owned());
            let server = std::thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
                    let mut head = Vec::new();
                    while !head.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        stream.read_exact(&mut byte).unwrap();
                        head.push(byte[0]);
                    }
                    let head = String::from_utf8(head).unwrap();
                    let header = |name: &str| {
                        head.lines().find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
                        })
                    };
                    let mut body = vec![0; header("content-length").map_or(0, |n| n.parse().unwrap())];
                    stream.read_exact(&mut body).unwrap();
                    let mut line = head.split(' ');
                    let (method, path) = (line.next().unwrap().to_owned(), line.next().unwrap().to_owned());
                    let (status, reply) =
                        answer(&mut serving.lock().unwrap(), &document, &method, &path, &header, &body);
                    write!(
                        stream,
                        "HTTP/1.1 {status} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        reply.len()
                    )
                    .unwrap();
                    stream.write_all(&reply).unwrap();
                }
            });
            Self { origin, room, stop, server: Some(server) }
        }
        fn room(&self) -> std::sync::MutexGuard<'_, Room> {
            self.room.lock().unwrap()
        }
    }
    impl Drop for Relay {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = self.server.take().map(std::thread::JoinHandle::join);
        }
    }
    fn answer(
        room: &mut Room,
        document: &str,
        method: &str,
        path: &str,
        header: &dyn Fn(&str) -> Option<String>,
        body: &[u8],
    ) -> (u16, Vec<u8>) {
        room.requests.push(format!("{method} {path}"));
        let info = |room: &Room| {
            let phase = room.phase.unwrap();
            let info = RoomInfo {
                document: document.into(),
                phase,
                seed_hash: room.seed.clone(),
                seed_size: None,
                app: None,
                compact: false,
            };
            (200, serde_json::to_vec(&info).unwrap())
        };
        let route = path.strip_prefix(&format!("/v1/documents/{document}")).unwrap_or(path);
        match (method, route) {
            ("GET", "") if room.phase.is_none() => (404, Vec::new()),
            ("GET", "") => info(room),
            ("POST", "/v1/documents") => {
                room.phase.get_or_insert(RoomPhase::Uploading);
                (204, Vec::new())
            }
            ("PUT", "/seed") => {
                room.seed = header("x-sharing-sha256");
                if let Some(after) = room.after_seed.take() {
                    after();
                }
                (200, b"ready".to_vec())
            }
            ("GET", "/blobs") => (200, serde_json::to_vec(&room.blobs).unwrap()),
            ("PUT", blob) if blob.starts_with("/blob/") => {
                if room.blob_limit.is_some_and(|limit| room.blobs.len() >= limit) {
                    return (503, Vec::new());
                }
                let hash = blob["/blob/".len()..].to_owned();
                let media_type = header("content-type").unwrap();
                room.blobs.push(Resource { key: hash.clone(), hash, size: body.len() as u64, media_type });
                (200, b"stored".to_vec())
            }
            ("POST", "/open") => {
                let Ok(open) = serde_json::from_slice::<OpenRoom>(body) else { return (400, Vec::new()) };
                if room.seed.as_ref() != Some(&open.seed_sha256) {
                    return (409, Vec::new());
                }
                room.phase = Some(RoomPhase::Live);
                room.opened.push(open.seed_sha256);
                info(room)
            }
            _ => (404, Vec::new()),
        }
    }
    fn set_title(owner: &Owner, title: &str) {
        let batch = serde_json::json!({"intents":[{"type":"set","path":["title"],"value":title}]});
        request(
            owner,
            Request::Apply { batch: serde_json::from_value(batch).unwrap(), origin: pop_core::Origin::Page },
        )
        .unwrap();
    }
    fn put_attachment(owner: &Owner, bytes: &[u8]) -> String {
        let Reply::Attachment { item } = request(owner, Request::PutAttachment { bytes: bytes.to_vec() }).unwrap()
        else {
            panic!("attachment")
        };
        item.id
    }
    /// Stores attachments the document references, sorted by identity.
    fn referenced(owner: &Owner, blobs: &[&[u8]]) -> Vec<String> {
        let mut ids: Vec<_> = blobs.iter().map(|bytes| put_attachment(owner, bytes)).collect();
        set_title(owner, &ids.join(" "));
        ids.sort();
        ids
    }
    fn stored(room: &Room) -> Vec<String> {
        let mut hashes: Vec<_> = room.blobs.iter().map(|blob| blob.hash.clone()).collect();
        hashes.sort();
        hashes
    }
    fn endpoint(owner: &Owner) -> Option<String> {
        owner.share().unwrap().map(|share| share.endpoint)
    }

    #[test]
    fn an_interrupted_first_upload_stays_uploading_and_a_retry_sends_only_the_missing_blobs() {
        let (directory, owner) = crate::client::tests::fixture();
        let ids = referenced(&owner, &[b"first attachment", b"second attachment"]);
        let document = owner.app().document_uuid.clone().unwrap();
        let relay = Relay::start(&document);
        let state = directory.path().join("relay-state");
        relay.room().blob_limit = Some(1);
        assert!(bootstrap_at(&relay.origin, &owner, "test-token", &state).is_err());
        let first = {
            let room = relay.room();
            assert_eq!(room.phase, Some(RoomPhase::Uploading), "a failed blob upload leaves the room uploading");
            assert!(
                !room.requests.iter().any(|request| request.ends_with("/open")),
                "the relay does not check attachments, so nothing may Open before they are all uploaded: {:?}",
                room.requests
            );
            assert_eq!(room.blobs.len(), 1);
            room.blobs[0].hash.clone()
        };
        assert_eq!(endpoint(&owner), None, "the owner binds only to a live room");
        {
            let mut room = relay.room();
            room.blob_limit = None;
            room.requests.clear();
        }
        bootstrap_at(&relay.origin, &owner, "test-token", &state).unwrap();
        let room = relay.room();
        let sent: Vec<_> =
            room.requests.iter().filter(|request| request.starts_with("PUT ") && request.contains("/blob/")).collect();
        assert_eq!(sent.len(), 1, "the retry uploads only the blob the relay lacks: {sent:?}");
        assert!(!sent[0].ends_with(&first));
        assert_eq!(stored(&room), ids);
        assert_eq!(room.opened, [room.seed.clone().unwrap()], "Open names the seed this attempt uploaded");
        assert_eq!(room.phase, Some(RoomPhase::Live));
        drop(room);
        assert_eq!(endpoint(&owner), Some(relay.origin.clone()));
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    }

    #[test]
    fn the_first_upload_sends_the_staged_attachments_whatever_the_owner_changes_meanwhile() {
        let (directory, owner) = crate::client::tests::fixture();
        let ids = referenced(&owner, &[b"kept attachment", b"attachment removed after staging"]);
        let document = owner.app().document_uuid.clone().unwrap();
        let relay = Relay::start(&document);
        let live = owner.clone();
        let kept = pop_storage::store::Attachment::from_bytes(b"kept attachment").unwrap().id;
        relay.room().after_seed = Some(Box::new(move || {
            // The live document stops referencing one attachment and stores another.
            set_title(&live, &kept);
            put_attachment(&live, b"attachment added after staging");
        }));
        bootstrap_at(&relay.origin, &owner, "test-token", &directory.path().join("relay-state")).unwrap();
        let room = relay.room();
        assert_eq!(stored(&room), ids, "the upload set is the staged snapshot's");
        assert_eq!(room.opened, [room.seed.clone().unwrap()]);
        drop(room);
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    }

    /// The checklist fixture with an incompressible app resource the seed cannot hold.
    fn oversized_app() -> (tempfile::TempDir, Arc<Owner>) {
        let directory = tempfile::tempdir().unwrap();
        let fixture = crate::client::tests::fixture_source();
        let source = directory.path().join("source");
        std::fs::create_dir_all(source.join("assets")).unwrap();
        for entry in std::fs::read_dir(fixture.join("assets")).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), source.join("assets").join(entry.file_name())).unwrap();
        }
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let media: Vec<u8> = (0..(pop_sharing_protocol::SEED_BYTES + (1 << 20)) / 8)
            .flat_map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state.to_le_bytes()
            })
            .collect();
        std::fs::write(source.join("assets/large.bin"), &media).unwrap();
        let mut input: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fixture.join("input.json")).unwrap()).unwrap();
        input["resources"].as_array_mut().unwrap().push(serde_json::json!({
            "kind": "app",
            "key": format!("media/{}.bin", hex(&Sha256::digest(&media))),
            "mediaType": "application/octet-stream",
            "path": "assets/large.bin",
        }));
        crate::client::tests::document(directory, &input.to_string(), &source)
    }

    #[test]
    fn a_document_too_large_to_share_creates_no_room_while_an_existing_room_still_reconnects() {
        let (directory, owner) = oversized_app();
        let document = owner.app().document_uuid.clone().unwrap();
        let relay = Relay::start(&document);
        let state = directory.path().join("relay-state");
        let error = bootstrap_at(&relay.origin, &owner, "test-token", &state).unwrap_err();
        assert!(error.contains("embedded app and saved history exceed the 16 MiB initial sync limit"), "{error}");
        assert_eq!(
            relay.room().requests,
            [format!("GET /v1/documents/{document}")],
            "the seed is checked before any room is created"
        );
        assert_eq!(endpoint(&owner), None);
        // A room that is already live only reconnects, however large the document has grown.
        {
            let mut room = relay.room();
            room.phase = Some(RoomPhase::Live);
            room.seed = Some("c".repeat(64));
            room.requests.clear();
        }
        bootstrap_at(&relay.origin, &owner, "test-token", &state).unwrap();
        let requests = relay.room().requests.clone();
        assert!(!requests.iter().any(|request| request.starts_with("PUT ")), "{requests:?}");
        assert_eq!(endpoint(&owner), Some(relay.origin.clone()));
        request(&owner, Request::Close { preview: None, icon: None }).unwrap();
    }
}
