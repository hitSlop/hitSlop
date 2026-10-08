//! The evaluator process: one bounded request on stdin, a fresh QuickJS realm under the
//! OS sandbox, and one reply on stdout.
use crate::{INPUT, Mode, OUTPUT, RUNTIME_ABI, sandbox};
use rquickjs::{CatchResultExt, Context, Function, Runtime};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::{Duration, Instant};

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

fn evaluate(input: Input) -> Result<String, String> {
    let prelude = prelude(input.runtime_abi)?;
    sandbox::restrict()?;
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(HEAP);
    runtime.set_max_stack_size(512 << 10);
    let deadline = Instant::now() + Duration::from_secs(2);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;
    let strict = |script: &str| format!("(function(){{\"use strict\";\n{script}\n}})();");
    context.with(|ctx| {
        ctx.eval::<(), _>(strict(prelude)).catch(&ctx).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(strict(&input.bundle)).catch(&ctx).map_err(|e| e.to_string())?;
        let function = match input.mode {
            Mode::Command => "__slopRun",
            Mode::Definition => "__slopDescribe",
        };
        let run: Function = ctx.globals().get(function).catch(&ctx).map_err(|e| e.to_string())?;
        run.call::<_, String>((input.request,)).catch(&ctx).map_err(|e| e.to_string())
    })
}

pub fn child() {
    let run = || -> Result<String, String> {
        let mut input = Vec::new();
        std::io::stdin().take(INPUT as u64 + 1).read_to_end(&mut input).map_err(|e| e.to_string())?;
        if input.len() > INPUT {
            return Err("Command input is too large".into());
        }
        evaluate(serde_json::from_slice(&input).map_err(|e| e.to_string())?)
    };
    let output = run().unwrap_or_else(|error| serde_json::json!({"ok":false,"error":error}).to_string());
    if output.len() <= OUTPUT {
        println!("{output}");
    }
}
