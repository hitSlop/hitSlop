//! Mobile hosts retain the evaluator interface and protocol, without process execution.
use super::{Batch, JobKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Evaluator {
    executable: PathBuf,
}
const UNAVAILABLE: &str = "Authored commands and native capture are unavailable on this host";
impl Evaluator {
    pub fn new(_executable: PathBuf, _arguments: Vec<String>) -> Result<Self, String> {
        Err(UNAVAILABLE.into())
    }
    pub fn executable_path(&self) -> &Path {
        &self.executable
    }
    pub fn run(&self, _runtime_abi: u64, _bundle: &str, _request: &str) -> Result<String, String> {
        Err(UNAVAILABLE.into())
    }
    pub fn markup(&self, _runtime_abi: u64, _bundle: &str, _request: &str, _widget: bool) -> Result<Batch, String> {
        Err(UNAVAILABLE.into())
    }
    pub fn raster(
        &self,
        _runtime_abi: u64,
        _batch: Batch,
        _images: BTreeMap<String, String>,
        _mode: JobKind,
    ) -> Result<String, String> {
        Err(UNAVAILABLE.into())
    }
}
