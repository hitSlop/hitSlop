//! The private engine boundary: frozen protocol preflight on argv, one Rust-owned JSON
//! request on bounded stdin, and one classified JSON reply on stdout. The core routes
//! document operations to the live owner or acquires its lock and runs the same owner; it
//! renders a closed document's export in this executable's restricted evaluator. Only
//! opening a window needs dotpop.app. Only build/protocol queries and the exact restricted
//! evaluator entry point are outside the JSON wire.
use pop_contracts::wire::Code;
use pop_contracts::wire::engine::{EngineRequest, EngineSuccess, FileRequest, True};
use pop_runtime::command;
use pop_runtime::owner::{Failure, FailureKind};
use pop_storage::{file, registry, store};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod attachments;
mod open;
mod preview;

/// A classified refusal, as every engine reply spells one.
fn rejected(reason: Code, error: impl std::fmt::Display) -> pop_contracts::wire::engine::Failure {
    command::classified_failure(
        Failure {
            kind: FailureKind::Rejected,
            message: error.to_string(),
            reason: Some(reason),
            op_index: None,
            path: vec![],
        },
        false,
        false,
    )
}
/// A file operation's refusal: nothing changed.
fn refusal(code: Code, message: impl Into<String>) -> store::Error {
    store::Error::Rejected(pop_contracts::Error { code, message: message.into(), op_index: None, path: vec![] })
}
/// The restricted evaluator: this same executable, run with `--evaluate-command`.
fn evaluator() -> Result<pop_runner::Evaluator, String> {
    pop_runner::Evaluator::process(
        std::env::current_exe().map_err(|e| e.to_string())?,
        vec!["--evaluate-command".into()],
    )
    .map_err(|error| error.to_string())
}

fn dispatch(request: FileRequest) -> command::DocumentResult {
    let result = (|| -> Result<EngineSuccess, store::Error> {
        Ok(match request {
            FileRequest::AttachmentRef { file } => {
                EngineSuccess::AttachmentRef { ok: True, reference: attachments::reference(Path::new(&file))? }
            }
            FileRequest::Templates {} => {
                EngineSuccess::Templates { ok: True, catalog: file::list_templates(&file::template_roots()) }
            }
            FileRequest::Create { from, output } => {
                file::create_document(&file::template_source(&from)?, Path::new(&output))?;
                let path = std::fs::canonicalize(&output).unwrap_or(PathBuf::from(output));
                EngineSuccess::Create { ok: True, document_path: path.to_string_lossy().into_owned() }
            }
            FileRequest::Pack { stage, file: path, app, render_views } => {
                let (stage, path) = (Path::new(&stage), Path::new(&path));
                let artwork = if render_views {
                    let evaluator = evaluator().map_err(|e| {
                        refusal(Code::InvalidRequest, format!("Cannot start the capture renderer: {e}"))
                    })?;
                    let render = |template: &Path, roles: &[file::Artwork]| {
                        pop_runtime::capture::render_views(template, &evaluator, roles)
                    };
                    file::pack(app.get(), stage, path, Some(&render))?
                } else {
                    file::pack(app.get(), stage, path, None)?
                };
                EngineSuccess::Pack { ok: True, artwork }
            }
            FileRequest::ValidateMetadata { metadata } => {
                pop_core::app::validate_metadata(&metadata).map_err(store::Error::Rejected)?;
                EngineSuccess::ValidateMetadata { ok: True }
            }
            FileRequest::ValidateApp { app, stage } => {
                let (window, capabilities) = file::validate_app(app.get(), Path::new(&stage))?;
                EngineSuccess::ValidateApp { ok: True, window, capabilities }
            }
            FileRequest::ArtworkExport { file: path, target, output } => {
                let name = match target {
                    pop_contracts::wire::engine::ArtworkTarget::Icon => file::Artwork::Icon,
                    pop_contracts::wire::engine::ArtworkTarget::Preview => file::Artwork::Preview,
                };
                let present = file::export_artwork(Path::new(&path), name, Path::new(&output))?;
                EngineSuccess::ArtworkExport { ok: True, output: present.then_some(output) }
            }
            FileRequest::Inspect { file: path } => {
                let path = Path::new(&path);
                EngineSuccess::Inspect { ok: True, info: file::inspect(path)? }
            }
            FileRequest::Schema { file: path } => EngineSuccess::Schema {
                ok: True,
                schema: serde_json::from_str(&file::descriptor(Path::new(&path))?).expect("valid descriptor"),
            },
            FileRequest::Open { document_path } => {
                EngineSuccess::Open { ok: True, document_path: open::document(Path::new(&document_path))? }
            }
        })
    })();
    result.map_err(|error| command::classified_failure(error.into(), false, false))
}
fn request(input: &str, protocol: u64) -> command::CommandReply {
    let request = match EngineRequest::parse(input) {
        Ok(request) => request,
        Err(error) => return Err(command::classified_failure(error.into(), false, false)).into(),
    };
    match request {
        EngineRequest::Document(request) => command::request_with_evaluator(request, protocol, evaluator().ok()),
        EngineRequest::File(request) => dispatch(request).into(),
    }
}
fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--evaluate-command" {
        pop_runner::child();
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
        if let Some(message) = pop_contracts::wire::socket::protocol_mismatch(version) {
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
        println!("{}", pop_runtime::BUILD_ID);
        return ExitCode::SUCCESS;
    }
    if let Some(folder) = std::env::var_os("POP_TEST_REGISTRY").filter(|folder| !folder.is_empty()) {
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
        Err(error) => Err(rejected(Code::InvalidRequest, error)).into(),
    };
    println!("{}", command::encode_reply(reply));
    ExitCode::SUCCESS
}
