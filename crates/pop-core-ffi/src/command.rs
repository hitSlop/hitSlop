//! Native transport adapter. All routing, framing and document commands live in Rust;
//! the native page only drains pending edits before Rust acquires a saved source.
use super::*;
use pop_contracts::wire::engine::ExportFormat;
use pop_contracts::wire::{Code, OutcomeCode};
use pop_runtime::command as core;
use pop_runtime::owner::Failure;

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

#[derive(uniffi::Object)]
pub struct NativePageSettlement(Arc<core::PageSettlement>);
#[uniffi::export]
impl NativePageSettlement {
    pub fn is_active(&self) -> bool {
        self.0.is_active()
    }
    pub fn complete(&self, failure: Option<Failure>) -> bool {
        self.0.complete(failure.map_or(Ok(()), Err))
    }
}
#[uniffi::export(callback_interface)]
pub trait NativePageBarrier: Send + Sync {
    fn settle(&self, completion: Arc<NativePageSettlement>);
    fn resume(&self);
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
struct PageBarrier(Box<dyn NativePageBarrier>);
impl core::PageBarrier for PageBarrier {
    fn settle(&self, completion: Arc<core::PageSettlement>) {
        self.0.settle(Arc::new(NativePageSettlement(completion)));
    }
    fn resume(&self) {
        self.0.resume();
    }
}
#[derive(uniffi::Object)]
pub struct NativeSocketServer(pop_runtime::socket::Server);
#[uniffi::export]
impl NativeSocketServer {
    #[uniffi::constructor]
    pub fn start(owner: Arc<NativeOwner>, page: Box<dyn NativePageBarrier>) -> Result<Arc<Self>, CoreError> {
        Ok(Arc::new(Self(pop_runtime::socket::Server::start(owner.0.clone(), Some(Arc::new(PageBarrier(page))))?)))
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
    pop_contracts::wire::page::PagePush::resync_json()
}
