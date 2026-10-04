//! wasm-bindgen adapter for the shared document core.
use hitslop_core::{Applied, Document as Core, Origin};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = coreBuildId)]
pub fn core_build_id() -> String {
    hitslop_core::BUILD_ID.into()
}

fn error(e: hitslop_core::Error) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"code".into(), &e.code.as_str().into()).expect("plain error object");
    js_sys::Reflect::set(&value, &"message".into(), &e.message.into()).expect("plain error object");
    if let Some(index) = e.op_index {
        js_sys::Reflect::set(&value, &"opIndex".into(), &JsValue::from_f64(index as f64)).expect("plain error object");
    }
    value.into()
}

#[wasm_bindgen]
pub fn validate(schema_json: &str, initial_json: &str) -> Result<(), JsValue> {
    hitslop_core::validate(schema_json, initial_json).map(|_| ()).map_err(error)
}
#[wasm_bindgen(js_name = validateThemeDefaults)]
pub fn validate_theme_defaults(json: &str) -> Result<(), JsValue> {
    hitslop_core::theme::validate_defaults(json).map(|_| ()).map_err(error)
}
/// Validates a manifest window `shape` (JSON, or undefined for the default).
#[wasm_bindgen(js_name = validateWindowShape)]
pub fn validate_window_shape(shape_json: Option<String>, width: f64, height: f64) -> Result<(), JsValue> {
    hitslop_core::shape::validate(shape_json.as_deref(), width, height).map_err(error)
}

/// Mirrors the native `ApplyResult` record.
#[wasm_bindgen(getter_with_clone)]
pub struct ApplyResult {
    pub sequence: f64,
    pub ids: Vec<String>,
    pub publication: Option<String>,
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

fn applied(applied: Applied) -> ApplyResult {
    ApplyResult { sequence: applied.sequence as f64, ids: applied.ids, publication: applied.publication }
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
    /// The owner's current state, as a page opens it.
    pub fn state(&self) -> Result<String, JsValue> {
        self.inner.state().map_err(error)
    }
    /// The full recomputation that `state` is tested against.
    pub fn snapshot(&self) -> Result<String, JsValue> {
        self.inner.snapshot().map_err(error)
    }
    /// A page's batch, part of the person's undo.
    #[wasm_bindgen(js_name = applyBatch)]
    pub fn apply_batch(&mut self, batch_json: &str) -> Result<ApplyResult, JsValue> {
        self.inner.apply_batch(batch_json, Origin::Page).map(applied).map_err(error)
    }
    pub fn undo(&mut self) -> Result<ApplyResult, JsValue> {
        self.inner.undo().map(applied).map_err(error)
    }
    pub fn redo(&mut self) -> Result<ApplyResult, JsValue> {
        self.inner.redo().map(applied).map_err(error)
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
}
