use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args == ["limits"] {
        println!("{}", serde_json::to_string(&pop_contracts::wire::open_limits()).expect("limits encode"));
        return ExitCode::SUCCESS;
    }
    if args.len() == 2 && args[0] == "acceptance-vectors" {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let vectors = pop_compat::acceptance_vectors()?;
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
    eprintln!("Use pop-compat limits or pop-compat acceptance-vectors DIR");
    ExitCode::from(2)
}
