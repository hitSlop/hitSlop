uniffi::setup_scaffolding!();

use hitslop_core::Document as Core;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

/// A rejected request leaves the owner usable; an invalidated owner refuses every call
/// until the host reloads saved state into a new owner.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{code}: {message}")]
    Rejected {
        code: String,
        message: String,
        op_index: Option<u32>,
    },
    #[error("{message}")]
    Invalidated { message: String },
}
fn rejected(e: hitslop_core::Error) -> CoreError {
    CoreError::Rejected {
        code: e.code,
        message: e.message,
        op_index: e.op_index.map(|i| i as u32),
    }
}
fn invalidated(message: &str) -> CoreError {
    CoreError::Invalidated {
        message: message.into(),
    }
}

#[derive(uniffi::Record)]
pub struct ApplyResult {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub publication: String,
}
#[derive(uniffi::Record)]
pub struct TextResult {
    pub sequence: u64,
    pub authored: String,
    pub selection_start: u32,
    pub selection_end: u32,
    pub publication: Option<String>,
}

/// The core build this library embeds; app and helper must report the same value.
#[uniffi::export]
pub fn core_build_id() -> String {
    hitslop_core::BUILD_ID.into()
}

#[derive(uniffi::Object)]
pub struct NativeDocument {
    inner: Mutex<Option<Core>>,
}
impl NativeDocument {
    fn construct(
        f: impl FnOnce() -> Result<Core, hitslop_core::Error>,
    ) -> Result<Arc<Self>, CoreError> {
        let core = catch_unwind(AssertUnwindSafe(f))
            .map_err(|_| invalidated("engine_panic: creation failed"))?
            .map_err(rejected)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(core)),
        }))
    }
    fn call<T>(
        &self,
        f: impl FnOnce(&mut Core) -> Result<T, hitslop_core::Error>,
    ) -> Result<T, CoreError> {
        let mut guard = self.inner.lock().map_err(|_| invalidated("owner_poisoned"))?;
        let core = guard
            .as_mut()
            .ok_or_else(|| invalidated("owner_poisoned: reload durable state"))?;
        match catch_unwind(AssertUnwindSafe(|| f(core))) {
            Ok(result) => result.map_err(rejected),
            Err(_) => {
                *guard = None;
                Err(invalidated(
                    "engine_panic: owner invalidated; reload durable state",
                ))
            }
        }
    }
}
#[uniffi::export]
impl NativeDocument {
    #[uniffi::constructor]
    pub fn create(schema_json: String, initial_json: String) -> Result<Arc<Self>, CoreError> {
        Self::construct(|| Core::create(&schema_json, &initial_json))
    }
    #[uniffi::constructor]
    pub fn open(
        schema_json: String,
        checkpoint: Vec<u8>,
        updates: Vec<Vec<u8>>,
    ) -> Result<Arc<Self>, CoreError> {
        Self::construct(|| Core::open(&schema_json, &checkpoint, &updates))
    }
    pub fn sequence(&self) -> Result<u64, CoreError> {
        self.call(|d| Ok(d.sequence()))
    }
    pub fn apply_batch(&self, batch_json: String) -> Result<ApplyResult, CoreError> {
        self.call(|d| {
            let applied = d.apply_batch(&batch_json)?;
            Ok(ApplyResult {
                sequence: applied.sequence,
                ids: applied.ids,
                publication: applied.publication,
            })
        })
    }
    pub fn edit_text(&self, request_json: String) -> Result<TextResult, CoreError> {
        self.call(|d| {
            let edit = d.edit_text(&request_json)?;
            Ok(TextResult {
                sequence: edit.sequence,
                authored: edit.authored,
                selection_start: edit.selection_start as u32,
                selection_end: edit.selection_end as u32,
                publication: edit.publication,
            })
        })
    }
    pub fn snapshot(&self) -> Result<String, CoreError> {
        self.call(|d| d.snapshot())
    }
    pub fn version(&self) -> Result<String, CoreError> {
        self.call(|d| Ok(d.version()))
    }
    pub fn import_updates(&self, bytes: Vec<u8>) -> Result<String, CoreError> {
        self.call(|d| d.import(&bytes))
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, CoreError> {
        self.call(|d| d.checkpoint())
    }
    pub fn export_since(&self, version: String) -> Result<Vec<u8>, CoreError> {
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
        assert!(matches!(result, Err(CoreError::Invalidated { .. })));
        assert!(matches!(owner.snapshot(), Err(CoreError::Invalidated { .. })));
        let restored = NativeDocument::open(schema.into(), saved, vec![]).unwrap();
        assert!(restored.snapshot().unwrap().contains("\"done\":false"));
    }
}
