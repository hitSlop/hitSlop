//! Serial document ownership, native commands, and live socket routing.
//! Semantics belong to pop-core, durable files to pop-storage, and rendering to pop-runner.
pub mod capture;
pub mod command;
pub mod owner;
#[cfg(not(target_os = "ios"))]
pub mod socket;
#[cfg(target_os = "ios")]
#[path = "socket_unavailable.rs"]
pub mod socket;

use pop_contracts::{Error, encode, parse};
use pop_core::lock;
#[cfg(not(target_os = "ios"))]
use pop_core::random_hex;
type Result<T> = std::result::Result<T, Error>;

/// Shared foundation implementation identity used by the native app and engine.
pub const BUILD_ID: &str = env!("POP_RUNTIME_BUILD_ID");

/// Selects the host's execution policy: iOS captures on the caller's thread;
/// other hosts launch restricted child processes.
pub fn evaluator(executable: std::path::PathBuf) -> std::result::Result<pop_runner::Evaluator, String> {
    #[cfg(target_os = "ios")]
    {
        pop_runner::Evaluator::in_process_capture(executable)
    }
    #[cfg(not(target_os = "ios"))]
    {
        pop_runner::Evaluator::process(executable, vec![])
    }
}
