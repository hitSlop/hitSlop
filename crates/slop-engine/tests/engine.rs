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

#[test]
fn the_engine_does_not_expose_development_rooms() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("untouched.slop");
    for entry in ["--dev-room", "--dev-replica-owner"] {
        let result = invoke(&["--client-protocol", &protocol(), entry, path.to_str().unwrap()], b"{}\n", None);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!path.exists());
    }
}
fn invoke(args: &[&str], input: &[u8], app: Option<&Path>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_slop-engine"));
    command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(app) = app {
        command.env("HITSLOP_APP", app);
    }
    let mut child = command.spawn().unwrap();
    let _ = child.stdin.take().unwrap().write_all(input);
    child.wait_with_output().unwrap()
}
fn request(body: Value) -> Value {
    reply(&body.to_string(), None)
}

#[test]
fn attachment_references_use_stored_blob_identity_and_media_rules_without_a_document() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("pretend.png");
    fs::write(&file, b"abc").unwrap();
    assert_eq!(
        request(json!({"method":"attachments.ref","file":file})),
        json!({"ok":true,"method":"attachments.ref","reference":{
            "id":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "byteLength":3,"name":"pretend.png","mimeType":"application/octet-stream"
        }})
    );
    assert_eq!(fs::read(&file).unwrap(), b"abc");
    let png = dir.path().join("image.txt");
    fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
    assert_eq!(request(json!({"method":"attachments.ref","file":png}))["reference"]["mimeType"], "image/png");

    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    for path in [dir.path().to_path_buf(), link, dir.path().join("missing")] {
        assert_eq!(request(json!({"method":"attachments.ref","file":path}))["reason"], "invalid_request");
    }
    fs::File::create(&file).unwrap().set_len(hitslop_core::ATTACHMENT_FILE_BYTES as u64 + 1).unwrap();
    assert_eq!(request(json!({"method":"attachments.ref","file":file}))["reason"], "too_large");
}

fn reply(input: &str, app: Option<&Path>) -> Value {
    let output = invoke(&["--client-protocol", &protocol()], input.as_bytes(), app);
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
            "commands":[],"views":{"preview":true,"export":true,"icon":true}
        },
        "roles":{"ui":"ui.js","preview":"preview.js","icon":"icon.js","export":"export.js"},
        "resources":[{"kind":"app","key":"ui.js","mediaType":"text/javascript","path":"assets/ui.js"},{"kind":"app","key":"preview.js","mediaType":"text/javascript","path":"assets/preview.js"},{"kind":"app","key":"icon.js","mediaType":"text/javascript","path":"assets/icon.js"},{"kind":"app","key":"export.js","mediaType":"text/javascript","path":"assets/export.js"}],
        "artwork":{}
    })
}

fn stage(dir: &Path) -> std::path::PathBuf {
    let stage = dir.join("stage");
    fs::create_dir_all(stage.join("assets")).unwrap();
    fs::write(stage.join("assets/ui.js"), "export default { mount() { return {}; } };").unwrap();
    fs::write(
        stage.join("assets/export.js"),
        include_str!("../../../tests/fixtures/checklist/document/assets/export.js"),
    )
    .unwrap();
    std::fs::copy(stage.join("assets/export.js"), stage.join("assets/preview.js")).unwrap();
    std::fs::copy(stage.join("assets/export.js"), stage.join("assets/icon.js")).unwrap();
    stage
}

#[test]
fn packs_creates_and_reads_typed_results() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Template file.slop");
    assert_eq!(
        request(json!({"method":"pack","stage":stage(dir.path()),"file":file,"app":app(json!({"title":"Hello"}))})),
        json!({"ok":true,"method":"pack","artwork":{"preview":"absent","icon":"absent"}})
    );
    let inspected = request(json!({"method":"inspect","file":file}));
    assert_eq!(inspected["info"]["kind"], "template");
    assert_eq!(inspected["info"]["live"], false);
    assert_eq!(inspected["info"]["metadata"]["categories"], json!(["utilities"]));
    assert_eq!(
        inspected["info"]["assets"],
        json!([{"name":"export.js","bytes":fs::metadata(dir.path().join("stage/assets/export.js")).unwrap().len()},{"name":"icon.js","bytes":fs::metadata(dir.path().join("stage/assets/icon.js")).unwrap().len()},{"name":"preview.js","bytes":fs::metadata(dir.path().join("stage/assets/preview.js")).unwrap().len()},{"name":"ui.js","bytes":fs::metadata(dir.path().join("stage/assets/ui.js")).unwrap().len()}])
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
    let input = json!({"runtimeABI":1,"mode":"definition","bundle":"globalThis.__slopDescribe=()=>JSON.stringify({ok:true,probe:'shared runner'});","request":"{}"});
    let output = invoke(&["--evaluate-command"], input.to_string().as_bytes(), None);
    assert!(output.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap(), json!({"ok":true,"probe":"shared runner"}));
}
#[test]
fn app_validation_preserves_marker_order_and_limits() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let app = app(json!({"title":"Hello"}));
    let validated = request(json!({"method":"validateApp","stage":stage,"app":app}));
    assert_eq!(validated["ok"], true);
    assert_eq!(validated["method"], "validateApp");
    assert_eq!(validated["window"]["frame"]["kind"], "standard");
    assert_eq!(validated["window"]["frame"]["frame"]["resizable"], true);
    assert_eq!(validated["window"]["frame"]["frame"]["radius"], "22px");
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
/// A template and a document made from it, from `stage()`'s fixture capture programs.
fn template_and_document(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let (template, document) = (dir.join("Template file.slop"), dir.join("-Document file.slop"));
    request(json!({"method":"pack","stage":stage(dir),"file":template,"app":app(json!({"title":"Exported"}))}));
    request(json!({"method":"create","from":template,"output":document}));
    (template, document)
}

