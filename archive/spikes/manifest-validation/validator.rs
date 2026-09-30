//! Experimental module copied into a disposable core workspace by prepare.ts.
use serde_json::{json, Value};

#[cfg(all(feature = "manifest-runtime", feature = "manifest-compiled"))]
compile_error!("Measure one validator at a time");

#[cfg(feature = "manifest-runtime")]
fn schema_check(value: &Value) -> Result<(), (String, String)> {
    static VALIDATOR: std::sync::OnceLock<jsonschema::Validator> = std::sync::OnceLock::new();
    let validator = VALIDATOR.get_or_init(|| {
        let schema: Value = serde_json::from_str(include_str!("../../../manifest.schema.json")).unwrap();
        jsonschema::draft7::options().should_validate_formats(true).build(&schema).unwrap()
    });
    validator.validate(value).map_err(|e| (e.instance_path().to_string(), e.to_string()))
}

#[cfg(feature = "manifest-compiled")]
#[jsonschema::validator(
    path = "../../manifest.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = false, validate = true, iter_errors = false }
)]
struct Manifest;

#[cfg(feature = "manifest-compiled")]
fn schema_check(value: &Value) -> Result<(), (String, String)> {
    Manifest::validate(value).map_err(|e| (e.instance_path().to_string(), e.to_string()))
}

#[cfg(not(any(feature = "manifest-runtime", feature = "manifest-compiled")))]
fn schema_check(_: &Value) -> Result<(), (String, String)> { Ok(()) }

pub fn probe(input: &str) -> String {
    let value: Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(error) => return json!({"schemaValid": false, "valid": false, "path": "", "error": error.to_string()}).to_string(),
    };
    if let Err((path, error)) = schema_check(&value) {
        return json!({"schemaValid": false, "valid": false, "path": path, "error": error}).to_string();
    }
    let p = &value["presentation"];
    if p.get("skin").is_none() {
        let shape = p.get("shape").map(Value::to_string);
        if let Err(error) = crate::shape::silhouette(shape.as_deref(), p["width"].as_f64().unwrap_or(0.0), p["height"].as_f64().unwrap_or(0.0)) {
            return json!({"schemaValid": true, "valid": false, "path": "/presentation/shape", "error": error.to_string()}).to_string();
        }
    }
    json!({"schemaValid": true, "valid": true}).to_string()
}
