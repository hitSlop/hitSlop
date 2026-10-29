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
    pub fn new(executable: PathBuf, arguments: Vec<String>) -> Result<Self, String> {
        if !executable.is_absolute() || !arguments.is_empty() {
            return Err("In-process capture requires an absolute host binary path and no arguments".into());
        }
        Ok(Self { executable })
    }
    pub fn executable_path(&self) -> &Path {
        &self.executable
    }
    pub fn run(&self, _runtime_abi: u64, _bundle: &str, _request: &str) -> Result<String, String> {
        Err("Authored commands are unavailable in the in-process capture driver".into())
    }
    pub fn markup(&self, runtime_abi: u64, bundle: &str, request: &str, widget: bool) -> Result<Batch, String> {
        let mode = if widget { JobKind::WidgetMarkup } else { JobKind::Markup };
        let output = self.evaluate(Job {
            runtime_abi,
            bundle: Cow::Borrowed(bundle),
            request: Cow::Borrowed(request),
            mode,
            raster: None,
        })?;
        Batch::decode(&output)
    }
    pub fn raster(
        &self,
        runtime_abi: u64,
        batch: Batch,
        images: BTreeMap<String, String>,
        mode: JobKind,
    ) -> Result<String, String> {
        if !matches!(mode, JobKind::Widget | JobKind::Capture) {
            return Err("In-process capture supports PNG artwork and widget images only".into());
        }
        self.evaluate(Job {
            runtime_abi,
            bundle: Cow::Borrowed(""),
            request: Cow::Borrowed(""),
            mode,
            raster: Some(RasterRequest { batch, images }),
        })
    }
    fn evaluate(&self, job: Job<'_>) -> Result<String, String> {
        let deadline = Instant::now() + Duration::from_millis(job.mode.limits().watchdog_ms);
        let input = serde_json::to_vec(&job).map_err(|e| e.to_string())?;
        if input.len() > INPUT {
            return Err("Capture input is too large".into());
        }
        evaluation::evaluate(&input, move || Instant::now() >= deadline)
    }
}
