//! wasm-bindgen adapter for the shared document core.
#![cfg(target_arch = "wasm32")]
use hitslop_core::{AppSpec, Applied, Document as Core, Origin};
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
    hitslop_core::validate(schema_json, initial_json).map_err(error)
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

/// A committed batch, as the page's `apply` reply carries it, with its publication. A text
/// edit (a set carrying `selection`) also has `authored` and its merged selection.
#[wasm_bindgen(getter_with_clone)]
pub struct ApplyResult {
    pub sequence: f64,
    pub ids: Vec<String>,
    pub publication: Option<String>,
    pub authored: Option<String>,
    #[wasm_bindgen(js_name = selectionStart)]
    pub selection_start: Option<u32>,
    #[wasm_bindgen(js_name = selectionEnd)]
    pub selection_end: Option<u32>,
}

fn applied(applied: Applied) -> ApplyResult {
    let selection = applied.text.as_ref().map(|text| text.selection.map(|offset| offset as u32));
    ApplyResult {
        sequence: applied.sequence as f64,
        ids: applied.ids,
        publication: applied.publication,
        authored: applied.text.map(|text| text.authored),
        selection_start: selection.map(|s| s[0]),
        selection_end: selection.map(|s| s[1]),
    }
}

#[wasm_bindgen]
pub struct WasmDocument {
    inner: Core,
}
#[wasm_bindgen]
impl WasmDocument {
    /// A new document of an app: its descriptor, its initial values, and optionally its
    /// template's slug and declared colors.
    pub fn create(schema_json: &str, initial_json: &str, template: Option<String>, theme_json: Option<String>) -> Result<WasmDocument, JsValue> {
        let app = AppSpec::new(schema_json, template.as_deref().unwrap_or(""), theme_json.as_deref().unwrap_or("{}")).map_err(error)?;
        Ok(Self { inner: Core::create(&app, initial_json).map_err(error)? })
    }
    /// A palette change, as the window's theme panel makes it.
    #[wasm_bindgen(js_name = themeSet)]
    pub fn theme_set(&mut self, values_json: &str) -> Result<ApplyResult, JsValue> {
        let batch = format!(r#"{{"intents":[{{"type":"setTheme","values":{values_json}}}]}}"#);
        self.inner.apply_batch(&batch, Origin::Window).map(applied).map_err(error)
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
}
