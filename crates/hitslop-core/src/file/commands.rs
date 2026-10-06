//! Immutable authored commands. Only the command parent reads these reserved assets;
//! the page runs its compiled imports and cannot fetch a runner bundle via AssetReader.
use super::{opened, read_asset, reader};
use crate::error::{Result, invalid};
use serde_json::Value;
use std::path::Path;

pub const COMMAND_METADATA: &str = "__commands/metadata.json";
pub const COMMAND_BUNDLE: &str = "__commands/run.js";
#[jsonschema::validator(path = "../../packages/hitslop/generated/commands-format-1.schema.json", draft = Draft7)]
struct Metadata;
#[jsonschema::validator(path = "../../packages/hitslop/generated/command-call.schema.json", draft = Draft7)]
struct Call;
pub fn valid_call(value: &Value) -> bool {
    Call::is_valid(value)
}

pub(super) fn validate(metadata: Option<&[u8]>, bundle: Option<&[u8]>) -> Result<()> {
    match (metadata, bundle) {
        (None, None) => Ok(()),
        (Some(metadata), Some(bundle)) => {
            let metadata: Value = serde_json::from_slice(metadata).map_err(invalid)?;
            if !Metadata::is_valid(&metadata) {
                return Err(invalid("Invalid stored commands metadata"));
            }
            if bundle.is_empty() {
                return Err(invalid("Empty command bundle"));
            }
            std::str::from_utf8(bundle).map_err(invalid)?;
            Ok(())
        }
        _ => Err(invalid("Command metadata and bundle must be stored together")),
    }
}
pub struct CommandAssets {
    pub metadata: Value,
    pub bundle: String,
}
pub fn commands(path: &Path) -> Result<Option<CommandAssets>> {
    let conn = reader(path)?;
    let tx = conn.unchecked_transaction().map_err(invalid)?;
    opened(&tx, path, false)?;
    let Some(metadata) = read_asset(&tx, COMMAND_METADATA)? else { return Ok(None) };
    let bundle = read_asset(&tx, COMMAND_BUNDLE)?.ok_or_else(|| invalid("Missing command bundle"))?;
    Ok(Some(CommandAssets {
        metadata: serde_json::from_slice(&metadata).map_err(invalid)?,
        bundle: String::from_utf8(bundle).map_err(invalid)?,
    }))
}
