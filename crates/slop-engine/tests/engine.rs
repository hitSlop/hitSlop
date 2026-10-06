//! The private process boundary: typed JSON, frozen preflight, and classified outcomes.
use hitslop_core::envelope::{self, Envelope};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
fn protocol() -> String {
    hitslop_core::command::protocol().parse::<Value>().unwrap()["version"].to_string()
}
fn invoke(args: &[&str], input: &[u8], helper: Option<&Path>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_slop-engine"));
    command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(helper) = helper {
        command.env("HITSLOP_NATIVE_CLI", helper);
    }
    let mut child = command.spawn().unwrap();
    let _ = child.stdin.take().unwrap().write_all(input);
    child.wait_with_output().unwrap()
}
fn request(body: Value) -> Value {
    reply(&body.to_string(), None)
}
fn reply(input: &str, helper: Option<&Path>) -> Value {
    let output = invoke(&["--client-protocol", &protocol()], input.as_bytes(), helper);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stderr.is_empty());
    assert!(envelope::is_valid(Envelope::EngineReply, &output.stdout), "{}", String::from_utf8_lossy(&output.stdout));
    serde_json::from_slice(&output.stdout).unwrap()
}
/// A build's `app.json` with these initial values.
fn write_app(stage: &Path, initial: &str) {
    let manifest = r#"{"author":{"name":"A"},"slug":"engine","title":"Engine","description":"Packs.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#;
    let descriptor = r#"{"kind":"object","properties":{"title":{"kind":"text"}}}"#;
    let app = format!(
        r##"{{"packageFormat":1,"runtimeABI":1,"manifest":{manifest},"descriptor":{descriptor},"initial":{initial},"theme":{{"accent":"#335577"}}}}"##
    );
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
fn packs_creates_and_reads_typed_results() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Template file.slop");
    assert_eq!(
        request(json!({"method":"pack","stage":stage(dir.path()),"file":file})),
        json!({"ok":true,"method":"pack"})
    );
    let inspected = request(json!({"method":"inspect","file":file}));
    assert_eq!(inspected["info"]["kind"], "template");
    assert_eq!(inspected["info"]["live"], false);
    assert_eq!(request(json!({"method":"schema","file":file}))["schema"]["properties"]["title"]["kind"], "text");
    let output = dir.path().join("-Document file.slop");
    let created = request(json!({"output":output,"method":"create","from":file}));
    assert_eq!(created["documentPath"], fs::canonicalize(&output).unwrap().to_str().unwrap());
    assert_eq!(request(json!({"method":"get","documentPath":output}))["state"]["value"], json!({"title":"Hello"}));
    assert_eq!(request(json!({"method":"create","from":file,"output":output}))["ok"], false);
}
#[test]
fn invalid_requests_are_classified_and_never_publish() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    write_app(&stage, r#"{"title":7}"#);
    let output = dir.path().join("Refused.slop");
    let refused = request(json!({"method":"pack","stage":stage,"file":output}));
    assert_eq!(refused["ok"], false);
    assert_eq!(refused["code"], "rejected");
    assert!(!output.exists());
    for input in [
        "{broken",
        "{}",
        r#"{"method":"templates","extra":true}"#,
        r#"{"method":"screenshot","documentPath":"x","output":"y","target":"other","ifPresent":false}"#,
        r#"{"method":"get","documentPath":"x","protocol":1}"#,
        r#"{"method":"validateApp"}"#,
        r#"{"method":"validateApp","app":null,"extra":true}"#,
    ] {
        assert_eq!(reply(input, None)["reason"], "invalid_request", "{input}");
    }
    let input = " ".repeat(hitslop_core::command::MAX_REQUEST_BYTES + 1);
    assert_eq!(reply(&input, None)["reason"], "too_large");
}
#[test]
fn app_validation_preserves_marker_order_and_limits() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let app: Value = serde_json::from_slice(&fs::read(stage.join("app.json")).unwrap()).unwrap();
    assert_eq!(request(json!({"method":"validateApp","app":app})), json!({"ok":true,"method":"validateApp"}));
    for (field, replacement) in [
        ("initial", json!({"title":7})),
        ("theme", json!({"accent":"#ABCDEF"})),
        ("descriptor", json!({"kind":"future"})),
        ("manifest", json!({"title":"Incomplete"})),
    ] {
        let mut app = app.clone();
        app[field] = replacement;
        fs::write(stage.join("app.json"), app.to_string()).unwrap();
        assert_eq!(
            request(json!({"method":"validateApp","app":app})),
            request(json!({"method":"pack","stage":stage,"file":dir.path().join("refused.slop")}))
        );
    }
    let mut future = app.clone();
    future["packageFormat"] = json!(999);
    future["manifest"] = json!({"future":true});
    assert_eq!(request(json!({"method":"validateApp","app":future}))["reason"], "requires_update");
    let future_number = r#"{"method":"validateApp","app":{"packageFormat":999,"runtimeABI":1,"manifest":{"future":1e999},"descriptor":{},"initial":{},"theme":{}}}"#;
    assert_eq!(reply(future_number, None)["reason"], "requires_update");
    let oversized = "x".repeat(hitslop_core::file::APP_INPUT_BYTES);
    let refused = request(json!({"method":"validateApp","app":oversized}));
    assert_eq!(refused["reason"], "invalid_request");
    assert!(refused["error"].as_str().unwrap().contains("large"));
}
#[test]
fn protocol_refusal_precedes_even_unparseable_arguments() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let current: u64 = protocol().parse().unwrap();
    for version in [current - 1, current + 1].map(|v| v.to_string()) {
        let output = Command::new(env!("CARGO_BIN_EXE_slop-engine"))
            .args([OsStr::new("--client-protocol"), OsStr::new(&version), OsStr::from_bytes(b"\xff")])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert_eq!(error.lines().count(), 1);
        assert!(error.contains("update"));
    }
}
#[test]
fn only_exact_bootstrap_modes_are_accepted() {
    let version = protocol();
    for args in [
        vec!["--evaluate-command", "--help"],
        vec!["--client-protocol", &version, "--evaluate-command"],
        vec!["--client-protocol"],
        vec!["--client-protocol", "invalid"],
        vec!["request"],
        vec!["--client-protocol", &version, "templates"],
        vec!["--protocol", "--build-id"],
    ] {
        let output = invoke(&args, b"", None);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
    }
    assert!(invoke(&["--protocol"], b"", None).status.success());
    assert!(invoke(&["--build-id"], b"", None).status.success());
}
#[test]
#[cfg(target_os = "macos")]
fn helper_receives_json_and_ambiguous_failures_stay_unknown() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let helper = dir.path().join("helper");
    let input = json!({"method":"screenshot","documentPath":"-Document file.slop","output":"-capture file.png","target":"icon","ifPresent":true}).to_string();
    for (body, code, outcome) in [
        (r#"{"ok":true,"method":"screenshot","output":null}"#, 0, "success"),
        (r#"{"ok":true,"method":"open","documentPath":"x"}"#, 0, "unknown_outcome"),
        (r#"{"ok":true,"method":"screenshot"}"#, 0, "unknown_outcome"),
        ("broken", 0, "unknown_outcome"),
        ("", 17, "unknown_outcome"),
    ] {
        let capture = dir.path().join("request");
        fs::write(&helper, format!("#!/bin/sh\n[ \"$1\" = --client-protocol ] && [ \"$#\" = 2 ] || exit 3\ncat > '{}'\nprintf '%s\\n' '{}'\nexit {code}\n", capture.display(), body)).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let result = reply(&input, Some(&helper));
        assert_eq!(fs::read_to_string(&capture).unwrap(), input);
        if outcome == "success" {
            assert_eq!(result["output"], Value::Null);
            assert_eq!(result["ok"], true);
        } else {
            assert_eq!(result["code"], outcome);
        }
    }
    fs::write(&helper, "#!/bin/sh\necho 'update hitSlop' >&2\nexit 2\n").unwrap();
    assert_eq!(reply(&input, Some(&helper))["reason"], "requires_update");
}
