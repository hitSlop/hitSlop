#[path = "../build-fingerprint.rs"]
mod fingerprint;
fn main() {
    fingerprint::emit(
        "POP_RUNTIME_BUILD_ID",
        &[
            "pop-runtime",
            "pop-core",
            "pop-contracts",
            "pop-storage",
            "pop-capture",
            "pop-sharing-protocol",
            "pop-runner",
            "pop-renderer",
        ],
        &["crates/pop-renderer/fonts"],
    );
}
