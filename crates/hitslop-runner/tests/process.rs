//! The parent supervises real processes even when they do not behave like QuickJS.
use hitslop_runner::Evaluator;
use std::time::{Duration, Instant};

fn run(script: &str) -> Result<String, String> {
    Evaluator::new("/bin/sh".into(), vec!["-c".into(), script.into()]).unwrap().run(1, "", "{}")
}

#[test]
fn a_complete_reply_follows_complete_input() {
    assert_eq!(run("/bin/cat >/dev/null; printf done").unwrap(), "done");
}

#[test]
fn unresponsive_and_oversized_children_are_stopped() {
    let start = Instant::now();
    assert!(run("/bin/cat >/dev/null; exec /bin/sleep 30").unwrap_err().contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(6));
    assert!(run("/bin/cat >/dev/null; exec /usr/bin/head -c 4194305 /dev/zero").unwrap_err().contains("too large"));
}