/// The engine exports a closed document's saved state itself, in its own restricted
/// evaluator, and never replaces a file or writes the document.
#[test]
fn the_engine_exports_a_closed_document_without_the_app() {
    let dir = tempfile::tempdir().unwrap();
    let (template, document) = template_and_document(dir.path());
    let saved = fs::read(&document).unwrap();
    let export = |file: &Path, format: &str, output: &Path| {
        request(json!({"method":"export","documentPath":file,"format":format,"output":output}))
    };
    for (format, signature) in [("png", &b"\x89PNG\r\n\x1a\n"[..]), ("pdf", &b"%PDF-"[..])] {
        let output = dir.path().join(format!("-Export file.{format}"));
        assert_eq!(export(&document, format, &output), json!({"ok":true,"method":"export","output":output}));
        let bytes = fs::read(&output).unwrap();
        assert!(bytes.starts_with(signature), "{format}");
        let refused = export(&document, format, &output);
        assert_eq!((refused["code"].as_str(), refused["reason"].as_str()), (Some("rejected"), Some("exists")));
        assert_eq!(fs::read(&output).unwrap(), bytes);
    }
    let output = dir.path().join("Template.png");
    assert_eq!(export(&template, "png", &output)["reason"], "is_template");
    assert!(!output.exists());
    assert_eq!(fs::read(&document).unwrap(), saved);
}

/// A template opens by creating a document from it; that refusal comes before any app is
/// chosen, and a named app that is missing is never replaced by another copy.
#[test]
#[cfg(target_os = "macos")]
fn open_refuses_a_template_and_a_missing_named_app() {
    let dir = tempfile::tempdir().unwrap();
    let (template, document) = template_and_document(dir.path());
    let missing = dir.path().join("Missing.app");
    let open = |file: &Path| reply(&json!({"method":"open","documentPath":file}).to_string(), Some(&missing));
    assert_eq!(open(&template)["reason"], "is_template");
    let refused = open(&document);
    assert_eq!((refused["code"].as_str(), refused["reason"].as_str()), (Some("rejected"), Some("invalid_request")));
    assert!(refused["error"].as_str().unwrap().contains("HITSLOP_APP"), "{refused}");
}

