//! Bounded command evaluation. Native processes run a QuickJS realm and ABI
//! prelude; only the owner applies returned intents.
use pop_contracts::evaluator::{Batch, JobKind};
#[cfg(any(feature = "child", feature = "capture"))]
mod evaluation;
#[cfg(feature = "in-process")]
mod in_process;
#[cfg(not(target_os = "ios"))]
mod native;
#[cfg(any(feature = "child", feature = "capture"))]
mod realm;
#[cfg(any(feature = "child", feature = "capture"))]
pub use evaluation::evaluate;
#[cfg(feature = "child")]
mod child;
#[cfg(feature = "child")]
mod sandbox;
#[cfg(feature = "child")]
pub use child::child;
#[cfg(feature = "capture")]
mod capture;
#[cfg(feature = "capture")]
mod render;

/// Explicit host execution policy. Enabling another Cargo feature never changes a
/// configured evaluator's isolation or gives a capture-only host authored commands.
#[derive(Clone, Debug)]
pub struct Evaluator {
    provider: Provider,
}
#[derive(Clone, Debug)]
enum Provider {
    #[cfg(not(target_os = "ios"))]
    Process(native::Evaluator),
    #[cfg(feature = "in-process")]
    InProcessCapture(in_process::Evaluator),
}
impl Evaluator {
    /// Start a fresh restricted child process for each command or capture job.
    #[cfg(not(target_os = "ios"))]
    pub fn process(executable: std::path::PathBuf, arguments: Vec<String>) -> Result<Self, String> {
        native::Evaluator::new(executable, arguments).map(|provider| Self { provider: Provider::Process(provider) })
    }
    /// Run capture on the caller's background thread, without process isolation.
    /// Authored commands remain unavailable even when process support is also built.
    #[cfg(feature = "in-process")]
    pub fn in_process_capture(executable: std::path::PathBuf) -> Result<Self, String> {
        in_process::Evaluator::new(executable).map(|provider| Self { provider: Provider::InProcessCapture(provider) })
    }
    /// Whether this host explicitly grants authored command execution.
    pub fn supports_commands(&self) -> bool {
        match self.provider {
            #[cfg(not(target_os = "ios"))]
            Provider::Process(_) => true,
            #[cfg(feature = "in-process")]
            Provider::InProcessCapture(_) => false,
        }
    }

    /// Host binary used for renderer and font freshness checks.
    pub fn executable_path(&self) -> &std::path::Path {
        match self.provider {
            #[cfg(not(target_os = "ios"))]
            Provider::Process(ref provider) => provider.executable_path(),
            #[cfg(feature = "in-process")]
            Provider::InProcessCapture(ref provider) => provider.executable_path(),
        }
    }
    /// Evaluate a command using the explicitly configured execution policy.
    pub fn run(&self, runtime_abi: u64, bundle: &str, request: &str) -> Result<String, String> {
        match self.provider {
            #[cfg(not(target_os = "ios"))]
            Provider::Process(ref provider) => provider.run(runtime_abi, bundle, request),
            #[cfg(feature = "in-process")]
            Provider::InProcessCapture(ref provider) => provider.run(runtime_abi, bundle, request),
        }
    }
    /// Collect capture markup using the configured provider.
    pub fn markup(&self, runtime_abi: u64, bundle: &str, request: &str, widget: bool) -> Result<Batch, String> {
        match self.provider {
            #[cfg(not(target_os = "ios"))]
            Provider::Process(ref provider) => provider.markup(runtime_abi, bundle, request, widget),
            #[cfg(feature = "in-process")]
            Provider::InProcessCapture(ref provider) => provider.markup(runtime_abi, bundle, request, widget),
        }
    }
    /// Rasterize markup using the configured provider's supported output kinds.
    pub fn raster(
        &self,
        runtime_abi: u64,
        batch: Batch,
        images: std::collections::BTreeMap<String, String>,
        mode: JobKind,
    ) -> Result<String, String> {
        match self.provider {
            #[cfg(not(target_os = "ios"))]
            Provider::Process(ref provider) => provider.raster(runtime_abi, batch, images, mode),
            #[cfg(feature = "in-process")]
            Provider::InProcessCapture(ref provider) => provider.raster(runtime_abi, batch, images, mode),
        }
    }
}
