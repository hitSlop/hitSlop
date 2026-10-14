//! The parent supervises real processes even when they do not behave like QuickJS.
use hitslop_runner::{Batch, Evaluator, JobKind};
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
    assert!(
        evaluator
            .raster(1, Batch { frames: vec![], fonts: vec![] }, Default::default(), JobKind::Widget)
            .unwrap_err()
            .contains("too large")
    );
    assert_eq!(
        evaluator
            .raster(1, Batch { frames: vec![], fonts: vec![] }, Default::default(), JobKind::Capture)
            .unwrap()
            .len(),
        8_388_609
    );

    // Capture and PDF allow the same output size, but only PDF gets a longer watchdog.
    let evaluator =
        Evaluator::new("/bin/sh".into(), vec!["-c".into(), "/bin/cat >/dev/null; /bin/sleep 4; printf done".into()])
            .unwrap();
    assert!(
        evaluator
            .raster(1, Batch { frames: vec![], fonts: vec![] }, Default::default(), JobKind::Capture)
            .unwrap_err()
            .contains("timed out")
    );
    assert_eq!(
        evaluator.raster(1, Batch { frames: vec![], fonts: vec![] }, Default::default(), JobKind::Pdf).unwrap(),
        "done"
    );
}

/// Quotes stress JSON escaping: nesting markup as another JSON string would make
/// the released markup + image budgets exceed the transport envelope.
#[test]
fn raster_input_holds_the_markup_and_image_budgets_together() {
    let evaluator =
        Evaluator::new("/bin/sh".into(), vec!["-c".into(), "/bin/cat >/dev/null; printf done".into()]).unwrap();
    let batch = Batch {
        frames: vec![hitslop_runner::Frame {
            html: "\"".repeat((hitslop_runner::MARKUP_BYTES - 256) / 2),
            css: String::new(),
            width: 64,
            height: 64,
            kind: Some("preview".into()),
        }],
        fonts: vec![],
    };
    let images = std::collections::BTreeMap::from([(
        "a".repeat(64),
        format!("data:image/png;base64,{}", "A".repeat(hitslop_runner::IMAGE_BYTES / 3 * 4)),
    )]);
    assert_eq!(evaluator.raster(1, batch, images, JobKind::Capture).unwrap(), "done");
}
