//! The private process boundary: typed JSON, frozen preflight, and classified outcomes.
use hitslop_core::EngineReply;
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
    assert!(
        serde_json::from_slice::<EngineReply>(&output.stdout).is_ok(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
/// Explicit compiler input; no metadata file is discovered in the stage.
fn app(initial: Value) -> Value {
    json!({
        "packageFormat":1,"runtimeABI":1,
        "declaration":{
            "metadata":{"author":{"name":"A"},"slug":"engine","title":"Engine","description":"Packs.","categories":["utilities"]},
            "window":{"kind":"standard","width":320,"height":240},
            "document":{"kind":"object","properties":{"title":{"kind":"text"}}},
            "initial":initial,"theme":[{"token":"accent","color":"#335577"}],
            "commands":[],"views":{"export":false,"icon":false}
        },
        "roles":{"ui":"ui.js"},
        "resources":[{"kind":"app","key":"ui.js","mediaType":"text/javascript","path":"assets/ui.js"}],
        "artwork":{}
    })
}

fn stage(dir: &Path) -> std::path::PathBuf {
    let stage = dir.join("stage");
    fs::create_dir_all(stage.join("assets")).unwrap();
    fs::write(stage.join("assets/ui.js"), "export default { mount() { return {}; } };").unwrap();
    stage
}

#[test]
fn packs_creates_and_reads_typed_results() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Template file.slop");
    assert_eq!(
        request(json!({"method":"pack","stage":stage(dir.path()),"file":file,"app":app(json!({"title":"Hello"}))})),
        json!({"ok":true,"method":"pack"})
    );
    let inspected = request(json!({"method":"inspect","file":file}));
    assert_eq!(inspected["info"]["kind"], "template");
    assert_eq!(inspected["info"]["live"], false);
    assert_eq!(inspected["info"]["metadata"]["categories"], json!(["utilities"]));
    assert_eq!(
        inspected["info"]["assets"],
        json!([{"name":"ui.js","bytes":fs::metadata(dir.path().join("stage/assets/ui.js")).unwrap().len()}])
    );
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
    let output = dir.path().join("Refused.slop");
    let refused = request(json!({"method":"pack","stage":stage,"file":output,"app":app(json!({"title":7}))}));
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
fn the_engine_keeps_its_private_evaluator_entrypoint() {
    let input = json!({"runtimeABI":1,"mode":"definition","bundle":"globalThis.__hitslopDescribe=()=>JSON.stringify({ok:true,probe:'shared runner'});","request":"{}"});
    let output = invoke(&["--evaluate-command"], input.to_string().as_bytes(), None);
    assert!(output.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap(), json!({"ok":true,"probe":"shared runner"}));
}
#[test]
fn app_validation_preserves_marker_order_and_limits() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let app = app(json!({"title":"Hello"}));
    assert_eq!(
        request(json!({"method":"validateApp","stage":stage,"app":app})),
        json!({"ok":true,"method":"validateApp"})
    );
    for (field, replacement) in [
        ("initial", json!({"title":7})),
        ("theme", json!([{"token":"accent","color":"invalid"}])),
        ("document", json!({"kind":"future"})),
        ("metadata", json!({"title":"Incomplete"})),
    ] {
        let mut app = app.clone();
        app["declaration"][field] = replacement;
        let refused = request(json!({"method":"validateApp","stage":stage,"app":app}));
        assert_eq!(refused["ok"], false, "{field}");
        assert_eq!(
            refused,
            request(json!({"method":"pack","stage":stage,"app":app,"file":dir.path().join("refused.slop")}))
        );
    }
    let mut future = app.clone();
    future["packageFormat"] = json!(999);
    future["declaration"] = json!({"future":true});
    assert_eq!(request(json!({"method":"validateApp","stage":stage,"app":future}))["reason"], "requires_update");
    let future_number = r#"{"method":"validateApp","stage":"/unused","app":{"packageFormat":999,"runtimeABI":1,"declaration":{"future":1e999}}}"#;
    assert_eq!(reply(future_number, None)["reason"], "requires_update");
    let oversized = "x".repeat(hitslop_core::file::APP_INPUT_BYTES);
    let refused = request(json!({"method":"validateApp","stage":stage,"app":oversized}));
    assert_eq!(refused["reason"], "too_large");
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

#[test]
fn a_preview_owner_refuses_an_unknown_page_request_and_keeps_serving() {
    let dir = tempfile::tempdir().unwrap();
    let template = dir.path().join("Preview template.slop");
    request(json!({"method":"pack","stage":stage(dir.path()),"file":template,"app":app(json!({"title":"Hello"}))}));
    let document = dir.path().join("Preview.slop");
    request(json!({"method":"create","from":template,"output":document}));
    let frames = [
        json!({"type":"page","id":1,"request":{"method":"future.method"}}),
        json!({"type":"page","id":2,"request":{"method":"open"}}),
    ];
    // EOF also terminates the last frame, without requiring a trailing newline.
    let input = frames.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let output = invoke(
        &["--client-protocol", &protocol(), "--preview-owner", document.to_str().unwrap()],
        input.as_bytes(),
        None,
    );
    let replies: Vec<Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|frame: &Value| frame["type"] == "reply" || frame["type"] == "fatal")
        .collect();
    assert!(output.status.success(), "{replies:?}");
    // Page replies are JSON text, refusals included, exactly as the native host sends them.
    let reply = |n: usize| serde_json::from_str::<Value>(replies[n]["reply"].as_str().unwrap()).unwrap();
    assert_eq!((replies[0]["id"].clone(), reply(0)["reason"].clone()), (json!(1), json!("invalid_request")));
    assert_eq!((replies[1]["id"].clone(), reply(1)["ok"].clone()), (json!(2), json!(true)));
}

#[test]
fn a_preview_with_stalled_output_closes_and_saves_without_waiting_for_its_reader() {
    stalled_preview(true);
}

#[test]
fn a_preview_with_stalled_output_fails_even_while_its_input_remains_open() {
    stalled_preview(false);
}

fn stalled_preview(close_input: bool) {
    use std::io::{BufRead, BufReader};
    use std::time::{Duration, Instant};
    struct Reap(std::process::Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let template = dir.path().join("Template.slop");
    let document = dir.path().join("Preview.slop");
    request(json!({"method":"pack","stage":stage(dir.path()),"file":template,"app":app(json!({"title":"Before"}))}));
    request(json!({"method":"create","from":template,"output":document}));
    let mut child = Reap(
        Command::new(env!("CARGO_BIN_EXE_slop-engine"))
            .args(["--client-protocol", &protocol(), "--preview-owner", document.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert_ne!(output.read_line(&mut line).unwrap(), 0);
        if serde_json::from_str::<Value>(&line).unwrap()["type"] == "ready" {
            break;
        }
    }
    // Keep stdout open but unread. This publication exceeds the pipe buffer.
    let title = "saved despite a stalled reader".repeat(8192);
    let batch = json!({"intents":[{"type":"set","path":["title"],"value":title}]}).to_string();
    let mut input = child.0.stdin.take().unwrap();
    writeln!(input, "{}", json!({"type":"page","id":1,"request":{"method":"apply","batch":batch}})).unwrap();
    let input = (!close_input).then_some(input);
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "preview shutdown waited for its stdout reader");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success(), "a stalled transport is a failed preview session");
    drop(input);
    assert_eq!(request(json!({"method":"get","documentPath":document}))["state"]["value"]["title"], title);
}
