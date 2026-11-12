use pop_runner::Evaluator;
#[cfg(all(not(target_os = "ios"), feature = "in-process"))]
#[test]
fn explicit_capture_and_process_providers_keep_distinct_command_authority() {
    let capture = Evaluator::in_process_capture(std::env::current_exe().unwrap()).unwrap();
    assert!(!capture.supports_commands());
    assert!(
        Evaluator::process(std::env::current_exe().unwrap(), vec!["--evaluate-command".into()])
            .unwrap()
            .supports_commands()
    );
    assert!(capture.run(1, "", "{}").is_err(), "capture does not acquire commands from feature unification");
}