/// Opening a window needs macOS and hitSlop.app; elsewhere it refuses and changes nothing.
#[test]
#[cfg(not(target_os = "macos"))]
fn opening_a_window_requires_macos() {
    let dir = tempfile::tempdir().unwrap();
    let (_, document) = template_and_document(dir.path());
    let refused = request(json!({"method":"open","documentPath":document}));
    assert_eq!(refused["reason"], "invalid_request");
    assert!(refused["error"].as_str().unwrap().contains("macOS"), "{refused}");
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

// A preview whose reader stalls closes and saves without waiting for it once its input
// closes, and fails even while its input remains open. Both wait out the write timeout, so
// they run at once.
#[test]
fn a_preview_with_stalled_output_saves_and_never_waits_for_its_reader() {
    std::thread::scope(|scope| {
        for run in [scope.spawn(|| stalled_preview(true)), scope.spawn(|| stalled_preview(false))] {
            if let Err(panic) = run.join() {
                std::panic::resume_unwind(panic);
            }
        }
    });
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
    let batch = json!({"intents":[{"type":"set","path":["title"],"value":title}]});
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

// Linux qualification: a template's missing artwork renders inside `pack`, in this engine's
// restricted evaluator, on every platform the engine ships for (no macOS host is involved).
// Each fixture draws the same page for every view, its body centred on a white canvas the
// view's size; the stored artwork is read back as the CLI reads it.
const WHITE: [u8; 4] = [255, 255, 255, 255];
fn fixture(dir: &Path, body: &str, css: &str) -> std::path::PathBuf {
    let program = format!(
        "globalThis.__slopCapture = json => {{ const r = JSON.parse(json); return JSON.stringify({{fonts:[],frames:[{{kind:r.kind,width:r.width,height:r.height,html:{html},css:'html,body{{margin:0;width:100%;height:'+r.height+'px;background:#fff}}body{{display:flex;align-items:center;justify-content:center}}'+{css}}}]}}); }};",
        html = serde_json::to_string(&format!("<html><body>{body}</body></html>")).unwrap(),
        css = serde_json::to_string(css).unwrap(),
    );
    let stage = stage(dir);
    for role in ["preview", "icon"] {
        fs::write(stage.join(format!("assets/{role}.js")), &program).unwrap();
    }
    let template = dir.join("Fixture.slop");
    let packed = request(
        json!({"method":"pack","stage":stage,"file":template,"app":app(json!({"title":"Fixture"})),"renderViews":true}),
    );
    assert_eq!(packed["artwork"], json!({"preview":"rendered","icon":"rendered"}), "{packed}");
    template
}
/// A template's stored artwork image, through `artwork.export`: its size, and its 8-bit RGBA
/// pixel at a position.
fn artwork(template: &Path, role: &str) -> ((u32, u32), impl Fn(u32, u32) -> [u8; 4]) {
    let output = template.with_file_name(format!("{role}.png"));
    let exported = request(json!({"method":"artwork.export","file":template,"target":role,"output":output}));
    assert_eq!(exported["output"], json!(output), "{exported}");
    let mut decoder = png::Decoder::new(std::io::Cursor::new(fs::read(&output).unwrap())).read_info().unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    let (width, channels) = (info.width, info.color_type.samples());
    ((info.width, info.height), move |x, y| {
        let pixel = &pixels[(y * width + x) as usize * channels..][..channels];
        [pixel[0], pixel[1], pixel[2], if channels == 4 { pixel[3] } else { 255 }]
    })
}

fn widget(app: &mut Value, stage: &Path, program: &str) {
    app["declaration"]["views"]["widget"] = json!({"refreshMinutes":60});
    app["roles"]["widget"] = json!("widget.js");
    app["resources"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind":"app","key":"widget.js","mediaType":"text/javascript","path":"assets/widget.js"}));
    fs::write(stage.join("assets/widget.js"), program).unwrap();
}

fn supplied_artwork(app: &mut Value, stage: &Path, role: &str) {
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, 512, 512);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&[0x33, 0x55, 0x77, 255].repeat(512 * 512)).unwrap();
    let path = format!("{role}.png");
    fs::write(stage.join(&path), &png).unwrap();
    app["artwork"][role] = json!(path);
}

fn refuses_view(app: Value, stage: &Path, destination: &Path, view: &str, cause: &str) {
    let packed = request(json!({"method":"pack","stage":stage,"file":destination,"app":app,"renderViews":true}));
    assert_eq!(packed["ok"], false, "{packed}");
    let error = packed["error"].as_str().unwrap();
    assert!(error.contains(&format!("{view}.svelte could not render:")), "{packed}");
    assert!(error.contains(cause), "{packed}");
    assert!(!destination.exists(), "a failed view must publish nothing");
    assert!(
        fs::read_dir(destination.parent().unwrap())
            .unwrap()
            .all(|entry| { !entry.unwrap().file_name().to_string_lossy().ends_with(".slop") }),
        "a failed pack must clean its staged template"
    );
}

#[test]
fn pack_refuses_an_export_dependency_that_uses_url() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    fs::write(stage.join("assets/export.js"), "globalThis.__slopCapture = json => new URL('x');").unwrap();
    refuses_view(app(json!({"title":"Fixture"})), &stage, &dir.path().join("Broken.slop"), "Export", "URL");
}

#[test]
fn pack_checks_the_pdf_export_as_well_as_png() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let program = fs::read_to_string(stage.join("assets/export.js")).unwrap();
    fs::write(stage.join("assets/export.js"), format!("{program}\nconst render = __slopCapture; globalThis.__slopCapture = json => {{ if (JSON.parse(json).format === 'pdf') throw new Error('PDF failed'); return render(json); }};")).unwrap();
    refuses_view(app(json!({"title":"Fixture"})), &stage, &dir.path().join("Broken.slop"), "Export", "PDF failed");
}

#[test]
fn pack_refuses_a_widget_that_throws() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let mut app = app(json!({"title":"Fixture"}));
    widget(&mut app, &stage, "globalThis.__slopWidget = json => { throw new Error('Widget failed'); };");
    refuses_view(app, &stage, &dir.path().join("Broken.slop"), "Widget", "Widget failed");
}

