//! `slop-engine`, the file engine `@hitslop/cli` runs. `validate-app` checks evaluated app
//! JSON from bounded standard input; `pack <stage> <file>` builds a template
//! from a build's stage; `inspect <file>` prints a file's kind, markers, app and sizes as
//! JSON, and whether a live owner has published its socket (`live`; a crashed owner's stays
//! until the document next opens or the app's launch sweep); `schema <file>` prints its app's
//! document descriptor. `request` routes through the live owner or acquires the writer
//! lock and runs the same owner in-process; its classified result is printed as JSON.
//! Other refusals print a message on stderr and exit 1; a usage error exits 2.
//! `HITSLOP_TEST_REGISTRY` selects an isolated registry for tests.
use hitslop_core::{command, file, registry};
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

fn validate_app() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .take(file::APP_INPUT_BYTES as u64 + 1)
        .read_to_string(&mut input)
        .map_err(|error| error.to_string())?;
    file::validate_app(&input).map_err(|error| error.to_string())?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    if args.as_slice() == ["--protocol"] {
        println!("{}", command::protocol());
        return ExitCode::SUCCESS;
    }
    if args.first() == Some(&"--client-protocol") {
        let Some(version) = args.get(1).and_then(|v| v.parse().ok()) else {
            eprintln!("Invalid client protocol");
            return ExitCode::from(2);
        };
        if !command::supports_protocol(version) {
            eprintln!("Unsupported client protocol; update hitSlop and @hitslop/cli");
            return ExitCode::from(2);
        }
        args.drain(..2);
    }
    if let Some(folder) =
        std::env::var_os("HITSLOP_TEST_REGISTRY").filter(|folder| !folder.is_empty())
    {
        let _ = registry::use_folder(Path::new(&folder));
    }
    if args.as_slice() == ["request"] {
        let mut input = String::new();
        return match std::io::stdin()
            .take(command::MAX_REQUEST_BYTES as u64 + 1)
            .read_to_string(&mut input)
        {
            Ok(_) => {
                println!("{}", command::request(&input, None));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
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
            let destination = Path::new(document);
            let parent = destination
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            match std::fs::create_dir_all(parent) {
                Ok(()) => file::create_document(Path::new(template), destination).map(|()| {
                    Some(
                        std::fs::canonicalize(destination)
                            .unwrap_or_else(|_| destination.to_owned())
                            .to_string_lossy()
                            .into_owned(),
                    )
                }),
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            }
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
                "usage: slop-engine validate-app < app.json | pack <stage> <file> | inspect <file> | schema <file> | request | create --from <template> --output <file> | --protocol | --build-id"
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
