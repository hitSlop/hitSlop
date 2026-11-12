//! Identical resource constraints, with mode-specific code installed by the caller.
use crate::Error;
use rquickjs::{Context, Runtime};
use std::{cell::Cell, rc::Rc};

pub(crate) struct Realm {
    pub runtime: Runtime,
    pub context: Context,
    interrupted: Rc<Cell<bool>>,
}
impl Realm {
    pub fn outcome<T>(&self, result: Result<T, Error>) -> Result<T, Error> {
        if self.interrupted.get() { Err(Error::Timeout) } else { result }
    }
}
pub(crate) fn create(mut interrupted: impl FnMut() -> bool + 'static) -> Result<Realm, Error> {
    let runtime = Runtime::new().map_err(|e| Error::Spawn(e.to_string()))?;
    runtime.set_memory_limit(256 << 20);
    runtime.set_max_stack_size(512 << 10);
    let triggered = Rc::new(Cell::new(false));
    let flag = triggered.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let stop = interrupted();
        if stop {
            flag.set(true);
        }
        stop
    })));
    let context = Context::full(&runtime).map_err(|e| Error::Spawn(e.to_string()))?;
    Ok(Realm { runtime, context, interrupted: triggered })
}

pub(crate) fn javascript_error(error: rquickjs::CaughtError<'_>) -> Error {
    match error {
        rquickjs::CaughtError::Error(rquickjs::Error::Allocation) => Error::Memory,
        other => Error::Program(other.to_string()),
    }
}
