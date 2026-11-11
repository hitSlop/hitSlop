#[path = "../build-fingerprint.rs"]
mod fingerprint;
fn main() {
    fingerprint::emit(
        "POP_RUNTIME_BUILD_ID",
        &["pop-runtime", "pop-core", "pop-contracts", "pop-storage", "pop-runner", "pop-engine"],
        &["crates/pop-runner/fonts"],
    );
}
