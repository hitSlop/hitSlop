//! Release-only acceptance capture and replay.
mod acceptance;
mod limits;

pub use acceptance::{
    AcceptanceVector, AcceptedRow, acceptance_vector_is_large, acceptance_vectors, check_acceptance_vector,
};
pub use limits::check_recorded_limits;

fn encode(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("core values encode as JSON")
}
