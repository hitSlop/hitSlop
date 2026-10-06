//! The engine as the CLI runs it: a stage packs into a template, which reads back; a refused
//! build prints why and publishes nothing.
use std::fs;
use std::path::Path;
use std::process::Command;

fn engine(args: &[&Path]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_slop-engine")).args(args).output().unwrap()
}

/// A build's `app.json` with these initial values.
fn write_app(stage: &Path, initial: &str) {
    let manifest = r#"{"author":{"name":"A"},"slug":"engine","title":"Engine","description":"Packs.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#;
    let descriptor = r#"{"kind":"object","properties":{"title":{"kind":"text"}}}"#;
    let app = format!(r##"{{"packageFormat":1,"runtimeABI":1,"manifest":{manifest},"descriptor":{descriptor},"initial":{initial},"theme":{{"accent":"#335577"}}}}"##);
    fs::write(stage.join("app.json"), app).unwrap();
}

fn stage(dir: &Path) -> std::path::PathBuf {
    let stage = dir.join("stage");
    fs::create_dir_all(stage.join("assets")).unwrap();
    write_app(&stage, r#"{"title":"Hello"}"#);
    fs::write(stage.join("assets/app.js"), "export default { mount() { return {}; } };").unwrap();
    stage
}

#[test]
fn packs_a_stage_and_reads_the_file_back() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("engine.slop");
    let packed = engine(&[Path::new("pack"), &stage(dir.path()), &out]);
    assert!(packed.status.success(), "{}", String::from_utf8_lossy(&packed.stderr));
    let inspected = engine(&[Path::new("inspect"), &out]);
    let value: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(value["kind"], "template");
    assert_eq!(value["manifest"]["slug"], "engine");
    assert_eq!(value["live"], false);
    let schema = engine(&[Path::new("schema"), &out]);
    let schema: serde_json::Value = serde_json::from_slice(&schema.stdout).unwrap();
    assert_eq!(schema["properties"]["title"]["kind"], "text");
}

#[test]
fn a_refused_build_says_why_and_publishes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    write_app(&stage, r#"{"title":7}"#);
    let out = dir.path().join("engine.slop");
    let refused = engine(&[Path::new("pack"), &stage, &out]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&refused.stderr).trim().is_empty());
    assert!(!out.exists());
    assert_eq!(engine(&[Path::new("unknown")]).status.code(), Some(2), "a usage error");
}
