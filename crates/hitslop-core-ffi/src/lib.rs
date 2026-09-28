uniffi::setup_scaffolding!();

use hitslop_core::Document as Core;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum BridgeError {
    #[error("{message}")]
    Failure { message: String },
}
fn failure(e: impl ToString) -> BridgeError {
    BridgeError::Failure {
        message: e.to_string(),
    }
}

#[derive(uniffi::Object)]
pub struct NativeDocument {
    inner: Mutex<Option<Core>>,
}
impl NativeDocument {
    fn construct(
        f: impl FnOnce() -> Result<Core, hitslop_core::Error>,
    ) -> Result<Arc<Self>, BridgeError> {
        let core = catch_unwind(AssertUnwindSafe(f))
            .map_err(|_| failure("engine_panic: creation failed"))?
            .map_err(failure)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(core)),
        }))
    }
    fn call<T>(
        &self,
        f: impl FnOnce(&mut Core) -> Result<T, hitslop_core::Error>,
    ) -> Result<T, BridgeError> {
        let mut guard = self.inner.lock().map_err(|_| failure("owner_poisoned"))?;
        let core = guard
            .as_mut()
            .ok_or_else(|| failure("owner_poisoned: reload durable state"))?;
        match catch_unwind(AssertUnwindSafe(|| f(core))) {
            Ok(result) => result.map_err(failure),
            Err(_) => {
                *guard = None;
                Err(failure(
                    "engine_panic: owner invalidated; reload durable state",
                ))
            }
        }
    }
}
#[uniffi::export]
impl NativeDocument {
    #[uniffi::constructor]
    pub fn create(schema_json: String, initial_json: String) -> Result<Arc<Self>, BridgeError> {
        Self::construct(|| Core::create(&schema_json, &initial_json))
    }
    #[uniffi::constructor]
    pub fn open(schema_json: String, checkpoint: Vec<u8>) -> Result<Arc<Self>, BridgeError> {
        Self::construct(|| Core::open(&schema_json, &checkpoint, &[]))
    }
    pub fn snapshot(&self) -> Result<String, BridgeError> {
        self.call(|d| d.snapshot())
    }
    pub fn version(&self) -> Result<String, BridgeError> {
        self.call(|d| Ok(d.version()))
    }
    pub fn command_current(&self, batch_json: String) -> Result<String, BridgeError> {
        self.call(|d| d.command_current(&batch_json))
    }
    pub fn apply(&self, batch_json: String) -> Result<String, BridgeError> {
        self.call(|d| d.apply(&batch_json))
    }
    pub fn import_updates(&self, bytes: Vec<u8>) -> Result<String, BridgeError> {
        self.call(|d| d.import(&bytes))
    }
    pub fn detach_renderer(&self) -> Result<(), BridgeError> {
        self.call(|d| {
            d.detach_renderer();
            Ok(())
        })
    }
    pub fn text(&self, request_json: String) -> Result<String, BridgeError> {
        self.call(|d| d.text(&request_json))
    }
    pub fn release_draft(&self, draft: String) -> Result<(), BridgeError> {
        self.call(|d| d.release_draft(&draft))
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, BridgeError> {
        self.call(|d| d.checkpoint())
    }
    pub fn export_since(&self, version: String) -> Result<Vec<u8>, BridgeError> {
        self.call(|d| d.export_since(&version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Gap: semantic errors do not prove FFI unwind containment. After a panic,
    // every call must refuse until a new owner is explicitly opened from bytes.
    #[test]
    fn panic_invalidates_owner_and_durable_reload_uses_a_new_owner() {
        let schema = r#"{"format":1,"root":{"kind":"object","properties":{"done":{"kind":"boolean"}}}}"#;
        let owner = NativeDocument::create(schema.into(), r#"{"done":false}"#.into()).unwrap();
        let saved = owner.checkpoint().unwrap();
        let result: Result<(), _> = owner.call(|_| panic!("injected unwind at the FFI boundary"));
        assert!(result.unwrap_err().to_string().contains("engine_panic"));
        assert!(owner.snapshot().unwrap_err().to_string().contains("owner_poisoned"));
        let restored = NativeDocument::open(schema.into(), saved).unwrap();
        assert!(restored.snapshot().unwrap().contains("\"done\":false"));
    }
}
