//! The app loads the runner into a process that keeps SIGPIPE's default action, unlike a
//! Rust binary. An evaluator that exits before reading its input must not end that process.
use hitslop_runner::Evaluator;

#[test]
fn an_evaluator_that_stops_reading_cannot_end_the_host_process() {
    // SAFETY: restores the default disposition the native app runs with.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    // Exits at once without reading stdin; the input exceeds every pipe buffer.
    let evaluator = Evaluator::new("/usr/bin/true".into(), vec![]).unwrap();
    let result = evaluator.run(1, &"x".repeat(4 << 20), "{}");
    assert!(result.is_err(), "an evaluator that wrote nothing is a refusal: {result:?}");
}
