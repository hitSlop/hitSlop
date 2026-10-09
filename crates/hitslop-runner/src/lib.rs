//! Bounded command evaluation. Native processes and disposable browser workers run
//! the same QuickJS realm and ABI prelude; only the owner applies returned intents.
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Evaluator;
#[cfg(feature = "child")]
mod evaluation;
#[cfg(feature = "child")]
pub use evaluation::evaluate;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
mod child;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
mod sandbox;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
pub use child::child;
pub const INPUT: usize = 64 << 20;
pub const OUTPUT: usize = 4 << 20;
pub const WATCHDOG_MS: u64 = 3000;
pub const EXECUTION_MS: u64 = 2000;
pub const RUNTIME_ABI: u64 = 1;
/// What the parent sends: `Input`, borrowed, so a large program is not copied per run.
#[derive(Serialize)]
struct Sent<'a> {
    #[serde(rename = "runtimeABI")]
    runtime_abi: u64,
    bundle: &'a str,
    request: &'a str,
    mode: Mode,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Mode {
    #[default]
    Command,
    Definition,
}

/// The bounded envelope used by both native and browser command drivers.
pub fn command_input(runtime_abi: u64, bundle: &str, request: &str) -> Result<String, String> {
    let input = serde_json::to_string(&Sent { runtime_abi, bundle, request, mode: Mode::Command })
        .map_err(|e| e.to_string())?;
    if input.len() > INPUT {
        return Err("Command input is too large".into());
    }
    Ok(input)
}
