//! Native manifest acceptance over the TypeBox-generated contract.
use crate::{err, shape, Code, Result};

#[jsonschema::validator(
    path = "../../packages/schema/generated/manifest.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = false, validate = true, iter_errors = false }
)]
struct Manifest;

pub fn validate(input: &str) -> Result<shape::Silhouette> {
    if input.len() > 64 * 1024 {
        return Err(err(Code::TooLarge, "manifest.json exceeds 64 KiB"));
    }
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|_| err(Code::InvalidRequest, "manifest.json must be valid JSON"))?;
    Manifest::validate(&value).map_err(|e| {
        let path = e.instance_path().as_str();
        err(Code::InvalidRequest, format!("Invalid manifest.json at {}", if path.is_empty() { "/" } else { path }))
    })?;
    let presentation = &value["presentation"];
    // The validated schema guarantees both dimensions and restricts the presentation.
    let width = presentation["width"].as_f64().expect("validated width");
    let height = presentation["height"].as_f64().expect("validated height");
    shape::silhouette_value(presentation.get("shape"), width, height)
}
