//! The private engine boundary: frozen protocol preflight on argv, one Rust-owned JSON
//! request on bounded stdin, and one classified JSON reply on stdout. The core routes
//! document operations to the live owner or acquires its lock and runs the same owner; it
//! renders a closed document's export in this executable's restricted evaluator. Only
//! opening a window needs hitSlop.app. Only build/protocol queries and the exact restricted
//! evaluator entry point are outside the JSON wire.
use hitslop_core::owner::{Failure, FailureKind};
use hitslop_core::{Code, EngineRequest, EngineSuccess, command, engine::True, file, registry, store};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod attachments;
mod open;
mod preview;

/// A classified refusal, as every engine reply spells one.
fn rejected(reason: Code, error: impl std::fmt::Display) -> String {
    command::failure(
        Failure { kind: FailureKind::Rejected, message: error.to_string(), reason: Some(reason), op_index: None },
        false,
        false,
    )
}
/// A file operation's refusal: nothing changed.
fn refusal(code: Code, message: impl Into<String>) -> store::Error {
    store::Error::Rejected(hitslop_core::Error { code, message: message.into(), op_index: None, path: vec![] })
}
/// The restricted evaluator: this same executable, run with `--evaluate-command`.
fn evaluator() -> Result<hitslop_runner::Evaluator, String> {
    hitslop_runner::Evaluator::new(
        std::env::current_exe().map_err(|e| e.to_string())?,
        vec!["--evaluate-command".into()],
    )
}

fn success(result: EngineSuccess) -> String {
    serde_json::to_string(&result).expect("serializable reply")
}
fn dispatch(request: EngineRequest) -> String {
    let result = (|| -> Result<EngineSuccess, store::Error> {
        Ok(match request {
            EngineRequest::AttachmentRef { file } => {
                EngineSuccess::AttachmentRef { ok: True, reference: attachments::reference(Path::new(&file))? }
            }
            EngineRequest::Templates {} => {
                EngineSuccess::Templates { ok: True, catalog: file::list_templates(&file::template_roots()) }
            }
            EngineRequest::Create { from, output } => {
                file::create_document(&file::template_source(&from)?, Path::new(&output))?;
                let path = std::fs::canonicalize(&output).unwrap_or(PathBuf::from(output));
                EngineSuccess::Create { ok: True, document_path: path.to_string_lossy().into_owned() }
            }
            EngineRequest::Pack { stage, file: path, app, render_views } => {
                let (stage, path) = (Path::new(&stage), Path::new(&path));
                let artwork = if render_views {
                    let evaluator = evaluator().map_err(|e| {
                        refusal(Code::InvalidRequest, format!("Cannot start the capture renderer: {e}"))
                    })?;
                    let render = |template: &Path, roles: &[file::Artwork]| {
                        hitslop_core::capture::render_views(template, &evaluator, roles)
                    };
                    file::pack(app.get(), stage, path, Some(&render))?
                } else {
                    file::pack(app.get(), stage, path, None)?
                };
                EngineSuccess::Pack { ok: True, artwork }
            }
            EngineRequest::ValidateMetadata { metadata } => {
                hitslop_core::app::validate_metadata(&metadata).map_err(store::Error::Rejected)?;
                EngineSuccess::ValidateMetadata { ok: True }
            }
            EngineRequest::ValidateApp { app, stage } => {
                let (window, capabilities) = file::validate_app(app.get(), Path::new(&stage))?;
                EngineSuccess::ValidateApp { ok: True, window, capabilities }
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
            EngineRequest::Open { document_path } => {
                EngineSuccess::Open { ok: True, document_path: open::document(Path::new(&document_path))? }
            }
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
    match request.route() {
        hitslop_core::engine::Route::Document => command::request_with_evaluator(request, protocol, evaluator().ok()),
        hitslop_core::engine::Route::File => dispatch(request),
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
    if args == ["--limits"] {
        println!("{}", serde_json::to_string(&hitslop_core::app::AppDefinition::open_limits()).expect("limits encode"));
        return ExitCode::SUCCESS;
    }
    if args.len() == 2 && args[0] == "--acceptance-vectors" {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let vectors = hitslop_core::app::AppDefinition::acceptance_vectors()?;
            let folder = Path::new(&args[1]);
            std::fs::create_dir_all(folder)?;
            for (name, vector) in vectors {
                std::fs::write(folder.join(format!("{name}.json")), serde_json::to_vec_pretty(&vector)?)?;
            }
            Ok(())
        })();
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Acceptance vectors: {error}");
                ExitCode::FAILURE
            }
        };
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
