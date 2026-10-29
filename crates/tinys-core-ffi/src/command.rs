//! Native transport adapter. All routing, framing and document commands live in Rust;
//! only a live owner's validated export request is handed to the native renderer.
use super::*;
use tinys_core::command as core;
use tinys_core::engine::ExportFormat;
use tinys_core::owner::Failure;
use tinys_core::{Code, OutcomeCode};

#[uniffi::remote(Enum)]
pub enum ExportFormat {
    Png,
    Pdf,
}
#[uniffi::remote(Enum)]
pub enum OutcomeCode {
    Rejected,
    OwnerReplaced,
    Closing,
    SaveFailed,
    OwnerInvalidated,
    UnknownOutcome,
}
#[uniffi::remote(Enum)]
pub enum Code {
    TypeMismatch,
    OutOfRange,
    PathNotFound,
    InvalidKey,
    Exists,
    DuplicateId,
    InvalidRequest,
    InvalidId,
    InvalidPath,
    InvalidSchema,
    TooLarge,
    StaleBase,
    InvalidBytes,
    MissingDependencies,
    EngineError,
    InvalidShape,
    RequiresUpdate,
    IsTemplate,
    Refused,
}
#[uniffi::export]
pub fn parse_core_code(name: String) -> Option<Code> {
    Code::from_name(&name)
}
#[uniffi::export]
pub fn core_code_name(code: Code) -> String {
    code.as_str().into()
}
#[uniffi::export]
pub fn parse_outcome_code(name: String) -> Option<OutcomeCode> {
    OutcomeCode::from_name(&name)
}
#[uniffi::export]
pub fn outcome_code_name(code: OutcomeCode) -> String {
    code.name()
}

#[derive(uniffi::Record)]
pub struct NativeExportRequest {
    pub document_path: String,
    pub format: Option<ExportFormat>,
    pub output: String,
}
#[derive(uniffi::Enum)]
pub enum NativeExportOutcome {
    Success { output: String },
    Failure { failure: Failure },
}
#[derive(uniffi::Object)]
pub struct NativeExportCompletion(Arc<core::ExportCompletion>);
#[uniffi::export]
impl NativeExportCompletion {
    pub fn is_active(&self) -> bool {
        self.0.is_active()
    }
    pub fn complete(&self, outcome: NativeExportOutcome) {
        self.0.complete(match outcome {
            NativeExportOutcome::Success { output } => Ok(output),
            NativeExportOutcome::Failure { failure } => Err(failure),
        });
    }
}
#[uniffi::export(callback_interface)]
pub trait NativeExportHandler: Send + Sync {
    fn export(&self, request: NativeExportRequest, completion: Arc<NativeExportCompletion>);
}
/// Consulted synchronously after the staged export is synced, immediately before publishing.
#[uniffi::export(callback_interface)]
pub trait NativeExportActivity: Send + Sync {
    fn is_active(&self) -> bool;
}
/// The same exclusive, durable publisher used by closed-document exports. CoreError
/// preserves rejected/exists and the unknown outcome of a post-publication sync failure.
#[uniffi::export]
pub fn publish_export(
    bytes: Vec<u8>,
    output: String,
    activity: Box<dyn NativeExportActivity>,
) -> Result<(), CoreError> {
    Ok(tinys_core::capture::publish_export(&bytes, Path::new(&output), || activity.is_active())?)
}
#[uniffi::export(callback_interface)]
pub trait NativeCommandCompletion: Send + Sync {
    fn complete(&self, reply_json: String);
}
/// A page request's answer: the reply for the page, and the owner's failure when it refused.
#[uniffi::export(callback_interface)]
pub trait PageCompletion: Send + Sync {
    fn complete(&self, reply: core::PageDispatch);
}
struct Exporter(Box<dyn NativeExportHandler>);
impl core::ExportHandler for Exporter {
    fn export(&self, request: core::ExportRequest, completion: Arc<core::ExportCompletion>) {
        self.0.export(
            NativeExportRequest {
                document_path: request.document_path,
                format: request.format,
                output: request.output,
            },
            Arc::new(NativeExportCompletion(completion)),
        );
    }
}
#[derive(uniffi::Object)]
pub struct NativeSocketServer(tinys_core::socket::Server);
#[uniffi::export]
impl NativeSocketServer {
    #[uniffi::constructor]
    pub fn start(owner: Arc<NativeOwner>, exporter: Box<dyn NativeExportHandler>) -> Result<Arc<Self>, CoreError> {
        Ok(Arc::new(Self(tinys_core::socket::Server::start(owner.0.clone(), Arc::new(Exporter(exporter)))?)))
    }
    pub fn publish(&self) -> Result<(), CoreError> {
        Ok(self.0.publish()?)
    }
    pub fn withdraw(&self) {
        self.0.withdraw();
    }
    pub fn stop(&self) {
        self.0.stop();
    }
}
/// A page or socket reply refusing with `failure`, encoded as the core encodes its own: the
/// one place a failure's kind becomes the outcome a caller reads.
#[uniffi::export]
pub fn failure_reply(failure: Failure) -> String {
    core::failure(failure, false, false)
}
/// A request written in command `protocol`, as the engine runs it, without its restricted
/// evaluator: a closed document's export is refused.
#[uniffi::export]
pub fn command_request(json: String, protocol: u64, completion: Box<dyn NativeCommandCompletion>) {
    std::thread::spawn(move || completion.complete(core::request(&json, protocol)));
}
#[uniffi::export]
impl NativeOwner {
    /// One document request from the page `view`; the owner admits and answers it.
    pub fn page(&self, json: String, view: String, completion: Box<dyn PageCompletion>) {
        core::page(&self.0, view, &json, move |reply| completion.complete(reply));
    }
}

#[uniffi::export]
pub fn resync_push() -> String {
    tinys_core::page_wire::PagePush::resync_json()
}
