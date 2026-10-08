//! Native transport adapter. All routing, framing and document commands live in Rust;
//! only a validated export request is handed to the native renderer.
use super::*;
use hitslop_core::command as core;
use hitslop_core::owner::Failure;

#[derive(uniffi::Record)]
pub struct NativeExportRequest {
    pub document_path: String,
    pub format: hitslop_core::engine::ExportFormat,
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
pub struct NativeSocketServer(hitslop_core::socket::Server);
#[uniffi::export]
impl NativeSocketServer {
    #[uniffi::constructor]
    pub fn start(owner: Arc<NativeOwner>, exporter: Box<dyn NativeExportHandler>) -> Result<Arc<Self>, CoreError> {
        Ok(Arc::new(Self(hitslop_core::socket::Server::start(owner.0.clone(), Arc::new(Exporter(exporter)))?)))
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
/// A request written in command `protocol`, as the engine runs it.
#[uniffi::export]
pub fn command_request(
    json: String,
    protocol: u64,
    exporter: Option<Box<dyn NativeExportHandler>>,
    completion: Box<dyn NativeCommandCompletion>,
) {
    std::thread::spawn(move || {
        let exporter = exporter.map(|e| Arc::new(Exporter(e)) as Arc<dyn core::ExportHandler>);
        completion.complete(core::request(&json, protocol, exporter));
    });
}
#[uniffi::export]
impl NativeOwner {
    /// One document request from the page `view`; the owner admits and answers it.
    pub fn page(&self, json: String, view: String, completion: Box<dyn PageCompletion>) {
        core::page(&self.0, view, &json, move |reply| completion.complete(reply));
    }
}
