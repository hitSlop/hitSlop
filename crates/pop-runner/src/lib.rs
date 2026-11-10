//! Bounded command evaluation. Native processes run a QuickJS realm and ABI
//! prelude; only the owner applies returned intents.
pub use pop_contracts::evaluator::{
    Batch, CAPTURE_OUTPUT, EXECUTION_MS, Font, Frame, IMAGE_BYTES, INPUT, Job, JobKind, JobLimits, MARKUP_BYTES,
    OUTPUT, PDF_BYTES, PDF_CPU_SECONDS, PDF_OUTPUT, PDF_WATCHDOG_MS, RUNTIME_ABI, RasterRequest, RenderReply,
    WATCHDOG_MS, WIDGET_OUTPUT, command_input,
};
#[cfg(feature = "evaluation")]
mod evaluation;
#[cfg(feature = "capture-in-process")]
mod in_process;
#[cfg(feature = "process")]
mod native;
#[cfg(feature = "evaluation")]
mod realm;
#[cfg(feature = "evaluation")]
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
    #[cfg(feature = "process")]
    Process(native::Evaluator),
    #[cfg(feature = "capture-in-process")]
    InProcessCapture(in_process::Evaluator),
}
#[cfg_attr(
    not(any(feature = "process", feature = "capture-in-process")),
    allow(unused_variables, reason = "a build without a provider cannot construct an Evaluator, so these never run")
)]
impl Evaluator {
    /// Start a fresh restricted child process for each command or capture job.
    pub fn process(executable: std::path::PathBuf, arguments: Vec<String>) -> Result<Self, String> {
        #[cfg(feature = "process")]
        {
            native::Evaluator::new(executable, arguments).map(|provider| Self { provider: Provider::Process(provider) })
        }
        #[cfg(not(feature = "process"))]
        {
            let _ = (executable, arguments);
            Err("The process provider is unavailable in this build".into())
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
            let _ = executable;
            Err("The in-process capture provider is unavailable in this build".into())
        }
    }
    /// Whether this host explicitly grants authored command execution.
    pub fn supports_commands(&self) -> bool {
        match self.provider {
            #[cfg(feature = "process")]
            Provider::Process(_) => true,
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(_) => false,
        }
    }

    /// Host binary used for renderer and font freshness checks.
    pub fn executable_path(&self) -> &std::path::Path {
        match self.provider {
            #[cfg(feature = "process")]
            Provider::Process(ref provider) => provider.executable_path(),
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(ref provider) => provider.executable_path(),
        }
    }
    /// Evaluate a command using the explicitly configured execution policy.
    pub fn run(&self, runtime_abi: u64, bundle: &str, request: &str) -> Result<String, String> {
        match self.provider {
            #[cfg(feature = "process")]
            Provider::Process(ref provider) => provider.run(runtime_abi, bundle, request),
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(ref provider) => provider.run(runtime_abi, bundle, request),
        }
    }
    /// Collect capture markup using the configured provider.
    pub fn markup(&self, runtime_abi: u64, bundle: &str, request: &str, widget: bool) -> Result<Batch, String> {
        match self.provider {
            #[cfg(feature = "process")]
            Provider::Process(ref provider) => provider.markup(runtime_abi, bundle, request, widget),
            #[cfg(feature = "capture-in-process")]
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
            #[cfg(feature = "process")]
            Provider::Process(ref provider) => provider.raster(runtime_abi, batch, images, mode),
            #[cfg(feature = "capture-in-process")]
            Provider::InProcessCapture(ref provider) => provider.raster(runtime_abi, batch, images, mode),
        }
    }
}
