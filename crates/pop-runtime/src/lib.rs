//! Serial document ownership, native commands, and live socket routing.
//! Semantics belong to pop-core, durable files to pop-storage, and rendering to pop-capture.
pub mod command;
pub mod owner;
#[cfg(feature = "desktop")]
pub mod socket;
#[cfg(not(feature = "desktop"))]
#[path = "socket_unavailable.rs"]
pub mod socket;

pub use pop_capture::{capture, widgets};
pub use pop_contracts::wire;
pub use pop_contracts::wire::engine::{EngineReply, EngineRequest, EngineSuccess};
pub use pop_contracts::wire::{
    ENGINE_PROTOCOL, HostLimits, NATIVE_RESOURCE_POLICY, OutcomeCode, engine, host_limits, page as page_wire, preview,
    socket as socket_wire,
};
pub use pop_core::{
    ATTACHMENT_FILE_BYTES, Anchor, AppSpec, Applied, Batch, Code, Document, Error, Intent, Origin, PACKAGE_FORMAT,
    RUNTIME_ABI, Reading, Segment, TextEdit, app, describe, sync, theme,
};
pub use pop_storage::registry;
pub use pop_storage::{file, images, media, store};

type Result<T> = std::result::Result<T, Error>;
fn parse<T: serde::de::DeserializeOwned>(input: &str) -> Result<T> {
    if input.len() > wire::JSON_BYTES {
        return Err(Error {
            code: Code::TooLarge,
            message: "JSON exceeds size limit".into(),
            op_index: None,
            path: vec![],
        });
    }
    serde_json::from_str(input).map_err(|error| Error {
        code: Code::InvalidRequest,
        message: error.to_string(),
        op_index: None,
        path: vec![],
    })
}
fn encode(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("runtime values encode as JSON")
}
fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}
#[cfg(feature = "desktop")]
fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0; bytes];
    getrandom::getrandom(&mut buffer).expect("random socket identity");
    buffer.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Shared foundation implementation identity used by the native app and engine.
pub const BUILD_ID: &str = env!("POP_RUNTIME_BUILD_ID");
