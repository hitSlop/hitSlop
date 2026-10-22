//! Export recursively from contract roots; dependencies are discovered by ts-rs.
use hitslop_core::build::BuildInput;
use hitslop_core::{Batch, EngineReply, EngineRequest, OwnerState, Publication, ThemeFile};
use ts_rs::{Config, TS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(std::env::args_os().nth(1).ok_or("Expected output directory")?);
    let config = Config::new().with_out_dir(&directory).with_large_int("number");
    hitslop_core::bindings::Node::export_all(&config)?;
    hitslop_core::preview::PreviewRequest::export_all(&config)?;
    hitslop_core::browser_wire::BrowserRequest::export_all(&config)?;
    hitslop_core::browser_wire::BrowserEvent::export_all(&config)?;
    hitslop_sharing_protocol::http::CreateRoom::export_all(&config)?;
    hitslop_sharing_protocol::http::CreatedRoom::export_all(&config)?;
    hitslop_sharing_protocol::http::AccountRoom::export_all(&config)?;
    hitslop_sharing_protocol::http::RoomInfo::export_all(&config)?;
    hitslop_sharing_protocol::http::ResourceGrant::export_all(&config)?;
    hitslop_sharing_protocol::http::ApiFailure::export_all(&config)?;
    hitslop_sharing_protocol::http::ConnectionPlan::export_all(&config)?;
    hitslop_sharing_protocol::http::SharingControl::export_all(&config)?;
    hitslop_sharing_protocol::http::SharingEvent::export_all(&config)?;
    hitslop_core::sync::Pause::export_all(&config)?;
    hitslop_sharing_protocol::resources::Manifest::export_all(&config)?;
    EngineRequest::export_all(&config)?;
    EngineReply::export_all(&config)?;
    BuildInput::export_all(&config)?;
    hitslop_runner::Job::export_all(&config)?;
    hitslop_core::widgets::WidgetRenderRequest::export_all(&config)?;
    hitslop_core::capture::ViewRenderRequest::export_all(&config)?;
    Batch::export_all(&config)?;
    OwnerState::export_all(&config)?;
    Publication::export_all(&config)?;
    ThemeFile::export_all(&config)?;
    hitslop_core::page_wire::PageRequest::export_all(&config)?;
    hitslop_core::page_wire::PageSuccess::export_all(&config)?;
    hitslop_core::page_wire::HostLayout::export_all(&config)?;
    hitslop_core::page_wire::PagePush::<Publication>::export_all(&config)?;
    hitslop_core::page_wire::HostRequest::export_all(&config)?;
    hitslop_core::page_wire::ClosePreparation::export_all(&config)?;
    let mut constants = String::from("// Generated from hitslop-core's contracts. Do not edit.\n");
    for (name, value) in hitslop_core::bindings::constants() {
        constants.push_str(&format!("export const {name} = {} as const;\n", serde_json::to_string(&value)?));
    }
    constants.push_str(&format!("export const SharingHost = {} as const;\n",serde_json::to_string(&serde_json::json!({
        "origin":hitslop_sharing_protocol::TRUSTED_ORIGIN,"domain":hitslop_sharing_protocol::AUTHORED_DOMAIN,
        "socketProtocol":hitslop_sharing_protocol::SOCKET_PROTOCOL,"seedBytes":hitslop_sharing_protocol::SEED_BYTES,
        "queueBytes":hitslop_sharing_protocol::QUEUED_BYTES,"queueFrames":hitslop_sharing_protocol::QUEUED_FRAMES,
        "controlBytes":hitslop_sharing_protocol::CONTROL_BYTES,"resourceRenewMs":hitslop_sharing_protocol::RESOURCE_RENEW_MS,
        "hostPolicy":hitslop_sharing_protocol::host_policy(),"pagePolicy":hitslop_sharing_protocol::page_policy()
    }))?));
    std::fs::write(
        directory.join("sharing.openapi.json"),
        hitslop_sharing_protocol::http::openapi().to_pretty_json()?,
    )?;
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
