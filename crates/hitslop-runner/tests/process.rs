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

#[test]
fn render_jobs_have_their_own_output_and_time_budgets() {
    let evaluator = Evaluator::new(
        "/bin/sh".into(),
        vec!["-c".into(), "/bin/cat >/dev/null; exec /usr/bin/head -c 8388609 /dev/zero".into()],
    )
    .unwrap();
    assert!(evaluator.widget(1, "", "{}").unwrap_err().contains("too large"));
    assert_eq!(evaluator.capture(1, "", "{}").unwrap().len(), 8_388_609);

    // Capture and PDF allow the same output size, but only PDF gets a longer watchdog.
    let evaluator =
        Evaluator::new("/bin/sh".into(), vec!["-c".into(), "/bin/cat >/dev/null; /bin/sleep 4; printf done".into()])
            .unwrap();
    assert!(evaluator.capture(1, "", "{}").unwrap_err().contains("timed out"));
    assert_eq!(evaluator.pdf(1, "", "{}").unwrap(), "done");
}
