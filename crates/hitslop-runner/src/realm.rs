//! Identical resource constraints, with mode-specific code installed by the caller.
use rquickjs::{Context, Runtime};

pub(crate) fn create(interrupted: impl FnMut() -> bool + 'static) -> Result<(Runtime, Context), String> {
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(256 << 20);
    runtime.set_max_stack_size(512 << 10);
    runtime.set_interrupt_handler(Some(Box::new(interrupted)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;
    Ok((runtime, context))
}
