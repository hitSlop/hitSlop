//! Native acceptance of a built package's manifest over the TypeBox-generated contract.
use crate::{err, shape, Code, Result, PACKAGE_FORMAT, RUNTIME_ABI};

#[jsonschema::validator(
    path = "../../packages/schema/generated/package-manifest.schema.json",
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
    // A newer package is refused before its other fields are judged: it may use fields
    // this build does not know.
    for (field, supported) in [("packageFormat", PACKAGE_FORMAT), ("runtimeABI", RUNTIME_ABI)] {
        if let Some(level) = value.get(field).and_then(serde_json::Value::as_u64).filter(|level| *level > supported) {
            return Err(err(Code::RequiresUpdate, format!("This slop needs {field} {level}; this hitSlop supports {supported}")));
        }
    }
    match value.get("packageFormat").and_then(serde_json::Value::as_u64) {
        Some(1) => validate_v1(&value),
        _ => Err(err(Code::InvalidRequest, "Invalid package format")),
    }
}

fn validate_v1(value: &serde_json::Value) -> Result<shape::Silhouette> {
    Manifest::validate(&value).map_err(|e| {
        let path = e.instance_path().as_str();
        err(Code::InvalidRequest, format!("Invalid manifest.json at {}", if path.is_empty() { "/" } else { path }))
    })?;
    let presentation = &value["presentation"];
    // The validated schema guarantees both dimensions and restricts the presentation.
    let width = presentation["width"].as_f64().expect("validated width");
    let height = presentation["height"].as_f64().expect("validated height");
    shape::silhouette(presentation.get("shape"), width, height)
}
