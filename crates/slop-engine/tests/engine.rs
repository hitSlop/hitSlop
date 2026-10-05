//! The engine as the CLI runs it: a stage packs into a template, which reads back; a refused
//! build prints why and publishes nothing.
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::io::Write;

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

fn validate(input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_slop-engine"))
        .arg("validate-app").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    // An oversized request can close the reader before every extra byte is written.
    let _ = child.stdin.take().unwrap().write_all(input);
    child.wait_with_output().unwrap()
}

#[test]
fn validates_metadata_without_assets_and_matches_pack_refusals() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let good = fs::read(stage.join("app.json")).unwrap();
    fs::remove_dir_all(stage.join("assets")).unwrap();
    assert!(validate(&good).status.success());
    fs::create_dir(stage.join("assets")).unwrap();
    fs::write(stage.join("assets/app.js"), "export default {};").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&good).unwrap();
    for (field, replacement) in [
        ("initial", serde_json::json!({"title": 7})),
        ("theme", serde_json::json!({"accent": "#ABCDEF"})),
        ("descriptor", serde_json::json!({"kind": "future"})),
        ("manifest", serde_json::json!({"title": "Incomplete"})),
        ("packageFormat", serde_json::json!(999)),
    ] {
        let mut value = value.clone();
        value[field] = replacement;
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(stage.join("app.json"), &bytes).unwrap();
        let checked = validate(&bytes);
        let packed = engine(&[Path::new("pack"), &stage, &dir.path().join("refused.slop")]);
        assert_eq!(checked.status.code(), Some(1), "{field}");
        assert_eq!(checked.stderr, packed.stderr, "{field}");
    }
    assert_eq!(validate(b"{broken").status.code(), Some(1));
    let too_large = vec![b' '; hitslop_core::file::APP_INPUT_BYTES + 1];
    let refused = validate(&too_large);
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("large"));
}
