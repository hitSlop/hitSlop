use hitslop_core::{
    browser_wire::{BrowserEvent as Event, BrowserRequest, BrowserSaveStatus as Status, ResourceRoute},
    owner, page_wire,
};
use rsqlite_vfs::transfer::{DbImport, DbTransfer};
use sqlite_wasm_vfs::sahpool::{OpfsSAHImportTarget, OpfsSAHPoolCfgBuilder, OpfsSAHPoolUtil, install};
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;

fn error(value: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&value.to_string())
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
                window: driver.owner().app().app.page_window(),
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
            BrowserRequest::Evaluated { output, error } => self.evaluated(output, error)?,
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
                    page_wire::HostAction::WindowResize { width, height } => {
                        page_wire::HostReply::WindowResize { width: width as f64, height: height as f64 }
                    }
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
        driver.poll(monotonic_ms as u64, unix_ms as u64);
        if let Some(input) = driver.take_evaluation() {
            push(&self.events, Event::Evaluate { input });
        }
        // Envelope construction can itself enqueue a refused evaluation. Drive that
        // completion before sleeping, so it cannot strand the caller.
        let next = driver.poll(monotonic_ms as u64, unix_ms as u64);
        Ok(next.map_or(-1.0, |n| n as f64))
    }
    pub fn events(&self) -> String {
        serde_json::to_string(&std::mem::take(&mut *self.events.lock().expect("worker events")))
            .expect("browser events")
    }
    pub fn evaluated(&mut self, output: Option<String>, failure: Option<String>) -> Result<(), JsValue> {
        let result = match (output, failure) {
            (Some(output), None) => Ok(output),
            (_, failure) => Err(failure.unwrap_or_else(|| "Evaluator returned no output".into())),
        };
        self.driver.as_mut().ok_or_else(|| error("Copy is not open"))?.evaluated(result).map_err(error)
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
fn route_to_file(attachment: bool) -> hitslop_core::file::ResourceRoute {
    if attachment { hitslop_core::file::ResourceRoute::Attachment } else { hitslop_core::file::ResourceRoute::App }
}

/// Validate asynchronous browser operations before JavaScript starts them.
#[wasm_bindgen(js_name = decodeBrowserRequest)]
pub fn decode_browser_request(json: &str) -> Result<String, JsValue> {
    serde_json::to_string(&BrowserRequest::decode(json).map_err(error)?).map_err(error)
}
