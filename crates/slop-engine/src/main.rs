//! `slop-engine`, the file engine `@hitslop/cli` runs. `pack <stage> <file>` builds a template
//! from a build's stage; `inspect <file>` prints a file's kind, markers, app and sizes as
//! JSON, and whether a live owner has published its socket (`live`; a crashed owner's stays
//! until the document next opens or the app's launch sweep); `schema <file>` prints its app's
//! document descriptor. A refusal prints its message on stderr and exits 1; a usage error
//! exits 2. `HITSLOP_TEST_REGISTRY` points the discovery read at a test run's registry; the
//! engine never takes a writer lock.
use hitslop_core::{file, registry};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Some(folder) = std::env::var_os("HITSLOP_TEST_REGISTRY").filter(|folder| !folder.is_empty()) {
        let _ = registry::use_folder(Path::new(&folder));
    }
    let result = match args.as_slice() {
        ["pack", stage, file] => file::pack(Path::new(stage), Path::new(file)).map(|()| None),
        ["inspect", file] => file::inspect(Path::new(file)).and_then(|mut value| {
            value["live"] = registry::discovery(Path::new(file))?.is_some().into();
            Ok(Some(value.to_string()))
        }),
        ["schema", file] => file::descriptor(Path::new(file)).map(Some),
        ["--build-id"] => Ok(Some(hitslop_core::BUILD_ID.to_owned())),
        _ => {
            eprintln!("usage: slop-engine pack <stage> <file> | inspect <file> | schema <file> | --build-id");
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
