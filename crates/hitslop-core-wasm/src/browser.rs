use hitslop_core::{
    browser_wire::{BrowserEvent as Event, BrowserRequest, BrowserSaveStatus as Status, ResourceRoute},
    owner, page_wire,
};
use rsqlite_vfs::transfer::{DbImport, DbTransfer};
use sha2::{Digest, Sha256};
use sqlite_wasm_vfs::sahpool::{OpfsSAHImportTarget, OpfsSAHPoolCfgBuilder, OpfsSAHPoolUtil, install};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;

use hitslop_sharing_protocol::{
    envelope,
    http::{Invitation, RoomInfo, RoomPhase},
};
fn invitation(link: &str, origin: &str) -> Result<Invitation, JsValue> {
    if origin != hitslop_sharing_protocol::TRUSTED_ORIGIN {
        return Err(error("Untrusted sharing host"));
    }
    Invitation::parse(link).map_err(error)
}
/// Room metadata comes from an authenticated GET, never from the link or authored frame.
fn room_info(json: &str) -> Result<RoomInfo, JsValue> {
    let info: RoomInfo = serde_json::from_str(json).map_err(error)?;
    if info.phase != RoomPhase::Live
        || !hitslop_sharing_protocol::valid_copy(&info.document)
        || !info.seed_hash.as_deref().is_some_and(hitslop_sharing_protocol::resources::hash_id)
        || !info.app.as_deref().is_some_and(hitslop_sharing_protocol::resources::hash_id)
        || !info.seed_size.is_some_and(|size| size >= 100 && size as usize <= hitslop_sharing_protocol::SEED_BYTES)
    {
        return Err(error("Room is not live or has invalid metadata"));
    }
    Ok(info)
}
#[derive(Default)]
struct SyncQueue {
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
    overflow: bool,
    status: String,
}
type SyncEvents = Arc<Mutex<SyncQueue>>;

fn error(value: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&value.to_string())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
type Events = Arc<Mutex<Vec<Event>>>;
fn push(events: &Events, event: Event) {
    events.lock().expect("worker events").push(event);
}

#[derive(Default)]
struct BrowserOs;
impl rsqlite_vfs::OsCallback for BrowserOs {
    fn sleep(&self, _: std::time::Duration) {}
    fn random(&self, bytes: &mut [u8]) -> usize {
        if getrandom::getrandom(bytes).is_ok() { bytes.len() } else { 0 }
    }
    fn epoch_timestamp_in_ms(&self) -> rsqlite_vfs::VfsResult<i64> {
        Ok(js_sys::Date::now() as i64)
    }
}

/// One pool and owner per dedicated worker. The pool must outlive SQLite and imports;
/// its allocation is reclaimed when this worker and its WASM instance terminate.
#[wasm_bindgen]
pub struct BrowserCopy {
    pool: &'static OpfsSAHPoolUtil,
    vfs: String,
    importing: Option<DbImport<OpfsSAHImportTarget<'static>>>,
    imported: u64,
    fresh: bool,
    expected: u64,
    driver: Option<owner::BrowserDriver>,
    events: Events,
    digest: Sha256,
    imported_hash: Option<String>,
    sync_events: SyncEvents,
    sync_link: Option<owner::SyncConnection>,
}

#[wasm_bindgen(js_name = installCopy)]
pub async fn install_copy(copy: &str) -> Result<BrowserCopy, JsValue> {
    if !hitslop_core::browser_wire::valid_copy(copy) {
        return Err(error("Invalid browser copy ID"));
    }
    let config = OpfsSAHPoolCfgBuilder::new().vfs_name(copy).directory(&format!("copies/{copy}")).build();
    let pool = install::<BrowserOs>(&config, false).await.map_err(error)?;
    Ok(BrowserCopy {
        pool: Box::leak(Box::new(pool)),
        vfs: copy.into(),
        importing: None,
        imported: 0,
        fresh: false,
        expected: 0,
        driver: None,
        events: Arc::default(),
        digest: Sha256::new(),
        imported_hash: None,
        sync_events: Arc::default(),
        sync_link: None,
    })
}

