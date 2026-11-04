//! Bounded command evaluation. Native processes and disposable browser workers run
//! the same QuickJS realm and ABI prelude; only the owner applies returned intents.
pub use pop_contracts::evaluator::{
    Batch, CAPTURE_OUTPUT, EXECUTION_MS, Font, Frame, IMAGE_BYTES, INPUT, Job, JobKind, JobLimits, MARKUP_BYTES,
    OUTPUT, PDF_BYTES, PDF_CPU_SECONDS, PDF_OUTPUT, PDF_WATCHDOG_MS, RUNTIME_ABI, RasterRequest, RenderReply,
    WATCHDOG_MS, WIDGET_OUTPUT, command_input,
};
#[cfg(feature = "evaluation")]
mod evaluation;
#[cfg(all(not(target_arch = "wasm32"), feature = "capture-in-process"))]
mod in_process;
#[cfg(all(not(target_arch = "wasm32"), feature = "process"))]
mod native;
#[cfg(feature = "evaluation")]
mod realm;
#[cfg(all(not(target_arch = "wasm32"), any(not(feature = "process"), not(feature = "capture-in-process"))))]
mod unavailable;
#[cfg(feature = "evaluation")]
pub use evaluation::evaluate;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
mod child;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
mod sandbox;
#[cfg(all(feature = "child", not(target_arch = "wasm32")))]
pub use child::child;
#[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
mod capture;

/// Explicit host execution policy. Enabling another Cargo feature never changes a
/// configured evaluator's isolation or gives a capture-only host authored commands.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct Evaluator {
    provider: Provider,
}
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
enum Provider {
    #[cfg(feature = "process")]
    Process(native::Evaluator),
    #[cfg(feature = "capture-in-process")]
    InProcessCapture(in_process::Evaluator),
    #[cfg(any(not(feature = "process"), not(feature = "capture-in-process")))]
    Unavailable(unavailable::Evaluator),
}
#[cfg(not(target_arch = "wasm32"))]
impl Evaluator {
    /// Start a fresh restricted child process for each command or capture job.
    pub fn process(executable: std::path::PathBuf, arguments: Vec<String>) -> Result<Self, String> {
        #[cfg(feature = "process")]
        {
            native::Evaluator::new(executable, arguments).map(|provider| Self { provider: Provider::Process(provider) })
        }
        #[cfg(not(feature = "process"))]
        {
            unavailable::Evaluator::new(executable, arguments, "process")
                .map(|provider| Self { provider: Provider::Unavailable(provider) })
        }
    }
    /// Run capture on the caller's background thread, without process isolation.
    /// Authored commands remain unavailable even when process support is also built.
    pub fn in_process_capture(executable: std::path::PathBuf) -> Result<Self, String> {
        #[cfg(feature = "capture-in-process")]
        {
            in_process::Evaluator::new(executable)
                .map(|provider| Self { provider: Provider::InProcessCapture(provider) })
        }
        #[cfg(not(feature = "capture-in-process"))]
        {
            unavailable::Evaluator::new(executable, vec![], "in-process capture")
                .map(|provider| Self { provider: Provider::Unavailable(provider) })
        }
    }
    /// Host binary used for renderer and font freshness checks.
    pub fn executable_path(&self) -> &std::path::Path {
        match &self.provider {
            #[cfg(feature = "process")]
            Provider::Process(provider) => provider.executable_path(),
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(provider) => provider.executable_path(),
            #[cfg(any(not(feature = "process"), not(feature = "capture-in-process")))]
            Provider::Unavailable(provider) => provider.executable_path(),
        }
    }
    /// Evaluate a command using the explicitly configured execution policy.
    pub fn run(&self, runtime_abi: u64, bundle: &str, request: &str) -> Result<String, String> {
        match &self.provider {
            #[cfg(feature = "process")]
            Provider::Process(provider) => provider.run(runtime_abi, bundle, request),
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(provider) => provider.run(runtime_abi, bundle, request),
            #[cfg(any(not(feature = "process"), not(feature = "capture-in-process")))]
            Provider::Unavailable(provider) => provider.run(runtime_abi, bundle, request),
        }
    }
    /// Collect capture markup using the configured provider.
    pub fn markup(&self, runtime_abi: u64, bundle: &str, request: &str, widget: bool) -> Result<Batch, String> {
        match &self.provider {
            #[cfg(feature = "process")]
            Provider::Process(provider) => provider.markup(runtime_abi, bundle, request, widget),
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(provider) => provider.markup(runtime_abi, bundle, request, widget),
            #[cfg(any(not(feature = "process"), not(feature = "capture-in-process")))]
            Provider::Unavailable(provider) => provider.markup(runtime_abi, bundle, request, widget),
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
        match &self.provider {
            #[cfg(feature = "process")]
            Provider::Process(provider) => provider.raster(runtime_abi, batch, images, mode),
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(provider) => provider.raster(runtime_abi, batch, images, mode),
            #[cfg(any(not(feature = "process"), not(feature = "capture-in-process")))]
            Provider::Unavailable(provider) => provider.raster(runtime_abi, batch, images, mode),
        }
    }
}
