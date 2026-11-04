//! Export recursively from contract roots; dependencies are discovered by ts-rs.
use pop_core::build::BuildInput;
use pop_core::{Batch, EngineReply, EngineRequest, OwnerState, Publication, ThemeFile};
use ts_rs::{Config, TS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(std::env::args_os().nth(1).ok_or("Expected output directory")?);
    let config = Config::new().with_out_dir(&directory).with_large_int("number");
    pop_core::bindings::Node::export_all(&config)?;
    pop_core::preview::PreviewRequest::export_all(&config)?;
    pop_core::browser_wire::BrowserRequest::export_all(&config)?;
    pop_core::browser_wire::BrowserEvent::export_all(&config)?;
    pop_sharing_protocol::http::CreateRoom::export_all(&config)?;
    pop_sharing_protocol::http::CreatedRoom::export_all(&config)?;
    pop_sharing_protocol::http::RoomInfo::export_all(&config)?;
    pop_sharing_protocol::document::DocumentEntry::export_all(&config)?;
    pop_sharing_protocol::document::RegisteredDocument::export_all(&config)?;
    pop_sharing_protocol::document::RegisteredDocuments::export_all(&config)?;
    pop_sharing_protocol::document::DocumentMetadata::export_all(&config)?;

    pop_sharing_protocol::document::AccessState::export_all(&config)?;
    pop_sharing_protocol::document::UpdateDocument::export_all(&config)?;
    pop_sharing_protocol::document::JoinDocument::export_all(&config)?;

    pop_sharing_protocol::lifecycle::EpochState::export_all(&config)?;
    pop_sharing_protocol::lifecycle::PublishEpoch::export_all(&config)?;
    pop_sharing_protocol::lifecycle::StagedSeed::export_all(&config)?;
    pop_sharing_protocol::http::ResourceGrant::export_all(&config)?;
    pop_sharing_protocol::http::ApiFailure::export_all(&config)?;
    pop_sharing_protocol::http::ConnectionPlan::export_all(&config)?;
    pop_sharing_protocol::http::SharingControl::export_all(&config)?;
    pop_sharing_protocol::http::SharingEvent::export_all(&config)?;
    pop_core::sync::Pause::export_all(&config)?;
    pop_core::sync::SyncState::export_all(&config)?;
    pop_core::sync::Confirmation::export_all(&config)?;
    pop_sharing_protocol::failure::Failure::export_all(&config)?;
    pop_sharing_protocol::maintenance::FullState::export_all(&config)?;
    pop_sharing_protocol::maintenance::Step::export_all(&config)?;
    pop_sharing_protocol::maintenance::MaintenanceAction::export_all(&config)?;
    pop_sharing_protocol::resources::Manifest::export_all(&config)?;
    EngineRequest::export_all(&config)?;
    EngineReply::export_all(&config)?;
    BuildInput::export_all(&config)?;
    pop_contracts::evaluator::Job::export_all(&config)?;
    pop_core::widgets::WidgetRenderRequest::export_all(&config)?;
    pop_core::capture::ViewRenderRequest::export_all(&config)?;
    Batch::export_all(&config)?;
    OwnerState::export_all(&config)?;
    Publication::export_all(&config)?;
    ThemeFile::export_all(&config)?;
    pop_core::page_wire::PageRequest::export_all(&config)?;
    pop_core::page_wire::PageSuccess::export_all(&config)?;
    pop_core::page_wire::HostLayout::export_all(&config)?;
    pop_core::page_wire::PagePush::<Publication>::export_all(&config)?;
    pop_core::page_wire::HostRequest::export_all(&config)?;
    pop_core::page_wire::ClosePreparation::export_all(&config)?;
    let mut constants = String::from("// Generated from pop-core's contracts. Do not edit.\n");
    for (name, value) in pop_core::bindings::constants() {
        constants.push_str(&format!("export const {name} = {} as const;\n", serde_json::to_string(&value)?));
    }
    constants.push_str(&format!("export const SharingHost = {} as const;\n",serde_json::to_string(&serde_json::json!({
        "origin":pop_sharing_protocol::TRUSTED_ORIGIN,"domain":pop_sharing_protocol::AUTHORED_DOMAIN,
        "socketProtocol":pop_sharing_protocol::SOCKET_PROTOCOL,"seedBytes":pop_sharing_protocol::SEED_BYTES,
        "queueBytes":pop_sharing_protocol::QUEUED_BYTES,"queueFrames":pop_sharing_protocol::QUEUED_FRAMES,
        "controlBytes":pop_sharing_protocol::CONTROL_BYTES,"resourceRenewMs":pop_sharing_protocol::RESOURCE_RENEW_MS,
        "recheckMs":pop_sharing_protocol::limits::ROOM_RECHECK_MS,"compactJitterMs":pop_sharing_protocol::limits::COMPACT_JITTER_MS,
        "compactRetryMs":pop_sharing_protocol::limits::COMPACT_RETRY_MS,"pingMs":pop_sharing_protocol::limits::PING_MS,
        "backoffMinMs":pop_sharing_protocol::limits::BACKOFF_MIN_MS,"backoffMaxMs":pop_sharing_protocol::limits::BACKOFF_MAX_MS,
        "hostPolicy":pop_sharing_protocol::host_policy(),"pagePolicy":pop_sharing_protocol::page_policy()
    }))?));
    std::fs::write(directory.join("sharing.openapi.json"), pop_sharing_protocol::http::openapi().to_pretty_json()?)?;
    std::fs::write(directory.join("constants.generated.ts"), constants)?;
    // ts-rs exports types. Small value registries come from their Rust data directly.
    std::fs::write(
        directory.join("media.generated.ts"),
        format!(
            "// Generated from pop-core's media registry. Do not edit.\nexport const MediaTypes: Readonly<Record<string, readonly [string, string]>> = {};\n",
            serde_json::to_string_pretty(&pop_contracts::media::build_registry())?,
        ),
    )?;
    Ok(())
}
