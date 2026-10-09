//! What the engine and its development room share: the preview owner, and how a
//! document owner gets its restricted command evaluator.
use hitslop_core::command;
use hitslop_core::owner::{Failure, FailureKind};
pub mod preview;

/// A classified refusal, as every engine reply spells one.
pub fn rejected(reason: hitslop_core::Code, error: impl std::fmt::Display) -> String {
    command::failure(
        Failure { kind: FailureKind::Rejected, message: error.to_string(), reason: Some(reason), op_index: None },
        false,
        false,
    )
}
/// The restricted evaluator: this same executable, run with `--evaluate-command`.
pub fn evaluator() -> Result<hitslop_runner::Evaluator, String> {
    hitslop_runner::Evaluator::new(
        std::env::current_exe().map_err(|e| e.to_string())?,
        vec!["--evaluate-command".into()],
    )
}