#[test]
fn pack_checks_views_even_when_both_artwork_images_are_supplied() {
    for role in ["preview", "icon"] {
        let dir = tempfile::tempdir().unwrap();
        let stage = stage(dir.path());
        let mut app = app(json!({"title":"Fixture"}));
        for supplied in ["preview", "icon"] {
            supplied_artwork(&mut app, &stage, supplied);
        }
        fs::write(
            stage.join(format!("assets/{role}.js")),
            "globalThis.__slopCapture = json => { throw new Error('Supplied view failed'); };",
        )
        .unwrap();
        refuses_view(
            app,
            &stage,
            &dir.path().join("Broken.slop"),
            if role == "preview" { "Preview" } else { "Icon" },
            "Supplied view failed",
        );
    }
}

#[test]
fn pack_checks_all_views_and_stores_only_unsupplied_artwork() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let mut app = app(json!({"title":"Fixture"}));
    widget(
        &mut app,
        &stage,
        "globalThis.__slopWidget = json => { const r = JSON.parse(json); return JSON.stringify({fonts:[],frames:r.viewports.map(v => ({width:v.width,height:v.height,html:'<p>'+r.snapshot.title+'</p>',css:'p{color:#335577}'}))}); };",
    );
    supplied_artwork(&mut app, &stage, "icon");
    let supplied_only = dir.path().join("Supplied.slop");
    let without_render = request(json!({"method":"pack","stage":stage,"file":supplied_only,"app":app}));
    assert_eq!(without_render["artwork"], json!({"preview":"absent","icon":"supplied"}));
    let template = dir.path().join("Healthy.slop");
    let packed = request(json!({"method":"pack","stage":stage,"file":template,"app":app,"renderViews":true}));
    assert_eq!(packed["artwork"], json!({"preview":"rendered","icon":"supplied"}), "{packed}");
    let (_, stored) = hitslop_core::file::artwork(&template, &[hitslop_core::file::Artwork::Icon]).unwrap().unwrap();
    // Supplied artwork is optimized by pack, retaining its pixels.
    assert_eq!(
        stored,
        hitslop_core::file::artwork(&supplied_only, &[hitslop_core::file::Artwork::Icon]).unwrap().unwrap().1
    );
    assert!(hitslop_core::file::artwork(&template, &[hitslop_core::file::Artwork::Preview]).unwrap().is_some());
}

#[test]
fn pack_renders_bundled_font_text_into_its_artwork() {
    let dir = tempfile::tempdir().unwrap();
    // Geist, the bundled sans-serif, at its heaviest: an H whose crossbar covers the centre.
    let css = "p{margin:0;font-family:sans-serif;font-weight:900;font-size:50vh;line-height:1;color:#123456}";
    let template = fixture(dir.path(), "<p>H</p>", css);
    for (role, size) in [("preview", (640, 480)), ("icon", (512, 512))] {
        let ((width, height), pixel) = artwork(&template, role);
        assert_eq!((width, height), size, "{role}");
        assert_eq!(pixel(0, 0), WHITE, "{role}: the page");
        assert_eq!(pixel(width / 2, height / 2), [0x12, 0x34, 0x56, 255], "{role}: the H's crossbar");
        assert_eq!(pixel(width / 2, height / 2 - 40), WHITE, "{role}: between the H's stems");
    }
}

#[test]
fn pack_renders_an_inline_image_into_its_artwork() {
    let dir = tempfile::tempdir().unwrap();
    // Templates hold no attachments: their images are inlined, here a 2 × 2 PNG.
    let mut image = Vec::new();
    let mut encoder = png::Encoder::new(&mut image, 2, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&[0x0f, 0x80, 0x40, 255].repeat(4)).unwrap();
    let body = format!("<img src=\"data:image/png;base64,{}\">", data_encoding::BASE64.encode(&image));
    let template = fixture(dir.path(), &body, "img{display:block;width:50vw;height:50vh}");
    for (role, size) in [("preview", (640, 480)), ("icon", (512, 512))] {
        let ((width, height), pixel) = artwork(&template, role);
        assert_eq!((width, height), size, "{role}");
        assert_eq!(pixel(0, 0), WHITE, "{role}: the page");
        assert_eq!(pixel(width / 2, height / 2), [0x0f, 0x80, 0x40, 255], "{role}: the image");
        assert_eq!(pixel(width / 2, height / 4 - 4), WHITE, "{role}: above the image");
        assert_eq!(pixel(width / 2, height / 4 + 4), [0x0f, 0x80, 0x40, 255], "{role}: the image's top edge");
    }
}
