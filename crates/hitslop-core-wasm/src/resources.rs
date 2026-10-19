use super::*;
use hitslop_sharing_protocol::resources::{self as resource_types, Manifest, Resource};

#[wasm_bindgen]
impl BrowserCopy {
    #[wasm_bindgen(js_name = verifyResources)]
    pub fn verify_resources(&self, json: &str) -> Result<String, JsValue> {
        let manifest: Manifest = serde_json::from_str(json).map_err(error)?;
        resource_types::validate(&manifest.resources, false).map_err(error)?;
        let store = self.driver.as_ref().ok_or_else(|| error("Copy missing"))?.store();
        let app = store.app_digest().map_err(error)?.iter().map(|b| format!("{b:02x}")).collect::<String>();
        if manifest.app != app || !manifest.resources.iter().any(|i| i.key == "ui.js") {
            return Err(error("Resource app identity mismatch"));
        }
        for item in &manifest.resources {
            let route = hitslop_core::file::ResourceRoute::App;
            let info = store
                .resource_info(route, &item.key)
                .map_err(error)?
                .ok_or_else(|| error("Resource missing from accepted app"))?;
            if info.size != item.size || info.media_type != item.media_type {
                return Err(error("Resource metadata mismatch"));
            }
            let mut hash = Sha256::new();
            let mut at = 0;
            while at < info.size {
                let bytes = store
                    .resource_range(
                        route,
                        &item.key,
                        at,
                        (info.size - at).min(hitslop_core::browser_wire::TRANSFER_BYTES as u64),
                    )
                    .map_err(error)?
                    .ok_or_else(|| error("Missing resource bytes"))?;
                if bytes.is_empty() {
                    return Err(error("Incomplete resource"));
                }
                at += bytes.len() as u64;
                hash.update(bytes);
            }
            if hex(&hash.finalize()) != item.hash {
                return Err(error("Resource hash mismatch"));
            }
        }
        Ok(app)
    }
    #[wasm_bindgen(js_name = attachmentInventory)]
    pub fn attachment_inventory(&self) -> Result<String, JsValue> {
        let store = self.driver.as_ref().ok_or_else(|| error("Copy missing"))?.store();
        let items: Vec<_> = store
            .attachments()
            .map_err(error)?
            .into_iter()
            .map(|i| Resource { key: i.id.clone(), hash: i.id, size: i.bytes, media_type: i.media_type })
            .collect();
        serde_json::to_string(&items).map_err(error)
    }
    #[wasm_bindgen(js_name = missingAttachments)]
    pub fn missing_attachments(&self, json: &str) -> Result<String, JsValue> {
        let mut items: Vec<Resource> = serde_json::from_str(json).map_err(error)?;
        resource_types::validate(&items, true).map_err(error)?;
        let store = self.driver.as_ref().ok_or_else(|| error("Copy missing"))?.store();
        let have: std::collections::HashSet<_> =
            store.attachments().map_err(error)?.into_iter().map(|i| i.id).collect();
        items.retain(|i| !have.contains(&i.hash));
        serde_json::to_string(&items).map_err(error)
    }
    #[wasm_bindgen(js_name = installAttachment)]
    pub fn install_attachment(
        &mut self,
        json: &str,
        bytes: &[u8],
        monotonic_ms: f64,
        unix_ms: f64,
    ) -> Result<(), JsValue> {
        let item: Resource = serde_json::from_str(json).map_err(error)?;
        resource_types::validate(std::slice::from_ref(&item), true).map_err(error)?;
        let accepted = hitslop_core::store::Attachment::from_bytes(bytes).map_err(error)?;
        if accepted.id != item.hash || accepted.bytes != item.size || accepted.media_type != item.media_type {
            return Err(error("Attachment checksum, size or type mismatch"));
        }
        let reply = Arc::new(Mutex::new(None));
        let output = reply.clone();
        self.driver.as_ref().ok_or_else(|| error("Copy missing"))?.owner().submit(
            owner::Request::PutAttachment { bytes: bytes.to_vec() },
            None,
            Box::new(move |r| *output.lock().unwrap() = Some(r)),
        );
        self.poll(monotonic_ms, unix_ms)?;
        let owner::Reply::Attachment { item: stored } =
            reply.lock().unwrap().take().ok_or_else(|| error("Attachment pending"))?.map_err(error)?
        else {
            return Err(error("Attachment not stored"));
        };
        if stored.id != item.hash {
            return Err(error("Attachment identity changed"));
        }
        Ok(())
    }
}
#[wasm_bindgen(js_name = frameOperation)]
pub fn frame_operation(bytes: &[u8]) -> Result<String, JsValue> {
    let frame = hitslop_sharing_protocol::envelope::decode(bytes).map_err(error)?;
    Ok(match frame {
        hitslop_core::sync::Frame::Error { message, .. } if message == "Sharing ended" => "ended",
        hitslop_core::sync::Frame::Push { .. } => "upload",
        hitslop_core::sync::Frame::Records(_) => "download",
        _ => "control",
    }
    .into())
}
