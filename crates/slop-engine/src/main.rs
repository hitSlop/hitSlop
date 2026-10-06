//! `slop-engine`, the file engine `hitslop` runs. `validate-app` checks evaluated app
//! JSON from bounded standard input; `pack <stage> <file>` builds a template
//! from a build's stage; `inspect <file>` prints a file's kind, markers, app and sizes as
//! JSON, and whether a live owner has published its socket (`live`; a crashed owner's stays
//! until the document next opens or the app's launch sweep); `schema <file>` prints its app's
//! document descriptor. `templates` prints the templates the app's catalog lists, as JSON;
//! `create --from` takes one's slug or a template's path. `request` routes through the live
//! owner or acquires the writer lock and runs the same owner in-process; its classified
//! result is printed as JSON.
//! What needs AppKit or WebKit runs in the app's helper: the engine passes it an export
//! request unchanged (`hitslop-native export`), `screenshot` artwork and `open` in a window,
//! so the CLI talks to one binary. The helper serves no other document request. `request`,
//! `templates`, `create`, `open` and `screenshot` run in the command protocol their caller
//! names first (`--client-protocol N`). Other refusals print a message on stderr and exit 1;
//! a usage error exits 2.
//! `HITSLOP_TEST_REGISTRY` selects an isolated registry for tests.
use hitslop_core::{command, file, registry};
mod call;
mod runner;
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
/// Runs the helper in `protocol` with `args` and `input` on its standard input. Its output
/// and exit status are this command's.
fn native(protocol: u64, args: &[&str], input: Option<&str>) -> ExitCode {
    let run = || -> std::io::Result<Option<i32>> {
        let helper = helper().map_err(std::io::Error::other)?;
        let mut child = Command::new(helper)
            .arg("--client-protocol")
            .arg(protocol.to_string())
            .args(args)
            .stdin(if input.is_some() { Stdio::piped() } else { Stdio::inherit() })
            .spawn()?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(input.as_bytes())?;
        }
        Ok(child.wait()?.code())
    };
    match run() {
        Ok(code) => ExitCode::from(code.unwrap_or(1).clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn validate_app() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .take(file::APP_INPUT_BYTES as u64 + 1)
        .read_to_string(&mut input)
        .map_err(|error| error.to_string())?;
    file::validate_app(&input).map_err(|error| error.to_string())?;
    Ok(())
}

/// The protocol a command that reaches a document or the app's helper runs in, which its
/// caller must name; authoring commands run the CLI's own engine and need none.
fn named(protocol: Option<u64>) -> Result<u64, ExitCode> {
    protocol.ok_or_else(|| {
        eprintln!("Name the command protocol with --client-protocol N");
        ExitCode::from(2)
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut protocol = None;
    if args.as_slice() == ["--evaluate-command"] {
        runner::child();
        return ExitCode::SUCCESS;
    }
    if args.first() == Some(&"--client-protocol") {
        let Some(version) = args.get(1).and_then(|v| v.parse().ok()) else {
            eprintln!("Invalid client protocol");
            return ExitCode::from(2);
        };
        if let Some(message) = command::protocol_mismatch(version) {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
        protocol = Some(version);
        args.drain(..2);
    }
    if args.as_slice() == ["--protocol"] {
        println!("{}", command::protocol());
        return ExitCode::SUCCESS;
    }
    if let Some(folder) = std::env::var_os("HITSLOP_TEST_REGISTRY").filter(|folder| !folder.is_empty()) {
        let _ = registry::use_folder(Path::new(&folder));
    }
    if args.first() == Some(&"call") || args.first() == Some(&"describe") {
        let protocol = match named(protocol) {
            Ok(protocol) => protocol,
            Err(code) => return code,
        };
        let reply = match args.as_slice() {
            ["describe", path] => call::describe(path, protocol),
            ["call"] => {
                let mut input = String::new();
                if std::io::stdin().take(command::MAX_REQUEST_BYTES as u64 + 1).read_to_string(&mut input).is_err()
                    || input.len() > command::MAX_REQUEST_BYTES
                {
                    eprintln!("Command input is too large or invalid UTF-8");
                    return ExitCode::FAILURE;
                }
                call::call(&input, protocol)
            }
            _ => {
                eprintln!("usage: describe <file> | call < request.json");
                return ExitCode::from(2);
            }
        };
        println!("{reply}");
        return ExitCode::SUCCESS;
    }
    if args.as_slice() == ["request"] {
        let protocol = match named(protocol) {
            Ok(protocol) => protocol,
            Err(code) => return code,
        };
        let mut input = String::new();
        return match std::io::stdin().take(command::MAX_REQUEST_BYTES as u64 + 1).read_to_string(&mut input) {
            Ok(_) => {
                if command::is_export(&input) {
                    return native(protocol, &["export"], Some(&input));
                }
                println!("{}", command::request(&input, protocol, None));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    if matches!(args.first(), Some(&"open" | &"screenshot")) {
        return match named(protocol) {
            Ok(protocol) => native(protocol, &args, None),
            Err(code) => code,
        };
    }
    if args.as_slice() == ["validate-app"] {
        return match validate_app() {
            Ok(_) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    let result = match args.as_slice() {
        ["create", "--from", template, "--output", document] => {
            if let Err(code) = named(protocol) {
                return code;
            }
            let destination = Path::new(document);
            let created =
                file::template_source(template).and_then(|template| file::create_document(&template, destination));
            created.map(|()| {
                Some(
                    std::fs::canonicalize(destination)
                        .unwrap_or_else(|_| destination.to_owned())
                        .to_string_lossy()
                        .into_owned(),
                )
            })
        }
        ["templates"] => {
            if let Err(code) = named(protocol) {
                return code;
            }
            // A path that is not UTF-8 has no JSON spelling.
            serde_json::to_string_pretty(&file::list_templates(&file::template_roots()))
                .map(Some)
                .map_err(|error| hitslop_core::store::Error::Failed(error.to_string()))
        }
        ["pack", stage, file] => file::pack(Path::new(stage), Path::new(file)).map(|()| None),
        ["inspect", file] => file::inspect(Path::new(file)).and_then(|mut value| {
            value["live"] = registry::discovery(Path::new(file))?.is_some().into();
            Ok(Some(value.to_string()))
        }),
        ["schema", file] => file::descriptor(Path::new(file)).map(Some),
        ["--build-id"] => Ok(Some(hitslop_core::BUILD_ID.to_owned())),
        _ => {
            eprintln!(
                "usage: slop-engine validate-app < app.json | pack <stage> <file> | inspect <file> | schema <file> | --client-protocol N (request | templates | create --from <slug-or-template> --output <file> | open <file> | screenshot <file> --output <png> [--target preview|icon] [--if-present]) | --protocol | --build-id"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(output) => {
            if let Some(output) = output {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
