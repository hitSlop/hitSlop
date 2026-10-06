//! The private engine boundary: frozen protocol preflight on argv, one TypeBox JSON
//! request on bounded stdin, and one classified JSON reply on stdout. The core routes
//! document operations to the live owner or acquires its lock and runs the same owner.
//! AppKit operations forward the original JSON to the native helper. Only build/protocol
//! queries and the exact restricted evaluator entry point are outside the JSON wire.
use hitslop_core::{
    EngineRequest, EngineSuccess, command,
    envelope::{self, Envelope},
    file, registry,
};
mod call;
mod runner;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// The app's helper. `HITSLOP_NATIVE_CLI` names one; otherwise the installed app's.
fn helper() -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        return Err("Opening windows, exporting and native artwork require macOS and hitSlop.app; document edits and authoring run anywhere.".into());
    }
    let executable =
        |path: &Path| std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
    if let Some(named) = std::env::var_os("HITSLOP_NATIVE_CLI").filter(|named| !named.is_empty()) {
        let named = PathBuf::from(named);
        return if executable(&named) { Ok(named) } else { Err("HITSLOP_NATIVE_CLI is not executable".into()) };
    }
    let home = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Applications/hitSlop.app/Contents/Helpers/hitslop-native"));
    [PathBuf::from("/Applications/hitSlop.app/Contents/Helpers/hitslop-native")]
        .into_iter()
        .chain(home)
        .find(|path| executable(path))
        .ok_or_else(|| {
            "Install hitSlop.app in /Applications or ~/Applications to open windows, export or render artwork".into()
        })
}

