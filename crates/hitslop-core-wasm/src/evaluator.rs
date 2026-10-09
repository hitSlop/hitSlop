use wasm_bindgen::prelude::*;

/// Invoked only in a disposable worker with a parent watchdog. No JS host function is
/// installed into QuickJS; its only result is the shared evaluator's bounded JSON.
#[wasm_bindgen(js_name = evaluateCommand)]
pub fn evaluate(input: &str) -> Result<String, JsValue> {
    let deadline = js_sys::Date::now() + hitslop_runner::EXECUTION_MS as f64;
    hitslop_runner::evaluate(input.as_bytes(), move || js_sys::Date::now() > deadline)
        .map_err(|error| JsValue::from_str(&error))
}

// QuickJS's wasm platform clock. This is available only to the interpreter's C
// runtime; authored code receives the deterministic ABI clock from its envelope.
#[unsafe(no_mangle)]
pub extern "C" fn __rquickjs_host_now_us() -> f64 {
    js_sys::Date::now() * 1000.0
}
