//! The app's private evaluator helper. It accepts one bounded request on stdin.
fn main() {
    if std::env::args_os().len() != 1 {
        eprintln!("hitslop-evaluator takes its request on stdin");
        std::process::exit(64);
    }
    hitslop_runner::child();
}
