//! Saved-view markup evaluation in the restricted QuickJS realm.
use crate::{Error, program_error};
use pop_contracts::evaluator::Batch;
use rquickjs::{CatchResultExt, Function};

pub fn evaluate_markup(
    bundle: &str,
    request: &str,
    entry: &str,
    interrupted: impl FnMut() -> bool + 'static,
) -> std::result::Result<Batch, Error> {
    let realm = crate::realm::create(interrupted)?;
    let json = realm.outcome(realm.context.with(|ctx| {
        ctx.eval::<(), _>(bundle.as_bytes()).catch(&ctx).map_err(crate::realm::javascript_error)?;
        let render: Function = ctx.globals().get(entry).catch(&ctx).map_err(crate::realm::javascript_error)?;
        render.call::<_, String>((request,)).catch(&ctx).map_err(crate::realm::javascript_error)
    }))?;
    // SSR is finished. Release its heap before returning markup to the owner.
    drop(realm.context);
    drop(realm.runtime);
    Batch::decode(&json).map_err(program_error)
}
