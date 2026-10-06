//! Native acceptance of a stored manifest: the authored contract, TypeBox-generated. The
//! document's package format and runtime ABI are columns beside it, checked before this runs
//! (`file::requirements`).
use crate::{Code, Result, err, shape};

#[jsonschema::validator(
    path = "../../packages/schema/generated/manifest.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = false, validate = true, iter_errors = false }
)]
struct Manifest;

/// A checked manifest's template slug, and its window: its shape, its size in points, and
/// the asset path its skin names, if any.
#[derive(Debug)]
pub struct Window {
    pub slug: String,
    pub silhouette: shape::Silhouette,
    pub width: u64,
    pub height: u64,
    pub skin: Option<String>,
}

// Every package format the core admits is read here. Raising it fails to compile: keep this
// reader, with the schema it validates against, for the released format and dispatch on the
// file's (docs/engineering-contract.md).
const _: () = assert!(crate::PACKAGE_FORMAT == 1, "keep the released package format's reader beside the new one");

pub fn validate(input: &str) -> Result<Window> {
    if input.len() > crate::wire::MANIFEST_BYTES {
        return Err(err(Code::TooLarge, format!("The manifest exceeds {} KiB", crate::wire::MANIFEST_BYTES >> 10)));
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|_| err(Code::InvalidRequest, "The manifest must be valid JSON"))?;
    Manifest::validate(&value).map_err(|e| {
        let path = e.instance_path().as_str();
        err(Code::InvalidRequest, format!("Invalid manifest at {}", if path.is_empty() { "/" } else { path }))
    })?;
    let presentation = &value["presentation"];
    // The validated schema guarantees both dimensions and restricts the presentation.
    // Read as numbers: JSON Schema counts `320.0` as an integer.
    let width = presentation["width"].as_f64().expect("validated width");
    let height = presentation["height"].as_f64().expect("validated height");
    Ok(Window {
        slug: value["slug"].as_str().expect("validated slug").to_owned(),
        silhouette: shape::silhouette(presentation.get("shape"), width, height)?,
        width: width as u64,
        height: height as u64,
        skin: presentation["skin"].as_str().map(str::to_owned),
    })
}
