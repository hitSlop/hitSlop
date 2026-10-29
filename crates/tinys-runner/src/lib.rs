//! Bounded command evaluation. Native processes and disposable browser workers run
//! the same QuickJS realm and ABI prelude; only the owner applies returned intents.
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
mod protocol;
pub use protocol::{Batch, Font, Frame, Job, JobKind, JobLimits, RasterRequest, RenderReply};
#[cfg(all(not(target_arch = "wasm32"), feature = "process"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "process"))]
pub use native::Evaluator;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "process"), feature = "capture-in-process"))]
mod in_process;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "process"), feature = "capture-in-process"))]
pub use in_process::Evaluator;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "process"), not(feature = "capture-in-process")))]
mod unavailable;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "process"), not(feature = "capture-in-process")))]
pub use unavailable::Evaluator;
#[cfg(feature = "evaluation")]
mod evaluation;
#[cfg(feature = "evaluation")]
mod realm;
#[cfg(feature = "evaluation")]
pub use evaluation::evaluate;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
mod child;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
mod sandbox;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
pub use child::child;
#[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
mod capture;
pub const MARKUP_BYTES: usize = 16 << 20;
pub const IMAGE_BYTES: usize = 24 << 20;
pub const WIDGET_OUTPUT: usize = 8 << 20;
pub const CAPTURE_OUTPUT: usize = 36 << 20;
pub const PDF_OUTPUT: usize = 36 << 20;
pub const PDF_BYTES: usize = 24 << 20;
pub const PDF_WATCHDOG_MS: u64 = 10_000;
pub const PDF_CPU_SECONDS: u64 = 8;
pub const INPUT: usize = 64 << 20;
pub const OUTPUT: usize = 4 << 20;
pub const WATCHDOG_MS: u64 = 3000;
pub const EXECUTION_MS: u64 = 2000;
pub const RUNTIME_ABI: u64 = 1;
/// The bounded envelope used by both native and browser command drivers.
pub fn command_input(runtime_abi: u64, bundle: &str, request: &str) -> Result<String, String> {
    let input = serde_json::to_string(&Job {
        runtime_abi,
        bundle: Cow::Borrowed(bundle),
        request: Cow::Borrowed(request),
        mode: JobKind::Command,
        raster: None,
    })
    .map_err(|e| e.to_string())?;
    if input.len() > INPUT {
        return Err("Command input is too large".into());
    }
    Ok(input)
}
