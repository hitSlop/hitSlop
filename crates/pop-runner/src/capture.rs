//! Saved-view markup evaluation in the restricted QuickJS realm.
use crate::Batch;
use rquickjs::{CatchResultExt, Function};

pub fn evaluate_markup(
    bundle: &str,
    request: &str,
    entry: &str,
    interrupted: impl FnMut() -> bool + 'static,
) -> std::result::Result<String, String> {
    let (runtime, context) = crate::realm::create(interrupted)?;
    let json = context.with(|ctx| {
        ctx.eval::<(), _>(bundle.as_bytes()).catch(&ctx).map_err(|e| e.to_string())?;
        let render: Function = ctx.globals().get(entry).catch(&ctx).map_err(|e| e.to_string())?;
        render.call::<_, String>((request,)).catch(&ctx).map_err(|e| e.to_string())
    })?;
    // SSR is finished. Release its heap before returning markup to the owner.
    drop(context);
    drop(runtime);
    Batch::decode(&json)?;
    Ok(json)
}
