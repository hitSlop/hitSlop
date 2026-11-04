use pop_core::{
    browser_wire::{BrowserEvent as Event, BrowserRequest, BrowserSaveStatus as Status, ResourceRoute},
    page_wire,
};
use pop_runtime::owner;
use rsqlite_vfs::VfsFilesManager;
use rsqlite_vfs::transfer::{DbImport, DbTransfer};
use sha2::{Digest, Sha256};
use sqlite_wasm_vfs::sahpool::{OpfsSAHImportTarget, OpfsSAHPoolCfgBuilder, OpfsSAHPoolUtil, install};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;

use pop_sharing_protocol::{
    envelope, failure, frame,
    http::{Invitation, RoomInfo, RoomPhase},
};
fn invitation(link: &str, origin: &str) -> Result<Invitation, JsValue> {
    if origin != pop_sharing_protocol::TRUSTED_ORIGIN {
        return Err(error("Untrusted sharing host"));
    }
    Invitation::parse(link).map_err(error)
}
/// Room metadata comes from an authenticated GET, never from the link or authored frame.
fn room_info(json: &str) -> Result<RoomInfo, JsValue> {
    let info: RoomInfo = serde_json::from_str(json).map_err(error)?;
    if info.phase != RoomPhase::Live
        || !pop_sharing_protocol::valid_copy(&info.document)
        || !info.seed_hash.as_deref().is_some_and(pop_sharing_protocol::resources::hash_id)
        || !info.app.as_deref().is_some_and(pop_sharing_protocol::resources::hash_id)
        || !info.seed_size.is_some_and(|size| size >= 100 && size as usize <= pop_sharing_protocol::SEED_BYTES)
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
    /// The owner's last report; `None` until it has made one for this connection.
    state: Option<pop_core::sync::SyncState>,
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

/// The live copy's pool outlives its owner and imports. Closed transfer/recovery
/// pools are explicitly released after the serial storage effect finishes.
#[wasm_bindgen]
pub struct BrowserCopy {
    pool: Option<&'static OpfsSAHPoolUtil>,
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
    accepted_epoch: Option<pop_sharing_protocol::lifecycle::EpochState>,
    transfer_active: Arc<AtomicBool>,
    epoch_supported: std::cell::Cell<Option<bool>>,
}

#[wasm_bindgen(js_name = installCopy)]
pub async fn install_copy(copy: &str) -> Result<BrowserCopy, JsValue> {
    if !pop_core::browser_wire::valid_copy(copy) {
        return Err(error("Invalid browser copy ID"));
    }
    let config = OpfsSAHPoolCfgBuilder::new().vfs_name(copy).directory(&format!("copies/{copy}")).build();
    let pool = install::<BrowserOs>(&config, false).await.map_err(error)?;
    if pool.is_paused() {
        pool.resume().await.map_err(error)?;
    }
    Ok(BrowserCopy {
        pool: Some(Box::leak(Box::new(pool))),
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
        accepted_epoch: None,
        transfer_active: Arc::default(),
        epoch_supported: std::cell::Cell::new(None),
    })
}

#[wasm_bindgen]
impl BrowserCopy {
    #[wasm_bindgen(js_name = importBegin)]
    pub fn import_begin(&mut self, size: u32) -> Result<(), JsValue> {
        if self.driver.is_some()
            || self.importing.is_some()
            || size < 100
            || size as u64 > pop_core::browser_wire::FILE_BYTES
        {
            return Err(error("Invalid browser import"));
        }
        self.accepted_epoch = None;
        self.importing = Some(self.pool()?.begin_import_unchecked("document.pop", size as u64).map_err(error)?);
        self.imported = 0;
        self.expected = size as u64;
        self.digest = Sha256::new();
        Ok(())
    }
    #[wasm_bindgen(js_name = importWrite)]
    pub fn import_write(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        if bytes.len() > pop_core::browser_wire::TRANSFER_BYTES || self.imported + bytes.len() as u64 > self.expected {
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
            "document.pop",
            &self.vfs,
            self.fresh,
            Arc::new(move |event| match event {
                owner::Event::Publication { json } => push(&events, Event::Publication { json }),
                owner::Event::SaveStatus { status, failure, .. } => push(
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
            self.pool()?.pause().map_err(error)?;
            self.pool()?.resume().await.map_err(error)?;
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
                let plan = pop_storage::file::resource_response(&info, route, range.as_deref());
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
        pop_runtime::command::page(driver.owner(), "browser".into(), request, move |dispatch| {
            let json = match dispatch {
                pop_runtime::command::PageDispatch::Reply { json, .. } => json,
                pop_runtime::command::PageDispatch::Host { action } => match action {
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
        if length as usize > pop_core::browser_wire::TRANSFER_BYTES {
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
                    .map_err(|e| pop_storage::store::Error::Failed(format!("Export write failed: {e:?}")))
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
        // A prior terminal-room cleanup may have cleared membership before its
        // flush/catalog acknowledgment was interrupted. Keep that local copy open.
        if share.is_some_and(|share| share.endpoint != origin || share.room != key.room) {
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
                share.endpoint != pop_storage::LIBRARY_SHARE_ENDPOINT
                    && (share.endpoint != origin || (!imported && share.room != key.room))
            })
        {
            self.driver = None;
            self.events.lock().unwrap().clear();
            return Err(error("Seed identity or membership mismatch"));
        }
        let (send, receive) = std::sync::mpsc::channel();
        driver.owner().submit(
            owner::Request::Share { share: pop_storage::store::Share { room: key.room, endpoint: origin.into() } },
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
                owner::SyncEvent::Status { status } => {
                    q.state = Some(status.state());
                }
                owner::SyncEvent::Send { bytes } => {
                    if q.overflow {
                        return;
                    }
                    if q.frames.len() >= pop_sharing_protocol::QUEUED_FRAMES
                        || q.bytes.saturating_add(bytes.len()) > pop_sharing_protocol::QUEUED_BYTES
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
                envelope::encode(pop_core::sync::Frame::decode(&bytes).map_err(error)?).map_err(error)
            })
            .transpose()
    }
    /// `{phase, pause?}` as of the owner's last report, for the worker to route on.
    #[wasm_bindgen(js_name = syncState)]
    pub fn sync_state(&self) -> Option<String> {
        self.sync_events.lock().unwrap().state.as_ref().map(|s| serde_json::to_string(s).expect("sync state"))
    }
    #[wasm_bindgen(js_name = pauseSync)]
    pub fn pause_sync(&self, pause: &str) -> Result<(), JsValue> {
        let pause: pop_core::sync::Pause = serde_json::from_str(pause).map_err(error)?;
        if let Some(link) = &self.sync_link {
            link.pause(pause.kind, pause.reason);
        } else {
            // A permanent HTTP refusal may precede the first WebSocket admission.
            self.sync_events.lock().unwrap().state =
                Some(pop_core::sync::Status::Paused { kind: pause.kind, reason: pause.reason }.state());
        }
        Ok(())
    }
    #[wasm_bindgen(js_name = syncBase)]
    pub fn sync_base(&mut self, monotonic: u64, unix: u64) -> Result<Option<SharingBase>, JsValue> {
        let link = self.sync_link.as_ref().ok_or_else(|| error("Connection missing"))?;
        let (send, receive) = std::sync::mpsc::channel();
        link.base(move |base| {
            let _ = send.send(base);
        });
        self.driver.as_mut().ok_or_else(|| error("Copy missing"))?.poll(monotonic, unix);
        match receive.try_recv().map_err(error)? {
            Ok(base) => Ok(base.map(SharingBase)),
            Err(pause) => {
                link.pause(pause.kind, pause.reason);
                self.driver.as_mut().expect("checked above").poll(monotonic, unix);
                Ok(None)
            }
        }
    }
}

#[wasm_bindgen(js_name = invitationRequest)]
pub fn invitation_request(link: &str, origin: &str, metadata: &str) -> Result<String, JsValue> {
    let key = invitation(link, origin)?;
    let info = room_info(metadata)?;
    serde_json::to_string(&pop_sharing_protocol::http::ConnectionPlan {
        document: info.document,
        size: info.seed_size.expect("validated size"),
        url: format!("{origin}/v1/rooms/{}/seed", key.room),
        headers: [("Authorization".into(), format!("Bearer {}", key.capability))].into_iter().collect(),
        socket: format!("{}/v1/rooms/{}/sync", origin.replacen("https://", "wss://", 1), key.room),
        protocols: vec![pop_sharing_protocol::SOCKET_PROTOCOL.into(), format!("cap.{}", key.capability)],
    })
    .map_err(error)
}

/// What a failed relay request means (`{outcome, message}`), from the table the native client uses.
#[wasm_bindgen(js_name = classifyFailure)]
pub fn classify_failure(status: u16, code: Option<String>, url: &str) -> String {
    let outcome = failure::classify(status, code.as_deref(), failure::Route::of(url));
    serde_json::to_string(&failure::Failure { outcome, message: outcome.message(status) })
        .expect("failure is serializable")
}

/// Apply the same compaction policy as the native supervisor.
#[wasm_bindgen(js_name = maintenanceAction)]
pub fn maintenance_action(full: &str, step: &str, random: u16) -> Result<String, JsValue> {
    use pop_sharing_protocol::maintenance;
    let full = serde_json::from_str(full).map_err(error)?;
    let step = serde_json::from_str(step).map_err(error)?;
    serde_json::to_string(&maintenance::decide(full, step, random)).map_err(error)
}

#[wasm_bindgen(js_name = socketBudget)]
pub fn socket_budget() -> u32 {
    pop_sharing_protocol::QUEUED_BYTES as u32
}

fn route_to_file(attachment: bool) -> pop_storage::file::ResourceRoute {
    if attachment { pop_storage::file::ResourceRoute::Attachment } else { pop_storage::file::ResourceRoute::App }
}

/// Validate asynchronous browser operations before JavaScript starts them.
#[wasm_bindgen(js_name = decodeBrowserRequest)]
pub fn decode_browser_request(json: &str) -> Result<String, JsValue> {
    serde_json::to_string(&BrowserRequest::decode(json).map_err(error)?).map_err(error)
}

#[path = "resources.rs"]
mod resources;

#[wasm_bindgen]
pub struct SharingBase(pop_core::sync::Base);
#[wasm_bindgen]
impl SharingBase {
    pub fn bytes(&self) -> Vec<u8> {
        self.0.bytes.clone()
    }
    pub fn frontier(&self) -> String {
        frame::version_json(self.0.frontier.iter().map(|(&p, &n)| (p, n as u32)))
    }
    pub fn root(&self) -> String {
        frame::version_json(self.0.start.iter().map(|(&p, &n)| (p, n as u32)))
    }
}

// Epoch transfers use separate installed VFSs. The live owner's file is only changed
// by its serial adoption effect, after any independent recovery copy is durable.
#[wasm_bindgen]
impl BrowserCopy {
    /// Detect a cutoff before downloading resources into the obsolete local file.
    /// Epoch roots are installed durably before owner adoption; ordinary unsaved edits
    /// cannot remove them, so the accepted saved frontier is sufficient here.
    #[wasm_bindgen(js_name = leaveRoom)]
    pub fn leave_room(&mut self) -> Result<(), JsValue> {
        let driver = self.driver.as_mut().ok_or_else(|| error("Copy is not open"))?;
        let (send, receive) = std::sync::mpsc::channel();
        driver.owner().submit(
            owner::Request::Unshare,
            None,
            Box::new(move |reply| {
                let _ = send.send(reply);
            }),
        );
        driver.poll(0, js_sys::Date::now() as u64);
        receive.try_recv().map_err(error)?.map_err(error)?;
        // Clear the shared retention policy before flushing: shared history may have
        // reached its limit, while the latest local state still fits. Keep this owner
        // and accepted edits alive if either operation cannot be acknowledged.
        driver.flush().map_err(error)
    }

    #[wasm_bindgen(js_name = hasEpoch)]
    pub fn has_epoch(&self, json: &str) -> Result<bool, JsValue> {
        let epoch: pop_sharing_protocol::lifecycle::EpochState = serde_json::from_str(json).map_err(error)?;
        epoch.validate().map_err(error)?;
        self.driver
            .as_ref()
            .ok_or_else(|| error("Copy is not open"))?
            .store()
            .document()
            .map_err(error)?
            .covers_wire_history(&epoch.root)
            .map_err(error)
    }

    #[wasm_bindgen(js_name = confirmEpoch)]
    pub fn confirm_epoch(&self, json: &str) -> Result<(), JsValue> {
        let receipt: pop_sharing_protocol::lifecycle::EpochState = serde_json::from_str(json).map_err(error)?;
        receipt.validate().map_err(error)?;
        let candidate = self.accepted_epoch.as_ref().ok_or_else(|| error("No accepted epoch candidate"))?;
        if receipt.seed_hash != candidate.seed_hash || receipt.root != candidate.root {
            return Err(error("Relay did not confirm the prepared epoch"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = supportsEpoch)]
    pub fn supports_epoch(&self) -> Result<bool, JsValue> {
        if let Some(supported) = self.epoch_supported.get() {
            return Ok(supported);
        }
        let supported = self
            .driver
            .as_ref()
            .ok_or_else(|| error("Copy is not open"))?
            .store()
            .document()
            .map_err(error)?
            .supports_deep_compaction();
        // Layout never migrates during ordinary edits; authority adoption only accepts
        // layout 2 on both sides. Do not reopen Loro on every paused transport tick.
        self.epoch_supported.set(Some(supported));
        Ok(supported)
    }

    /// Prepare a private SQLite snapshot through the same serial owner effect as
    /// native sharing. The expected relay frontier is checked before history is cut.
    #[wasm_bindgen(js_name = prepareEpoch)]
    pub fn prepare_epoch(&mut self, candidate: &mut BrowserCopy, expected: &str) -> Result<String, JsValue> {
        use pop_sharing_protocol::lifecycle::{EpochBase, EpochState, PublishEpoch};
        use pop_sharing_protocol::resources::Resource;
        let expected: EpochBase = serde_json::from_str(expected).map_err(error)?;
        expected.validate().map_err(error)?;
        if candidate.driver.is_some() || candidate.importing.is_some() || candidate.vfs == self.vfs {
            return Err(error("Invalid epoch staging destination"));
        }
        candidate.accepted_epoch = None;
        candidate.clear_epoch_scratch()?;
        let source_version = self.current_version()?;
        let driver = self.driver.as_mut().expect("current version requires open owner");
        let (send, receive) = std::sync::mpsc::channel();
        let active = candidate.transfer_active.clone();
        active.store(true, Ordering::Release);
        driver.owner().submit(
            owner::Request::PrepareEpoch {
                destination: pop_storage::store::EpochLocation::Vfs {
                    name: "document.pop".into(),
                    vfs: candidate.vfs.clone(),
                },
                expected_version: source_version.clone(),
                required_heads: Some(expected.heads.clone()),
            },
            None,
            Box::new(move |reply| {
                active.store(false, Ordering::Release);
                let _ = send.send(reply);
            }),
        );
        driver.poll(0, js_sys::Date::now() as u64);
        let result = receive.try_recv().map_err(error)?.map_err(error);
        let cleanup = candidate.clear_epoch_scratch();
        let owner::Reply::EpochCandidate { .. } = result? else {
            return Err(error("Unexpected epoch preparation reply"));
        };
        cleanup?;
        let store = pop_storage::store::Store::open_vfs("document.pop", &candidate.vfs, false).map_err(error)?;
        let result = (|| {
            let mut hash = Sha256::new();
            let mut size = 0_u64;
            let exported = store.export_pages(|bytes| {
                size += bytes.len() as u64;
                if size > pop_sharing_protocol::SEED_BYTES as u64 {
                    return Err(pop_storage::store::Error::Full);
                }
                hash.update(bytes);
                Ok(())
            });
            if size > pop_sharing_protocol::SEED_BYTES as u64 {
                let failure = js_sys::Error::new("This document has outgrown sharing");
                js_sys::Reflect::set(&failure, &"code".into(), &"outgrown".into())?;
                return Err(failure.into());
            }
            exported.map_err(error)?;
            let seed_hash = hex(&hash.finalize());
            let root = store.document().map_err(error)?.wire_history();
            let resources = store.sharing_manifest().map_err(error)?;
            let attachments = store
                .attachments()
                .map_err(error)?
                .into_iter()
                .map(|item| Resource {
                    key: item.id.clone(),
                    hash: item.id,
                    size: item.bytes,
                    media_type: item.media_type,
                })
                .collect();
            let publish =
                PublishEpoch { expected, seed_hash: seed_hash.clone(), root: root.clone(), resources, attachments };
            publish.validate().map_err(error)?;
            candidate.accepted_epoch = Some(EpochState { seed_hash, root: root.clone(), heads: root, handoff: None });
            serde_json::to_string(
                &serde_json::json!({"sourceVersion": source_version, "size": size, "publish": publish}),
            )
            .map_err(error)
        })();
        let closed = store.close().map_err(error);
        let output = result?;
        closed?;
        Ok(output)
    }

    /// Transport streams immutable SQLite pages to an OPFS upload file, never JSON.
    #[wasm_bindgen(js_name = exportEpoch)]
    pub fn export_epoch(&self, write: &js_sys::Function) -> Result<(), JsValue> {
        if self.accepted_epoch.is_none() || self.driver.is_some() || self.transfer_active.load(Ordering::Acquire) {
            return Err(error("No prepared epoch"));
        }
        let store = pop_storage::store::Store::open_vfs("document.pop", &self.vfs, false).map_err(error)?;
        let result = store
            .export_pages(|bytes| {
                write
                    .call1(&JsValue::NULL, &js_sys::Uint8Array::from(bytes))
                    .map(|_| ())
                    .map_err(|error| pop_storage::store::Error::Failed(format!("Epoch export failed: {error:?}")))
            })
            .map_err(error);
        let closed = store.close().map_err(error);
        result?;
        closed
    }

    #[wasm_bindgen(js_name = epochAttachment)]
    pub fn epoch_attachment(&self, id: &str) -> Result<Vec<u8>, JsValue> {
        if self.accepted_epoch.is_none() || self.driver.is_some() || self.transfer_active.load(Ordering::Acquire) {
            return Err(error("No prepared epoch"));
        }
        let store = pop_storage::store::Store::open_vfs("document.pop", &self.vfs, false).map_err(error)?;
        let result = store.attachment(id).map_err(error);
        let closed = store.close().map_err(error);
        let bytes = result?;
        closed?;
        Ok(bytes)
    }

    #[wasm_bindgen(js_name = validateEpoch)]
    pub fn validate_epoch(&mut self, metadata: &str, epoch: &str) -> Result<(), JsValue> {
        if self.driver.is_some() || self.importing.is_some() || self.transfer_active.load(Ordering::Acquire) {
            return Err(error("Epoch candidate must be closed"));
        }
        self.accepted_epoch = None;
        let info = room_info(metadata)?;
        let epoch: pop_sharing_protocol::lifecycle::EpochState = serde_json::from_str(epoch).map_err(error)?;
        epoch.validate().map_err(error)?;
        if epoch.handoff.is_some()
            || Some(epoch.seed_hash.as_str()) != info.seed_hash.as_deref()
            || self.imported_hash.as_deref() != Some(epoch.seed_hash.as_str())
        {
            return Err(error("Epoch seed checksum or authority mismatch"));
        }
        let store = pop_storage::store::Store::open_vfs("document.pop", &self.vfs, false).map_err(error)?;
        let accepted = (|| {
            if store.app().document_uuid.as_deref() != Some(info.document.as_str())
                || Some(hex(&store.app_digest().map_err(error)?)).as_deref() != info.app.as_deref()
                || !store.document().map_err(error)?.covers_wire_history(&epoch.root).map_err(error)?
            {
                return Err(error("Epoch document, app or root mismatch"));
            }
            Ok(())
        })();
        let closed = store.close().map_err(error);
        accepted?;
        closed?;
        self.accepted_epoch = Some(epoch);
        Ok(())
    }

    #[wasm_bindgen(js_name = openLocal)]
    pub fn open_local(&mut self) -> Result<(), JsValue> {
        self.open()?;
        if self.driver.as_ref().expect("opened").store().share().map_err(error)?.is_some() {
            self.driver = None;
            self.events.lock().expect("events").clear();
            return Err(error("This shared copy needs its invitation"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = documentIdentity)]
    pub fn document_identity(&self) -> Result<String, JsValue> {
        self.driver
            .as_ref()
            .ok_or_else(|| error("Copy is not open"))?
            .store()
            .app()
            .document_uuid
            .clone()
            .ok_or_else(|| error("Missing document identity"))
    }

    /// A catalog reservation can outlive a lost acknowledgment. Only the atomic clean
    /// copy result (new identity and no membership) may be promoted to a usable copy.
    #[wasm_bindgen(js_name = openRecovery)]
    pub fn open_recovery(&mut self, original: &str) -> Result<(), JsValue> {
        if !pop_core::browser_wire::valid_copy(original) || self.fresh || self.driver.is_some() {
            return Err(error("Invalid recovery reservation"));
        }
        let store = pop_storage::store::Store::open_vfs("document.pop", &self.vfs, false).map_err(error)?;
        let complete = store.app().document_uuid.as_deref().is_some_and(|id| id != original)
            && store.share().map_err(error)?.is_none();
        store.close().map_err(error)?;
        if !complete {
            return Err(error("Recovery is incomplete; reopen the original shared copy to retry"));
        }
        self.open()
    }

    #[wasm_bindgen(js_name = currentVersion)]
    pub fn current_version(&mut self) -> Result<String, JsValue> {
        let driver = self.driver.as_mut().ok_or_else(|| error("Copy is not open"))?;
        let (send, receive) = std::sync::mpsc::channel();
        driver.owner().submit(
            owner::Request::State,
            None,
            Box::new(move |reply| {
                let _ = send.send(reply);
            }),
        );
        driver.poll(0, js_sys::Date::now() as u64);
        match receive.try_recv().map_err(error)?.map_err(error)? {
            owner::Reply::State { reading, .. } => Ok(reading.version),
            _ => Err(error("Unexpected owner state reply")),
        }
    }

    #[wasm_bindgen(js_name = confirmedVersion)]
    pub fn confirmed_version(&mut self) -> Result<Option<String>, JsValue> {
        let Some(link) = &self.sync_link else { return Ok(None) };
        let (send, receive) = std::sync::mpsc::channel();
        link.confirmed_version(move |confirmation| {
            let _ = send.send(confirmation.map(|value| value.version));
        });
        self.driver.as_mut().ok_or_else(|| error("Copy is not open"))?.poll(0, js_sys::Date::now() as u64);
        receive.try_recv().map_err(error)
    }

    #[wasm_bindgen(js_name = adoptEpoch)]
    pub fn adopt_epoch(
        &mut self,
        candidate: &BrowserCopy,
        recovery: &BrowserCopy,
        confirmed: &str,
    ) -> Result<bool, JsValue> {
        if candidate.accepted_epoch.is_none()
            || candidate.transfer_active.load(Ordering::Acquire)
            || recovery.transfer_active.load(Ordering::Acquire)
            || candidate.driver.is_some()
            || recovery.driver.is_some()
            || self.vfs == candidate.vfs
            || self.vfs == recovery.vfs
            || candidate.vfs == recovery.vfs
        {
            return Err(error("Invalid epoch or recovery destination"));
        }
        recovery.clear_epoch_scratch()?;
        let driver = self.driver.as_mut().ok_or_else(|| error("Copy is not open"))?;
        let location = |copy: &BrowserCopy| pop_storage::store::EpochLocation::Vfs {
            name: "document.pop".into(),
            vfs: copy.vfs.clone(),
        };
        let (send, receive) = std::sync::mpsc::channel();
        let candidate_active = candidate.transfer_active.clone();
        let recovery_active = recovery.transfer_active.clone();
        candidate_active.store(true, Ordering::Release);
        recovery_active.store(true, Ordering::Release);
        driver.owner().submit(
            owner::Request::AdoptEpoch {
                source: location(candidate),
                expected_version: confirmed.into(),
                recovery_destination: Some(location(recovery)),
            },
            None,
            Box::new(move |reply| {
                candidate_active.store(false, Ordering::Release);
                recovery_active.store(false, Ordering::Release);
                let _ = send.send(reply);
            }),
        );
        driver.poll(0, js_sys::Date::now() as u64);
        let result = receive.try_recv().map_err(error)?.map_err(error);
        // Both connections have left the serial effect by this point. A failed
        // cleanup leaves only private scratch, which the next attempt removes.
        let cleanup = recovery.clear_epoch_scratch();
        match result? {
            owner::Reply::EpochAdopted { recovery_path } => {
                cleanup?;
                Ok(recovery_path.is_some())
            }
            _ => Err(error("Unexpected epoch adoption reply")),
        }
    }

    #[wasm_bindgen(js_name = abortImport)]
    pub fn abort_import(&mut self) -> Result<(), JsValue> {
        if self.driver.is_some() || self.transfer_active.load(Ordering::Acquire) {
            return Err(error("Copy storage is in use"));
        }
        drop(self.importing.take());
        Ok(())
    }

    /// Release sync access handles for an unopened transfer/recovery pool. Persistent
    /// data remains available to a later worker under its own Web Lock.
    #[wasm_bindgen(js_name = releaseStorage)]
    pub fn release_storage(&mut self) -> Result<(), JsValue> {
        if self.driver.is_some() || self.importing.is_some() || self.transfer_active.load(Ordering::Acquire) {
            return Err(error("Copy storage is still in use"));
        }
        // SAFETY: this transfer-only copy has no owner or active import. Every
        // temporary Store and persistence effect has closed its SQLite connection:
        // the effect callback clears transfer_active only after perform returns.
        // Exports are synchronous &self calls, so wasm-bindgen's borrow guard also
        // prevents mutable release during an export callback. These satisfy the
        // OpfsSAHPoolUtil::uninstall contract: no SQLite users or saved VFS pointers.
        // Unregister as well as closing handles so repeated refreshes do not retain
        // a pool's slots and namespace for the lifetime of the worker.
        let Some(pool) = self.pool.take() else { return Ok(()) };
        // SAFETY: the checks and lifecycle argument above exclude every SQLite user
        // and retained VFS pointer before removing this exact registration.
        if let Err(failure) = unsafe { pool.uninstall() } {
            self.pool = Some(pool);
            return Err(error(failure));
        }
        // SAFETY: install_copy allocated this exact utility with Box::leak. No
        // import borrows it, SQLite's registration is gone, and taking the Option
        // makes every later pool access fail rather than retain a dangling reference.
        unsafe { drop(Box::from_raw(std::ptr::from_ref(pool).cast_mut())) };
        Ok(())
    }
}

impl BrowserCopy {
    fn pool(&self) -> Result<&'static OpfsSAHPoolUtil, JsValue> {
        self.pool.ok_or_else(|| error("Copy storage has been released"))
    }

    fn clear_epoch_scratch(&self) -> Result<(), JsValue> {
        if self.driver.is_some() || self.importing.is_some() || self.transfer_active.load(Ordering::Acquire) {
            return Err(error("Epoch scratch is still in use"));
        }
        self.pool()?.remove("epoch-staging.pop-journal").map_err(error)?;
        self.pool()?.remove("epoch-staging.pop").map_err(error)?;
        Ok(())
    }
}
