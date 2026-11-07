#[path = "../build-fingerprint.rs"]
mod fingerprint;
fn main() {
    fingerprint::emit("POP_CORE_BUILD_ID", &["pop-core", "pop-contracts", "pop-sharing-protocol"], &[]);
}