fn rejected(reason: &str, error: impl std::fmt::Display) -> String {
    json!({"ok":false,"code":"rejected","reason":reason,"error":error.to_string()}).to_string()
}
fn unknown(error: impl std::fmt::Display) -> String {
    json!({"ok":false,"code":"unknown_outcome","error":error.to_string()}).to_string()
}
fn success(result: EngineSuccess) -> String {
    #[derive(serde::Serialize)]
    struct Reply {
        ok: bool,
        #[serde(flatten)]
        result: EngineSuccess,
    }
    serde_json::to_string(&Reply { ok: true, result }).expect("serializable reply")
}
fn raw(value: impl serde::Serialize) -> Box<serde_json::value::RawValue> {
    serde_json::value::to_raw_value(&value).expect("serializable result")
}
fn native(protocol: u64, method: &str, input: &str) -> String {
    let path = match helper() {
        Ok(path) => path,
        Err(error) => return rejected("invalid_request", error),
    };
    let mut child = match Command::new(path)
        .args(["--client-protocol", &protocol.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return rejected("invalid_request", error),
    };
    let sent = child.stdin.take().expect("piped stdin").write_all(input.as_bytes());
    // Always reap, including an early refusal that closed stdin before the write.
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => return unknown(error),
    };
    if output.status.code() == Some(2) {
        return rejected("requires_update", String::from_utf8_lossy(&output.stderr).trim());
    }
    if !output.status.success() || sent.is_err() || !envelope::is_valid(Envelope::NativeReply, &output.stdout) {
        return unknown("Native helper stopped without a valid reply; inspect the document and output before retrying");
    }
    let reply: Value = serde_json::from_slice(&output.stdout).expect("validated reply");
    if reply["ok"] == true && reply["method"] != method {
        return unknown("Native helper replied to another method; inspect the document and output before retrying");
    }
    String::from_utf8(output.stdout).expect("validated JSON")
}
fn dispatch(request: EngineRequest) -> String {
    let result = (|| -> Result<EngineSuccess, hitslop_core::store::Error> {
        Ok(match request {
            EngineRequest::Templates {} => {
                EngineSuccess::Templates { catalog: raw(file::list_templates(&file::template_roots())) }
            }
            EngineRequest::Create { from, output } => {
                file::create_document(&file::template_source(&from)?, Path::new(&output))?;
                let path = std::fs::canonicalize(&output).unwrap_or(PathBuf::from(output));
                EngineSuccess::Create { documentPath: path.to_string_lossy().into_owned() }
            }
            EngineRequest::Pack { stage, file: path } => {
                file::pack(Path::new(&stage), Path::new(&path))?;
                EngineSuccess::Pack {}
            }
            EngineRequest::ValidateApp { app } => {
                file::validate_app(app.get())?;
                EngineSuccess::ValidateApp {}
            }
            EngineRequest::Inspect { file: path } => {
                let path = Path::new(&path);
                let mut info = file::inspect(path)?;
                info["live"] = registry::discovery(path)?.is_some().into();
                EngineSuccess::Inspect { info: raw(info) }
            }
            EngineRequest::Schema { file: path } => EngineSuccess::Schema {
                schema: serde_json::value::RawValue::from_string(file::descriptor(Path::new(&path))?)
                    .expect("valid descriptor"),
            },
            _ => unreachable!("routed before file dispatch"),
        })
    })();
    match result {
        Ok(result) => success(result),
        Err(error) => command::failure(error.into(), false, false),
    }
}
fn request(input: &str, protocol: u64) -> String {
    if input.len() > command::MAX_REQUEST_BYTES {
        return rejected("too_large", "Engine request is too large");
    }
    let request = match EngineRequest::parse(input) {
        Ok(request) => request,
        Err(error) => return rejected("invalid_request", error),
    };
    // The generated decoder checks the closed envelope without interpreting RawValue.
    // validateApp has no other fields to constrain: its core checks the markers before
    // reading the app, even when a future payload contains numbers this JSON DOM cannot hold.
    if !matches!(request, EngineRequest::ValidateApp { .. })
        && !envelope::is_valid(Envelope::EngineRequest, input.as_bytes())
    {
        return rejected("invalid_request", "Invalid engine request");
    }
    match request {
        EngineRequest::Open { .. } | EngineRequest::Screenshot { .. } | EngineRequest::Export { .. } => {
            native(protocol, request.method(), input)
        }
        EngineRequest::Get { .. }
        | EngineRequest::Batch { .. }
        | EngineRequest::ThemeExport { .. }
        | EngineRequest::AttachmentsList { .. }
        | EngineRequest::AttachmentsRead { .. } => command::request(input, protocol, None),
        EngineRequest::Describe { documentPath } => call::describe(&documentPath, protocol).to_string(),
        EngineRequest::Call { documentPath, command, args } => {
            call::call(&json!({"documentPath":documentPath,"command":command,"args":args}).to_string(), protocol)
                .to_string()
        }
        _ => dispatch(request),
    }
}
fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--evaluate-command" {
        runner::child();
        return ExitCode::SUCCESS;
    }
    let mut args = args.into_iter().peekable();
    let mut protocol = None;
    // Frozen: first argument, checked before all other arguments, stdin and file access.
    if args.peek().is_some_and(|arg| arg == "--client-protocol") {
        args.next();
        let Some(version) = args.next().and_then(|v| v.to_str().and_then(|v| v.parse().ok())) else {
            eprintln!("Invalid client protocol");
            return ExitCode::from(2);
        };
        if let Some(message) = command::protocol_mismatch(version) {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
        protocol = Some(version);
    }
    let args: Vec<_> = args.collect();
    if args == ["--protocol"] {
        println!("{}", command::protocol());
        return ExitCode::SUCCESS;
    }
    if args == ["--build-id"] {
        println!("{}", hitslop_core::BUILD_ID);
        return ExitCode::SUCCESS;
    }
    let Some(protocol) = protocol.filter(|_| args.is_empty()) else {
        eprintln!("Use --client-protocol N and one JSON request on stdin");
        return ExitCode::from(2);
    };
    if let Some(folder) = std::env::var_os("HITSLOP_TEST_REGISTRY").filter(|folder| !folder.is_empty()) {
        let _ = registry::use_folder(Path::new(&folder));
    }
    let mut input = String::new();
    let reply = match std::io::stdin().take(command::MAX_REQUEST_BYTES as u64 + 1).read_to_string(&mut input) {
        Ok(_) => request(&input, protocol),
        Err(error) => rejected("invalid_request", error),
    };
    println!("{}", reply.trim_end());
    ExitCode::SUCCESS
}
