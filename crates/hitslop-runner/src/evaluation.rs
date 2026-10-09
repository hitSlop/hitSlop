//! A fresh bounded QuickJS realm, with no host functions exposed to authored code.
use crate::{INPUT, Mode, OUTPUT, RUNTIME_ABI};
use rquickjs::{CatchResultExt, Context, Function, Runtime};
use serde::{Deserialize, Serialize};
/// The QuickJS heap. It holds the program, the parsed request (up to `INPUT` bytes of
/// JSON) and what the command builds, so it is several times the input limit.
const HEAP: usize = 256 << 20;
/// One prelude per admitted runtime ABI, index `abi - 1`: raising `RUNTIME_ABI` does not
/// compile until the new prelude is added, and a released ABI's prelude stays. A released
/// prelude is frozen (`scripts/build/runner.ts` refuses to regenerate it).
const PRELUDES: [&str; RUNTIME_ABI as usize] = [include_str!("abi/1.generated.js")];
fn prelude(abi: u64) -> Result<&'static str, String> {
    abi.checked_sub(1)
        .and_then(|index| PRELUDES.get(index as usize).copied())
        .ok_or_else(|| "Unsupported command runtime ABI".into())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(rename = "runtimeABI")]
    runtime_abi: u64,
    bundle: String,
    request: String,
    #[serde(default)]
    mode: Mode,
}

pub fn evaluate(input: &[u8], interrupted: impl FnMut() -> bool + 'static) -> Result<String, String> {
    if input.len() > INPUT {
        return Err("Command input is too large".into());
    }
    let input: Input = serde_json::from_slice(input).map_err(|e| e.to_string())?;
    let prelude = prelude(input.runtime_abi)?;
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(HEAP);
    runtime.set_max_stack_size(512 << 10);
    runtime.set_interrupt_handler(Some(Box::new(interrupted)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;
    let strict = |script: &str| format!("(function(){{\"use strict\";\n{script}\n}})();");
    let output = context.with(|ctx| {
        ctx.eval::<(), _>(strict(prelude)).catch(&ctx).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(strict(&input.bundle)).catch(&ctx).map_err(|e| e.to_string())?;
        let function = match input.mode {
            Mode::Command => "__slopRun",
            Mode::Definition => "__slopDescribe",
        };
        let run: Function = ctx.globals().get(function).catch(&ctx).map_err(|e| e.to_string())?;
        run.call::<_, String>((input.request,)).catch(&ctx).map_err(|e| e.to_string())
    })?;
    if output.len() > OUTPUT {
        return Err("Command output is too large".into());
    }
    Ok(output)
}
