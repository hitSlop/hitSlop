//! A fresh bounded QuickJS realm, with no host functions exposed to authored code.
use crate::{Job, JobKind, OUTPUT, RUNTIME_ABI};
use rquickjs::{CatchResultExt, Function};
/// One prelude per admitted runtime ABI, index `abi - 1`: raising `RUNTIME_ABI` does not
/// compile until the new prelude is added, and a released ABI's prelude stays. A released
/// prelude is frozen (`scripts/build/runner.ts` refuses to regenerate it).
const PRELUDES: [&str; RUNTIME_ABI as usize] = [include_str!("abi/1.generated.js")];
fn prelude(abi: u64) -> Result<&'static str, String> {
    abi.checked_sub(1)
        .and_then(|index| PRELUDES.get(index as usize).copied())
        .ok_or_else(|| "Unsupported command runtime ABI".into())
}
pub fn evaluate(input: &[u8], interrupted: impl FnMut() -> bool + 'static) -> Result<String, String> {
    evaluate_job(Job::decode(input)?, interrupted)
}

pub(crate) fn evaluate_job(input: Job<'_>, interrupted: impl FnMut() -> bool + 'static) -> Result<String, String> {
    let prelude = prelude(input.runtime_abi)?;
    if matches!(input.mode, JobKind::Widget) {
        #[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
        return crate::capture::evaluate(&input.bundle, &input.request, interrupted);
        #[cfg(not(all(feature = "capture", not(target_arch = "wasm32"))))]
        return Err("Native widget rendering is unavailable".into());
    }
    if matches!(input.mode, JobKind::Capture | JobKind::Pdf) {
        #[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
        return crate::capture::evaluate_view(
            &input.bundle,
            &input.request,
            matches!(input.mode, JobKind::Pdf),
            interrupted,
        );
        #[cfg(not(all(feature = "capture", not(target_arch = "wasm32"))))]
        return Err("Native capture rendering is unavailable".into());
    }
    let (_runtime, context) = crate::realm::create(interrupted)?;
    let strict = |script: &str| format!("(function(){{\"use strict\";\n{script}\n}})();");
    let output = context.with(|ctx| {
        ctx.eval::<(), _>(strict(prelude)).catch(&ctx).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(strict(&input.bundle)).catch(&ctx).map_err(|e| e.to_string())?;
        let function = match input.mode {
            JobKind::Command => "__slopRun",
            JobKind::Definition => "__slopDescribe",
            JobKind::Widget | JobKind::Capture | JobKind::Pdf => unreachable!("handled before the command realm"),
        };
        let run: Function = ctx.globals().get(function).catch(&ctx).map_err(|e| e.to_string())?;
        run.call::<_, String>((input.request.as_ref(),)).catch(&ctx).map_err(|e| e.to_string())
    })?;
    if output.len() > OUTPUT {
        return Err("Command output is too large".into());
    }
    Ok(output)
}
