//! iOS capture-only driver. The caller runs it on a background thread.
//! QuickJS has its ordinary heap/stack limits and cooperative deadline. Native raster
//! work has no process watchdog or crash isolation; this is not a sandbox substitute.
use super::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Evaluator {
    executable: PathBuf,
}
impl Evaluator {
    /// The containing binary supplies the renderer/font fingerprint; it is never spawned.
    pub fn new(executable: PathBuf) -> Result<Self, Error> {
        if !executable.is_absolute() {
            return Err(Error::Spawn("In-process capture requires an absolute host binary path".into()));
        }
        Ok(Self { executable })
    }
    pub fn executable_path(&self) -> &Path {
        &self.executable
    }
    pub fn run(&self, _runtime_abi: u64, _bundle: &str, _request: &str) -> Result<String, Error> {
        Err(Error::Sandbox("Authored commands are unavailable in the in-process capture driver".into()))
    }
    pub fn markup(&self, runtime_abi: u64, bundle: &str, request: &str, widget: bool) -> Result<Batch, Error> {
        if bundle.len().saturating_add(request.len()) > pop_contracts::evaluator::INPUT {
            return Err(Error::Output("Capture input is too large".into()));
        }
        let deadline = Instant::now() + Duration::from_millis(pop_contracts::evaluator::EXECUTION_MS);
        evaluation::prelude(runtime_abi)?;
        crate::capture::evaluate_markup(
            bundle,
            request,
            if widget { "__popWidget" } else { "__popCapture" },
            move || Instant::now() >= deadline,
        )
    }

    pub fn raster(
        &self,
        runtime_abi: u64,
        batch: Batch,
        images: BTreeMap<String, Vec<u8>>,
        mode: JobKind,
    ) -> Result<Vec<Vec<u8>>, Error> {
        evaluation::prelude(runtime_abi)?;
        if !matches!(mode, JobKind::Widget | JobKind::Capture) {
            return Err(Error::Output("In-process capture supports PNG artwork and widget images only".into()));
        }
        batch.validate().map_err(program_error)?;
        crate::render::raster(batch, images, mode)
    }
}
