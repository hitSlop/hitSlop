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
    pub fn process(executable: std::path::PathBuf, arguments: Vec<String>) -> Result<Self, Error> {
        native::Evaluator::new(executable, arguments).map(|provider| Self { provider: Provider::Process(provider) })
    }
    /// Run capture on the caller's background thread, without process isolation.
    /// Authored commands remain unavailable even when process support is also built.
    #[cfg(feature = "in-process")]
    pub fn in_process_capture(executable: std::path::PathBuf) -> Result<Self, Error> {
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
    pub fn run(&self, runtime_abi: u64, bundle: &str, request: &str) -> Result<String, Error> {
        match self.provider {
            #[cfg(not(target_os = "ios"))]
            Provider::Process(ref provider) => provider.run(runtime_abi, bundle, request),
            #[cfg(feature = "in-process")]
            Provider::InProcessCapture(ref provider) => provider.run(runtime_abi, bundle, request),
        }
    }
    /// Collect capture markup using the configured provider.
    pub fn markup(&self, runtime_abi: u64, bundle: &str, request: &str, widget: bool) -> Result<Batch, Error> {
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
        images: std::collections::BTreeMap<String, Vec<u8>>,
        mode: JobKind,
    ) -> Result<Vec<Vec<u8>>, Error> {
        match self.provider {
            #[cfg(not(target_os = "ios"))]
            Provider::Process(ref provider) => provider.raster(runtime_abi, batch, images, mode),
            #[cfg(feature = "in-process")]
            Provider::InProcessCapture(ref provider) => provider.raster(runtime_abi, batch, images, mode),
        }
    }
}

/// Failures distinguish authored programs from host execution and reply admission.
#[derive(Clone, Debug)]
pub enum Error {
    Timeout,
    Memory,
    Spawn(String),
    Sandbox(String),
    Program(String),
    Output(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => f.write_str("Evaluator execution timed out"),
            Self::Memory => f.write_str("Evaluator memory limit exceeded"),
            Self::Spawn(message) | Self::Sandbox(message) | Self::Program(message) | Self::Output(message) => {
                f.write_str(message)
            }
        }
    }
}
impl std::error::Error for Error {}

/// The child's failure line. `ok` and `error` are the reply shape every reader of the child
/// knows (the authoring tools read them directly); `kind` lets the parent restore the typed error.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FailedReply {
    ok: pop_contracts::wire::engine::False,
    error: String,
    kind: FailureKind,
}
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum FailureKind {
    Timeout,
    Memory,
    Spawn,
    Sandbox,
    Program,
    Output,
}
impl From<Error> for FailedReply {
    fn from(error: Error) -> Self {
        let kind = match &error {
            Error::Timeout => FailureKind::Timeout,
            Error::Memory => FailureKind::Memory,
            Error::Spawn(_) => FailureKind::Spawn,
            Error::Sandbox(_) => FailureKind::Sandbox,
            Error::Program(_) => FailureKind::Program,
            Error::Output(_) => FailureKind::Output,
        };
        Self { ok: pop_contracts::wire::engine::False, error: error.to_string(), kind }
    }
}
impl From<FailedReply> for Error {
    fn from(reply: FailedReply) -> Self {
        match reply.kind {
            FailureKind::Timeout => Self::Timeout,
            FailureKind::Memory => Self::Memory,
            FailureKind::Spawn => Self::Spawn(reply.error),
            FailureKind::Sandbox => Self::Sandbox(reply.error),
            FailureKind::Program => Self::Program(reply.error),
            FailureKind::Output => Self::Output(reply.error),
        }
    }
}

/// Image types a raster job accepts, as (content test, data-URL header). The child's request
/// carries images as data URLs; in-process callers pass the bytes.
type ImageHeader = (fn(&[u8]) -> bool, &'static str);
#[cfg(any(feature = "capture", not(target_os = "ios")))]
const IMAGE_HEADERS: [ImageHeader; 4] = [
    (|b| b.starts_with(b"\x89PNG\r\n\x1a\n"), "data:image/png;base64"),
    (|b| b.starts_with(&[0xff, 0xd8, 0xff]), "data:image/jpeg;base64"),
    (|b| b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a"), "data:image/gif;base64"),
    (|b| b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP", "data:image/webp;base64"),
];

fn output_error(error: impl ToString) -> Error {
    Error::Output(error.to_string())
}
fn program_error(error: impl ToString) -> Error {
    Error::Program(error.to_string())
}
