//! The private engine boundary: frozen protocol preflight on argv, one Rust-owned JSON
//! request on bounded stdin, and one classified JSON reply on stdout. The core routes
//! document operations to the live owner or acquires its lock and runs the same owner.
//! AppKit operations forward the original JSON to the native helper. Only build/protocol
//! queries and the exact restricted evaluator entry point are outside the JSON wire.
use hitslop_core::owner::{Failure, FailureKind};
use hitslop_core::{Code, EngineRequest, EngineSuccess, command, engine::True, file, native::NativeReply, registry};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

mod preview;

/// A classified refusal, as every engine reply spells one.
fn rejected(reason: Code, error: impl std::fmt::Display) -> String {
    command::failure(
        Failure { kind: FailureKind::Rejected, message: error.to_string(), reason: Some(reason), op_index: None },
        false,
        false,
    )
}
/// The restricted evaluator: this same executable, run with `--evaluate-command`.
fn evaluator() -> Result<hitslop_runner::Evaluator, String> {
    hitslop_runner::Evaluator::new(
        std::env::current_exe().map_err(|e| e.to_string())?,
        vec!["--evaluate-command".into()],
    )
}

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

fn unknown(error: impl std::fmt::Display) -> String {
    command::failure(hitslop_core::store::Error::Failed(error.to_string()).into(), false, false)
}
fn success(result: EngineSuccess) -> String {
    serde_json::to_string(&result).expect("serializable reply")
}
fn native(protocol: u64, method: &str, input: &str) -> String {
    let path = match helper() {
        Ok(path) => path,
        Err(error) => return rejected(Code::InvalidRequest, error),
    };
    let mut child = match Command::new(path)
        .args(["--client-protocol", &protocol.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return rejected(Code::InvalidRequest, error),
    };
    let sent = child.stdin.take().expect("piped stdin").write_all(input.as_bytes());
    // Always reap, including an early refusal that closed stdin before the write.
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => return unknown(error),
    };
    if output.status.code() == Some(2) {
        return rejected(Code::RequiresUpdate, String::from_utf8_lossy(&output.stderr).trim());
    }
    const NO_REPLY: &str =
        "Native helper stopped without a valid reply; inspect the document and output before retrying";
    if !output.status.success() || sent.is_err() {
        return unknown(NO_REPLY);
    }
    let Ok(reply) = serde_json::from_slice::<NativeReply>(&output.stdout) else {
        return unknown(NO_REPLY);
    };
    if reply.method().is_some_and(|received| received != method) {
        return unknown("Native helper replied to another method; inspect the document and output before retrying");
    }
    String::from_utf8(output.stdout).expect("validated JSON")
}
fn dispatch(request: EngineRequest) -> String {
    let result = (|| -> Result<EngineSuccess, hitslop_core::store::Error> {
        Ok(match request {
            EngineRequest::Templates {} => {
                EngineSuccess::Templates { ok: True, catalog: file::list_templates(&file::template_roots()) }
            }
            EngineRequest::Create { from, output } => {
                file::create_document(&file::template_source(&from)?, Path::new(&output))?;
                let path = std::fs::canonicalize(&output).unwrap_or(PathBuf::from(output));
                EngineSuccess::Create { ok: True, document_path: path.to_string_lossy().into_owned() }
            }
            EngineRequest::Pack { stage, file: path, app } => {
                file::pack(app.get(), Path::new(&stage), Path::new(&path))?;
                EngineSuccess::Pack { ok: True }
            }
            EngineRequest::ValidateMetadata { metadata } => {
                hitslop_core::app::validate_metadata(&metadata).map_err(hitslop_core::store::Error::Rejected)?;
                EngineSuccess::ValidateMetadata { ok: True }
            }
            EngineRequest::ValidateApp { app, stage } => {
                file::validate_app(app.get(), Path::new(&stage))?;
                EngineSuccess::ValidateApp { ok: True }
            }
            EngineRequest::ArtworkExport { file: path, target, output } => {
                let name = match target {
                    hitslop_core::engine::ArtworkTarget::Icon => file::Artwork::Icon,
                    hitslop_core::engine::ArtworkTarget::Preview => file::Artwork::Preview,
                };
                let present = file::export_artwork(Path::new(&path), name, Path::new(&output))?;
                EngineSuccess::ArtworkExport { ok: True, output: present.then_some(output) }
            }
            EngineRequest::Inspect { file: path } => {
                let path = Path::new(&path);
                EngineSuccess::Inspect { ok: True, info: file::inspect(path)? }
            }
            EngineRequest::Schema { file: path } => EngineSuccess::Schema {
                ok: True,
                schema: serde_json::from_str(&file::descriptor(Path::new(&path))?).expect("valid descriptor"),
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
        return rejected(Code::TooLarge, "Engine request is too large");
    }
    let request = match EngineRequest::parse(input) {
        Ok(request) => request,
        Err(error) => return rejected(Code::InvalidRequest, error),
    };
    match request {
        EngineRequest::Open { .. } | EngineRequest::Screenshot { .. } | EngineRequest::Export { .. } => {
            native(protocol, request.method(), input)
        }
        EngineRequest::Get { .. }
        | EngineRequest::Batch { .. }
        | EngineRequest::Copy { .. }
        | EngineRequest::ThemeExport { .. }
        | EngineRequest::AttachmentsList { .. }
        | EngineRequest::AttachmentsRead { .. }
        | EngineRequest::Describe { .. }
        | EngineRequest::Call { .. } => command::request_with_evaluator(request, protocol, None, evaluator().ok()),
        _ => dispatch(request),
    }
}
fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--evaluate-command" {
        hitslop_runner::child();
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
    if let Some(folder) = std::env::var_os("HITSLOP_TEST_REGISTRY").filter(|folder| !folder.is_empty()) {
        let _ = registry::use_folder(Path::new(&folder));
    }
    if protocol.is_some() && args.len() == 2 && args[0] == "--preview-owner" {
        return preview::serve(Path::new(&args[1]));
    }
    let Some(protocol) = protocol.filter(|_| args.is_empty()) else {
        eprintln!("Use --client-protocol N and one JSON request on stdin");
        return ExitCode::from(2);
    };
    let mut input = String::new();
    let reply = match std::io::stdin().take(command::MAX_REQUEST_BYTES as u64 + 1).read_to_string(&mut input) {
        Ok(_) => request(&input, protocol),
        Err(error) => rejected(Code::InvalidRequest, error),
    };
    println!("{}", reply.trim_end());
    ExitCode::SUCCESS
}