#[wasm_bindgen]
impl BrowserCopy {
    #[wasm_bindgen(js_name = importBegin)]
    pub fn import_begin(&mut self, size: u32) -> Result<(), JsValue> {
        if self.driver.is_some()
            || self.importing.is_some()
            || size < 100
            || size as u64 > hitslop_core::browser_wire::FILE_BYTES
        {
            return Err(error("Invalid browser import"));
        }
        self.importing = Some(self.pool.begin_import_unchecked("document.slop", size as u64).map_err(error)?);
        self.imported = 0;
        self.expected = size as u64;
        self.digest = Sha256::new();
        Ok(())
    }
    #[wasm_bindgen(js_name = importWrite)]
    pub fn import_write(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        if bytes.len() > hitslop_core::browser_wire::TRANSFER_BYTES
            || self.imported + bytes.len() as u64 > self.expected
        {
            return Err(error("Import exceeds its declared size"));
        }
        if self.imported == 0
            && (bytes.len() < 100 || &bytes[..16] != b"SQLite format 3\0" || bytes[18] != 1 || bytes[19] != 1)
        {
            return Err(error(
                "Browser imports require a rollback-journal SQLite snapshot; WAL files are not supported",
            ));
        }
        self.importing.as_mut().ok_or_else(|| error("No import is active"))?.write(bytes).map_err(error)?;
        self.digest.update(bytes);
        self.imported += bytes.len() as u64;
        Ok(())
    }
    #[wasm_bindgen(js_name = importFinish)]
    pub fn import_finish(&mut self) -> Result<(), JsValue> {
        if self.imported != self.expected {
            return Err(error("Incomplete import"));
        }
        self.importing.take().ok_or_else(|| error("No import is active"))?.finish().map_err(error)?;
        self.fresh = true;
        self.imported_hash = Some(hex(&self.digest.clone().finalize()));
        Ok(())
    }
    pub fn open(&mut self) -> Result<(), JsValue> {
        if self.driver.is_some() || self.importing.is_some() {
            return Err(error("Copy is already open or importing"));
        }
        let events = self.events.clone();
        let driver = owner::BrowserDriver::open(
            "document.slop",
            &self.vfs,
            self.fresh,
            Arc::new(move |event| match event {
                owner::Event::Publication { json } => push(&events, Event::Publication { json }),
                owner::Event::SaveStatus { status, failure } => push(
                    &events,
                    Event::Save {
                        status: match status {
                            owner::SaveStatus::Saved => Status::Saved,
                            owner::SaveStatus::Saving => Status::Saving,
                            owner::SaveStatus::Failed => Status::Failed,
                        },
                        error: failure.map(|f| f.message),
                    },
                ),
                _ => {}
            }),
        )
        .map_err(error)?;
        driver.owner().attach("browser".into());
        push(
            &self.events,
            Event::Ready {
                title: driver.owner().app().app.metadata().title.clone(),
                window: driver.owner().app().app.window().clone(),
                capabilities: driver.owner().app().app.capabilities().to_vec(),
            },
        );
        self.fresh = false;
        self.driver = Some(driver);
        Ok(())
    }
    /// OPFS may quarantine an incomplete journal slot after an I/O failure. Once
    /// SQLite has closed, restore the pool namespace before reopening its pager.
    /// The owner and its unsaved state stay alive, under the same Web Lock.
    #[wasm_bindgen(js_name = recoverStorage)]
    pub async fn recover_storage(&self) -> Result<(), JsValue> {
        if self.driver.as_ref().is_some_and(|driver| driver.store().browser_needs_recovery()) {
            self.pool.pause().map_err(error)?;
            self.pool.resume().await.map_err(error)?;
        }
        Ok(())
    }
    pub fn request(&mut self, json: &str) -> Result<Option<Vec<u8>>, JsValue> {
        match BrowserRequest::decode(json).map_err(error)? {
            BrowserRequest::Page { id, request } => self.page(id, &request)?,
            BrowserRequest::Flush { id } => self.page(id, r#"{"method":"flush"}"#)?,
            BrowserRequest::ResourcePlan { id, route, key, range } => {
                let route = route_to_file(matches!(route, ResourceRoute::Attachment));
                let info = self
                    .driver
                    .as_ref()
                    .ok_or_else(|| error("Copy is not open"))?
                    .store()
                    .resource_info(route, &key)
                    .map_err(error)?
                    .ok_or_else(|| error("Resource not found"))?;
                let plan = hitslop_core::file::resource_response(&info, route, range.as_deref());
                push(&self.events, Event::ResourcePlan { id, size: info.size, plan });
            }
            BrowserRequest::Resource { id, route, key, offset, length } => {
                let attachment = matches!(route, ResourceRoute::Attachment);
                let store = self.driver.as_ref().ok_or_else(|| error("Copy is not open"))?.store();
                let info = store
                    .resource_info(route_to_file(attachment), &key)
                    .map_err(error)?
                    .ok_or_else(|| error("Resource not found"))?;
                let offset = offset.unwrap_or(0);
                let bytes = self.resource_range(attachment, &key, offset, length.unwrap_or(0))?;
                push(&self.events, Event::Resource { id, size: info.size, media_type: info.media_type, offset });
                return Ok(Some(bytes));
            }
            BrowserRequest::Open { .. } | BrowserRequest::Export { .. } => {
                return Err(error("This request needs a browser operation"));
            }
        }
        Ok(None)
    }
    pub fn page(&self, id: String, request: &str) -> Result<(), JsValue> {
        let driver = self.driver.as_ref().ok_or_else(|| error("Copy is not open"))?;
        let events = self.events.clone();
        hitslop_core::command::page(driver.owner(), "browser".into(), request, move |dispatch| {
            let json = match dispatch {
                hitslop_core::command::PageDispatch::Reply { json, .. } => json,
                hitslop_core::command::PageDispatch::Host { action } => match action {
                    page_wire::HostAction::Environment => {
                        page_wire::HostReply::Environment { environment: page_wire::HostEnvironment::unknown() }
                    }
                    page_wire::HostAction::WindowResize { width, height } => {
                        page_wire::HostReply::WindowResize { width: width as f64, height: height as f64 }
                    }
                    page_wire::HostAction::HapticsPlay { .. } => page_wire::HostReply::HapticsPlay,
                    page_wire::HostAction::Ready => page_wire::HostReply::Ready,
                    page_wire::HostAction::PageRecovered => page_wire::HostReply::PageRecovered,
                    page_wire::HostAction::Failed { error: message } => {
                        push(&events, Event::Error { id: None, error: message });
                        page_wire::HostReply::Failed
                    }
                    page_wire::HostAction::PageError { error: message, .. } => {
                        push(&events, Event::Error { id: None, error: message });
                        page_wire::HostReply::PageError
                    }
                    // A browser copy has no speech service; the page says so.
                    page_wire::HostAction::SpeechTranscribe { .. } => {
                        page_wire::HostReply::SpeechTranscribe { outcome: page_wire::SpeechOutcome::unsupported_host() }
                    }
                }
                .to_json(),
            };
            push(&events, Event::Reply { id, json });
        });
        Ok(())
    }
    /// A negative delay means there is no pending timer.
    pub fn poll(&mut self, monotonic_ms: f64, unix_ms: f64) -> Result<f64, JsValue> {
        let driver = self.driver.as_mut().ok_or_else(|| error("Copy is not open"))?;
        let next = driver.poll(monotonic_ms as u64, unix_ms as u64);
        Ok(next.map_or(-1.0, |n| n as f64))
    }
    pub fn events(&self) -> String {
        serde_json::to_string(&std::mem::take(&mut *self.events.lock().expect("worker events")))
            .expect("browser events")
    }
    #[wasm_bindgen(js_name = resourceInfo)]
    pub fn resource_info(&self, attachment: bool, key: &str) -> Result<String, JsValue> {
        let store = self.driver.as_ref().ok_or_else(|| error("Copy is not open"))?.store();
        let info = store
            .resource_info(route_to_file(attachment), key)
            .map_err(error)?
            .ok_or_else(|| error("Resource not found"))?;
        Ok(serde_json::json!({"size":info.size,"media_type":info.media_type}).to_string())
    }
    #[wasm_bindgen(js_name = resourceRange)]
    pub fn resource_range(&self, attachment: bool, key: &str, offset: u32, length: u32) -> Result<Vec<u8>, JsValue> {
        if length as usize > hitslop_core::browser_wire::TRANSFER_BYTES {
            return Err(error("Resource range is too large"));
        }
        self.driver
            .as_ref()
            .ok_or_else(|| error("Copy is not open"))?
            .store()
            .resource_range(route_to_file(attachment), key, offset as u64, length as u64)
            .map_err(error)?
            .ok_or_else(|| error("Resource not found"))
    }
    /// The page drains drafts first; Rust independently flushes accepted edits.
    /// The serial worker cannot admit writes while this snapshot is streamed.
    pub fn export(&mut self, write: &js_sys::Function) -> Result<(), JsValue> {
        self.driver.as_mut().ok_or_else(|| error("Copy is not open"))?.flush().map_err(error)?;
        self.driver
            .as_ref()
            .ok_or_else(|| error("Copy is not open"))?
            .store()
            .export_pages(|bytes| {
                let bytes = js_sys::Uint8Array::from(bytes);
                write
                    .call1(&JsValue::NULL, &bytes)
                    .map(|_| ())
                    .map_err(|e| hitslop_core::store::Error::Failed(format!("Export write failed: {e:?}")))
            })
            .map_err(error)
    }
}

#[wasm_bindgen]
impl BrowserCopy {
    /// Reopen an installed copy after remote access has ended, without rebinding it.
    #[wasm_bindgen(js_name = openSavedJoined)]
    pub fn open_saved_joined(&mut self, link: &str, origin: &str) -> Result<(), JsValue> {
        let key = invitation(link, origin)?;
        if self.fresh {
            return Err(error("An ended room cannot be joined"));
        }
        self.open()?;
        let driver = self.driver.as_ref().ok_or_else(|| error("Copy missing"))?;
        let share = driver.owner().share().map_err(error)?;
        if !share.is_some_and(|share| share.endpoint == origin && share.room == key.room) {
            self.driver = None;
            self.events.lock().unwrap().clear();
            return Err(error("Saved membership mismatch"));
        }
        Ok(())
    }
    #[wasm_bindgen(js_name = openJoined)]
    pub fn open_joined(&mut self, link: &str, origin: &str, metadata: &str) -> Result<(), JsValue> {
        let key = invitation(link, origin)?;
        let info = room_info(metadata)?;
        if self.fresh && self.imported_hash.as_deref() != info.seed_hash.as_deref() {
            return Err(error("Seed checksum mismatch"));
        }
        // An ordinary local import still calls open(), renewing identity. Joining is
        // a separate operation preserving document identity and binding the validated room.
        let imported = self.fresh;
        self.fresh = false;
        self.open()?;
        let driver = self.driver.as_mut().ok_or_else(|| error("Copy missing"))?;
        let share = driver.owner().share().map_err(error)?;
        if driver.owner().app().document_uuid.as_deref() != Some(info.document.as_str())
            || Some(hex(&driver.store().app_digest().map_err(error)?)).as_deref() != info.app.as_deref()
            || share.is_some_and(|share| {
                share.endpoint != hitslop_core::LIBRARY_SHARE_ENDPOINT
                    && (share.endpoint != origin || (!imported && share.room != key.room))
            })
        {
            self.driver = None;
            self.events.lock().unwrap().clear();
            return Err(error("Seed identity or membership mismatch"));
        }
        let (send, receive) = std::sync::mpsc::channel();
        driver.owner().submit(
            owner::Request::Share { share: hitslop_core::store::Share { room: key.room, endpoint: origin.into() } },
            None,
            Box::new(move |reply| {
                let _ = send.send(reply);
            }),
        );
        driver.poll(0, js_sys::Date::now() as u64);
        receive.try_recv().map_err(|_| error("Sharing membership is still saving"))?.map_err(error)?;
        Ok(())
    }
    #[wasm_bindgen(js_name = syncAttach)]
    pub fn sync_attach(&mut self) -> Result<(), JsValue> {
        let driver = self.driver.as_ref().ok_or_else(|| error("Copy missing"))?;
        *self.sync_events.lock().unwrap() = SyncQueue::default();
        let events = self.sync_events.clone();
        self.sync_link = Some(driver.owner().sync_connection(Arc::new(move |e| {
            let mut q = events.lock().unwrap();
            match e {
                owner::SyncEvent::Reconnect => q.overflow = true,
                owner::SyncEvent::Status { status } => q.status = format!("{status:?}"),
                owner::SyncEvent::Send { bytes } => {
                    if q.overflow {
                        return;
                    }
                    if q.frames.len() >= hitslop_sharing_protocol::QUEUED_FRAMES
                        || q.bytes.saturating_add(bytes.len()) > hitslop_sharing_protocol::QUEUED_BYTES
                    {
                        q.frames.clear();
                        q.bytes = 0;
                        q.overflow = true;
                        return;
                    }
                    q.bytes += bytes.len();
                    q.frames.push_back(bytes);
                }
            }
        })));
        Ok(())
    }
    #[wasm_bindgen(js_name = syncDetach)]
    pub fn sync_detach(&self) {
        if let Some(link) = &self.sync_link {
            link.detach();
        }
    }
    #[wasm_bindgen(js_name = syncFrame)]
    pub fn sync_frame(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let frame = envelope::decode(bytes).map_err(error)?;
        if !self.sync_link.as_ref().ok_or_else(|| error("Connection missing"))?.frame(frame.encode()) {
            return Err(error("Sync queue refused this connection"));
        }
        Ok(())
    }
    #[wasm_bindgen(js_name = nextSyncFrame)]
    pub fn next_sync_frame(&self) -> Result<Option<Vec<u8>>, JsValue> {
        let mut q = self.sync_events.lock().unwrap();
        if q.overflow {
            return Err(error("Socket queue budget exceeded"));
        }
        q.frames
            .pop_front()
            .map(|bytes| {
                q.bytes -= bytes.len();
                envelope::encode(hitslop_core::sync::Frame::decode(&bytes).map_err(error)?).map_err(error)
            })
            .transpose()
    }
    #[wasm_bindgen(js_name = syncStatus)]
    pub fn sync_status(&self) -> String {
        self.sync_events.lock().unwrap().status.clone()
    }
}

#[wasm_bindgen(js_name = invitationRequest)]
pub fn invitation_request(link: &str, origin: &str, metadata: &str) -> Result<String, JsValue> {
    let key = invitation(link, origin)?;
    let info = room_info(metadata)?;
    serde_json::to_string(&hitslop_sharing_protocol::http::ConnectionPlan {
        document: info.document,
        size: info.seed_size.expect("validated size"),
        url: format!("{origin}/v1/rooms/{}/seed", key.room),
        headers: [("Authorization".into(), format!("Bearer {}", key.capability))].into_iter().collect(),
        socket: format!("{}/v1/rooms/{}/sync", origin.replacen("https://", "wss://", 1), key.room),
        protocols: vec![hitslop_sharing_protocol::SOCKET_PROTOCOL.into(), format!("cap.{}", key.capability)],
    })
    .map_err(error)
}

#[wasm_bindgen(js_name = socketBudget)]
pub fn socket_budget() -> u32 {
    hitslop_sharing_protocol::QUEUED_BYTES as u32
}

fn route_to_file(attachment: bool) -> hitslop_core::file::ResourceRoute {
    if attachment { hitslop_core::file::ResourceRoute::Attachment } else { hitslop_core::file::ResourceRoute::App }
}

/// Validate asynchronous browser operations before JavaScript starts them.
#[wasm_bindgen(js_name = decodeBrowserRequest)]
pub fn decode_browser_request(json: &str) -> Result<String, JsValue> {
    serde_json::to_string(&BrowserRequest::decode(json).map_err(error)?).map_err(error)
}

#[path = "resources.rs"]
mod resources;
