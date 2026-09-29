//! wasm-bindgen adapter for the shared document core.
use hitslop_core::Document as Core;
use wasm_bindgen::prelude::*;

fn error(e: hitslop_core::Error) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Mirrors the native `TextResult` record.
#[wasm_bindgen(getter_with_clone)]
pub struct TextResult {
    pub sequence: f64,
    pub authored: String,
    #[wasm_bindgen(js_name = selectionStart)]
    pub selection_start: u32,
    #[wasm_bindgen(js_name = selectionEnd)]
    pub selection_end: u32,
    pub publication: Option<String>,
}

#[wasm_bindgen(js_name = coreBuildId)]
pub fn core_build_id() -> String {
    hitslop_core::BUILD_ID.into()
}

#[wasm_bindgen]
pub struct WasmDocument {
    inner: Core,
}
#[wasm_bindgen]
impl WasmDocument {
    pub fn create(schema_json: &str, initial_json: &str) -> Result<WasmDocument, JsValue> {
        Ok(Self {
            inner: Core::create(schema_json, initial_json).map_err(error)?,
        })
    }
    pub fn open(schema_json: &str, checkpoint: &[u8]) -> Result<WasmDocument, JsValue> {
        Ok(Self {
            inner: Core::open(schema_json, checkpoint, &[]).map_err(error)?,
        })
    }
    pub fn snapshot(&self) -> Result<String, JsValue> {
        self.inner.snapshot().map_err(error)
    }
    pub fn version(&self) -> String {
        self.inner.version()
    }
    pub fn apply(&mut self, batch_json: &str) -> Result<String, JsValue> {
        self.inner.apply(batch_json).map_err(error)
    }
    pub fn import_updates(&mut self, bytes: &[u8]) -> Result<String, JsValue> {
        self.inner.import(bytes).map_err(error)
    }
    pub fn sequence(&self) -> f64 {
        self.inner.sequence() as f64
    }
    #[wasm_bindgen(js_name = editText)]
    pub fn edit_text(&mut self, request_json: &str) -> Result<TextResult, JsValue> {
        let edit = self.inner.edit_text(request_json).map_err(error)?;
        Ok(TextResult {
            sequence: edit.sequence as f64,
            authored: edit.authored,
            selection_start: edit.selection_start as u32,
            selection_end: edit.selection_end as u32,
            publication: edit.publication,
        })
    }
    pub fn text(&mut self, request_json: &str) -> Result<String, JsValue> {
        self.inner.text(request_json).map_err(error)
    }
    pub fn release_draft(&mut self, draft: &str) -> Result<(), JsValue> {
        self.inner.release_draft(draft).map_err(error)
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, JsValue> {
        self.inner.checkpoint().map_err(error)
    }
    pub fn export_since(&self, version: &str) -> Result<Vec<u8>, JsValue> {
        self.inner.export_since(version).map_err(error)
    }
}
