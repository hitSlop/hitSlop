//! A fresh bounded QuickJS realm, with no host functions exposed to authored code.
use crate::{Error, output_error, program_error};
use pop_contracts::evaluator::{Job, JobKind, OUTPUT, RUNTIME_ABI};
use rquickjs::{CatchResultExt, Function};
/// One prelude per admitted runtime ABI, index `abi - 1`: raising `RUNTIME_ABI` does not
/// compile until the new prelude is added, and a released ABI's prelude stays. A released
/// prelude is frozen (`scripts/build/runner.ts` refuses to regenerate it).
const PRELUDES: [&str; RUNTIME_ABI as usize] = [include_str!("abi/1.generated.js")];
pub(crate) fn prelude(abi: u64) -> Result<&'static str, Error> {
    abi.checked_sub(1)
        .and_then(|index| PRELUDES.get(index as usize).copied())
        .ok_or_else(|| Error::Output("Unsupported command runtime ABI".into()))
}
pub fn evaluate(input: &[u8], interrupted: impl FnMut() -> bool + 'static) -> Result<String, Error> {
    evaluate_job(Job::decode(input).map_err(output_error)?, interrupted)
}

pub(crate) fn evaluate_job(input: Job<'_>, interrupted: impl FnMut() -> bool + 'static) -> Result<String, Error> {
    let prelude = prelude(input.runtime_abi)?;
    let function = match input.mode {
        JobKind::Widget | JobKind::Capture | JobKind::Pdf => {
            if !input.bundle.is_empty() || !input.request.is_empty() {
                return Err(Error::Program("Raster jobs cannot carry JavaScript or an authored request".into()));
            }
            #[cfg(feature = "capture")]
            {
                let raster =
                    input.raster.ok_or_else(|| Error::Output("Raster job requires frames and images".into()))?;
                raster.batch.validate().map_err(program_error)?;
                if serde_json::to_vec(&raster.batch).map_err(|e| Error::Program(e.to_string()))?.len()
                    > pop_contracts::evaluator::MARKUP_BYTES
                {
                    return Err(Error::Program("Capture markup exceeds 16 MiB".into()));
                }
                use base64::Engine;
                // The child's request carries each image as a data URL of an accepted type.
                let images = raster
                    .images
                    .into_iter()
                    .map(|(id, url)| {
                        let (header, encoded) = url
                            .split_once(',')
                            .ok_or_else(|| Error::Program(format!("Invalid image resource {id}")))?;
                        if !crate::IMAGE_HEADERS.iter().any(|(_, accepted)| *accepted == header) {
                            return Err(Error::Program(format!("Unsupported image resource {id}")));
                        }
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(encoded)
                            .map_err(|e| Error::Program(format!("Invalid image resource {id}: {e}")))?;
                        Ok((id, bytes))
                    })
                    .collect::<Result<_, Error>>()?;
                let images = crate::render::raster(raster.batch, images, input.mode)?;
                let reply = pop_contracts::evaluator::RenderReply::Images {
                    ok: pop_contracts::wire::engine::True,
                    images: images
                        .into_iter()
                        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
                        .collect(),
                };
                return serde_json::to_string(&reply).map_err(output_error);
            }
            #[cfg(not(feature = "capture"))]
            return Err(Error::Program("Native capture rendering is unavailable".into()));
        }
        _ if input.raster.is_some() => return Err(Error::Program("JavaScript jobs cannot carry raster images".into())),
        JobKind::Markup | JobKind::WidgetMarkup => {
            #[cfg(feature = "capture")]
            return crate::capture::evaluate_markup(
                &input.bundle,
                &input.request,
                if matches!(input.mode, JobKind::WidgetMarkup) { "__popWidget" } else { "__popCapture" },
                interrupted,
            )
            .and_then(|batch| serde_json::to_string(&batch).map_err(output_error));
            #[cfg(not(feature = "capture"))]
            return Err(Error::Program("Native capture rendering is unavailable".into()));
        }
        JobKind::Command => "__popRun",
        JobKind::Definition => "__popDescribe",
    };
    let realm = crate::realm::create(interrupted)?;
    let strict = |script: &str| format!("(function(){{\"use strict\";\n{script}\n}})();");
    let output = realm.outcome(realm.context.with(|ctx| {
        ctx.eval::<(), _>(strict(prelude)).catch(&ctx).map_err(crate::realm::javascript_error)?;
        ctx.eval::<(), _>(strict(&input.bundle)).catch(&ctx).map_err(crate::realm::javascript_error)?;
        let run: Function = ctx.globals().get(function).catch(&ctx).map_err(crate::realm::javascript_error)?;
        run.call::<_, String>((input.request.as_ref(),)).catch(&ctx).map_err(crate::realm::javascript_error)
    }))?;
    if output.len() > OUTPUT {
        return Err(Error::Output("Command output is too large".into()));
    }
    Ok(output)
}
