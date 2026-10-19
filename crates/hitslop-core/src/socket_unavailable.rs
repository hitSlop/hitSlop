//! The mobile owner has no desktop discovery or socket listener.
use crate::{
    Code,
    command::{ExportHandler, Result},
    owner::{Failure, Owner},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
fn unavailable() -> Failure {
    Failure::rejected(Code::EngineError, "Desktop socket routing is unavailable on this host")
}
pub fn call(_path: &Path, _input: &str) -> Result<String> {
    Err(unavailable())
}
pub struct Server {
    path: PathBuf,
}
impl Server {
    pub fn start(_owner: Arc<Owner>, _exporter: Arc<dyn ExportHandler>) -> Result<Self> {
        Err(unavailable())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn publish(&self) -> Result<()> {
        Err(unavailable())
    }
    pub fn withdraw(&self) {}
    pub fn stop(&self) {}
}
