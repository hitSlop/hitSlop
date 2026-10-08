//! Export recursively from contract roots; dependencies are discovered by ts-rs.
use hitslop_core::build::BuildInput;
use hitslop_core::{Batch, EngineReply, EngineRequest, OwnerState, Publication, ThemeFile};
use ts_rs::{Config, TS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(std::env::args_os().nth(1).ok_or("Expected output directory")?);
    let config = Config::new().with_out_dir(&directory).with_large_int("number");
    hitslop_core::preview::PreviewRequest::export_all(&config)?;
    EngineRequest::export_all(&config)?;
    EngineReply::export_all(&config)?;
    BuildInput::export_all(&config)?;
    Batch::export_all(&config)?;
    OwnerState::export_all(&config)?;
    Publication::export_all(&config)?;
    ThemeFile::export_all(&config)?;
    hitslop_core::socket_wire::SocketRequest::export_all(&config)?;
    hitslop_core::socket_wire::Discovery::export_all(&config)?;
    hitslop_core::page_wire::PageRequest::export_all(&config)?;
    hitslop_core::page_wire::PageSuccess::export_all(&config)?;
    hitslop_core::page_wire::PagePush::export_all(&config)?;
    hitslop_core::page_wire::HostRequest::export_all(&config)?;
    hitslop_core::page_wire::HostCaptureResult::export_all(&config)?;
    let mut constants = String::from("// Generated from hitslop-core's contracts. Do not edit.\n");
    for (name, value) in hitslop_core::bindings::constants() {
        constants.push_str(&format!("export const {name} = {} as const;\n", serde_json::to_string(&value)?));
    }
    std::fs::write(directory.join("constants.generated.ts"), constants)?;
    // ts-rs exports types. Small value registries come from their Rust data directly.
    std::fs::write(
        directory.join("media.generated.ts"),
        format!(
            "// Generated from hitslop-core's media registry. Do not edit.\nexport const MediaTypes: Readonly<Record<string, readonly [string, string]>> = {};\n",
            serde_json::to_string_pretty(&hitslop_core::media::build_registry())?,
        ),
    )?;
    Ok(())
}
