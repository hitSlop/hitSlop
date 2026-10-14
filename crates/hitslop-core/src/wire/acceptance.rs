//! Stored app rows, captured by the producing engine and decoded by later builds.
//! The three row payloads are Brotli-compressed JSON, encoded as base64 within the
//! vector's JSON file. Keep the full byte-bound cases without inflating each release's
//! corpus; integrity hashes the compressed representation, replay checks the raw row.
use super::*;
use crate::app::AppDefinition;
use crate::{Result, encode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Ordinary acceptance replay excludes rows whose decompressed JSON exceeds 1 MiB.
const LARGE_ACCEPTANCE_VECTOR_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceVector {
    pub package_format: u64,
    #[serde(rename = "runtimeABI")]
    pub runtime_abi: u64,
    /// JSON reconstructed from the catalog columns, as passed to the format reader.
    #[serde(with = "compressed")]
    pub metadata: String,
    /// The original definition_json text, before any typed decoder discards members.
    #[serde(with = "compressed")]
    pub definition: String,
    /// The producing reader's typed re-encoding. Later readers retain these members
    /// and may also understand optional members the producing reader ignored.
    #[serde(with = "compressed")]
    pub accepted: AcceptedRow,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcceptedRow {
    pub metadata: Value,
    pub definition: Value,
}
mod compressed {
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};

    pub fn serialize<T: Serialize, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
        let mut bytes = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut bytes, 4096, 5, 22);
            serde_json::to_writer(&mut writer, value).map_err(serde::ser::Error::custom)?;
        }
        serializer.serialize_str(&data_encoding::BASE64.encode(&bytes))
    }

    pub fn deserialize<'de, T: DeserializeOwned, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        let bytes = data_encoding::BASE64.decode(encoded.as_bytes()).map_err(serde::de::Error::custom)?;
        serde_json::from_reader(brotli::Decompressor::new(bytes.as_slice(), 4096)).map_err(serde::de::Error::custom)
    }
}

impl AcceptanceVector {
    pub fn check(&self) -> Result<()> {
        let app = AppDefinition::decode(self.package_format, self.runtime_abi, &self.metadata, &self.definition)?;
        let metadata: Value = serde_json::from_str(&encode(app.metadata())).expect("encoded metadata");
        let definition: Value = serde_json::from_str(&app.definition_json()).expect("encoded definition");
        if !retains(&self.accepted.metadata, &metadata) || !retains(&self.accepted.definition, &definition) {
            return Err(crate::err(crate::Code::InvalidBytes, "Accepted row re-encodes differently"));
        }
        Ok(())
    }
}

/// A later reader may expose an optional member that was unknown when captured. All
/// members the producing reader understood must keep the same value and array order.
fn retains(expected: &Value, current: &Value) -> bool {
    match (expected, current) {
        (Value::Object(expected), Value::Object(current)) => {
            expected.iter().all(|(key, value)| current.get(key).is_some_and(|current| retains(value, current)))
        }
        (Value::Array(expected), Value::Array(current)) => {
            expected.len() == current.len()
                && expected.iter().zip(current).all(|(expected, current)| retains(expected, current))
        }
        _ => expected == current,
    }
}

impl AppDefinition {
    /// Positive raw-row acceptance cases for release capture, checked by this reader.
    pub fn acceptance_vectors() -> Result<Vec<(String, AcceptanceVector)>> {
        acceptance_vectors()
    }

    /// Classifies the decompressed JSON payloads without parsing the large stored rows.
    pub fn acceptance_vector_is_large(input: &str) -> Result<bool> {
        use std::io::Read;
        let stored: Value =
            serde_json::from_str(input).map_err(|error| crate::err(crate::Code::InvalidBytes, error))?;
        let mut total = 0;
        for field in ["metadata", "definition", "accepted"] {
            let encoded = stored[field]
                .as_str()
                .ok_or_else(|| crate::err(crate::Code::InvalidBytes, "Missing vector payload"))?;
            let bytes = data_encoding::BASE64
                .decode(encoded.as_bytes())
                .map_err(|error| crate::err(crate::Code::InvalidBytes, error))?;
            let mut reader = brotli::Decompressor::new(bytes.as_slice(), 4096);
            let mut buffer = [0; 8192];
            loop {
                let read = reader.read(&mut buffer).map_err(|error| crate::err(crate::Code::InvalidBytes, error))?;
                if read == 0 {
                    break;
                }
                total += read;
                if total > LARGE_ACCEPTANCE_VECTOR_BYTES {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Decodes and compares a captured row with its producing reader's typed result.
    pub fn check_acceptance_vector(input: &str) -> Result<()> {
        let vector: AcceptanceVector =
            serde_json::from_str(input).map_err(|error| crate::err(crate::Code::InvalidBytes, error))?;
        vector.check()
    }
}

fn base() -> (Value, Value) {
    (
        json!({
            "slug": "acceptance", "title": "Acceptance", "description": "Stored row",
            "author": {"name": "hitSlop", "url": "https://example.com/author"},
            "categories": ["developer-tools", "utilities"],
        }),
        json!({
            "window": {"kind": "standard", "width": OPEN_WINDOW_MIN_WIDTH, "height": OPEN_WINDOW_MIN_HEIGHT},
            "document": {"kind": "object", "properties": {
                "place": {"kind": "optional", "inner": {"kind": "object", "properties": {
                    "set": {"kind": "text"}, "clear": {"kind": "text"},
                    "path": {"kind": "text"}, "node": {"kind": "text"},
                }}},
            }},
            "theme": [{"token": "accent", "color": "#123456"}],
            "commands": [{"name": "probe", "description": "Probe", "args": {"kind": "object", "properties": {}}}],
            "views": {"preview": true, "export": true, "icon": true},
            "capabilities": ["camera", "microphone", "speech", "futureCapability"],
        }),
    )
}

fn vector(metadata: String, definition: String) -> Result<AcceptanceVector> {
    let app = AppDefinition::decode(PACKAGE_FORMAT, RUNTIME_ABI, &metadata, &definition)?;
    let accepted = AcceptedRow {
        metadata: serde_json::from_str(&encode(app.metadata())).expect("encoded metadata"),
        definition: serde_json::from_str(&app.definition_json()).expect("encoded definition"),
    };
    let vector =
        AcceptanceVector { package_format: PACKAGE_FORMAT, runtime_abi: RUNTIME_ABI, metadata, definition, accepted };
    Ok(vector)
}
fn push(out: &mut Vec<(String, AcceptanceVector)>, name: &str, metadata: &Value, definition: &Value) -> Result<()> {
    out.push((name.into(), vector(encode(metadata), encode(definition))?));
    Ok(())
}
/// Pads a raw JSON object to an exact byte bound with insignificant interior whitespace.
/// Raw descriptor text retains the whitespace; other typed containers may discard it.
fn padded(value: &Value, bytes: usize) -> String {
    let mut text = encode(value);
    assert!(text.len() <= bytes, "vector exceeds its target bound");
    text.insert_str(1, &" ".repeat(bytes - text.len()));
    text
}

/// Produces positive acceptance cases only. Refusals are not frozen: readers may loosen.
/// Every case self-checks through the actual format reader before capture writes it.
pub fn acceptance_vectors() -> Result<Vec<(String, AcceptanceVector)>> {
    acceptance_vectors_with_large(true)
}

// Capture always includes every case. Ordinary producer tests avoid constructing the
// three byte-bound rows; their target bounds alone already exceed the large threshold.
fn acceptance_vectors_with_large(include_large: bool) -> Result<Vec<(String, AcceptanceVector)>> {
    let mut out = vec![];
    let (metadata, definition) = base();
    push(&mut out, "standard-min", &metadata, &definition)?;
    for background in ["glass", "transparent"] {
        let mut row = definition.clone();
        row["window"]["background"] = background.into();
        row["window"]["width"] = OPEN_WINDOW_MAX.into();
        row["window"]["height"] = OPEN_WINDOW_MAX.into();
        row["window"]["fullscreenable"] = true.into();
        row["window"]["resizable"] = false.into();
        row["window"]["lockAspect"] = true.into();
        push(&mut out, &format!("standard-{background}-max"), &metadata, &row)?;
    }
    for (name, widget) in [("widget-empty", json!({})), ("widget-refresh", json!({"refreshMinutes": u32::MAX}))] {
        let mut row = definition.clone();
        row["views"]["widget"] = widget;
        push(&mut out, name, &metadata, &row)?;
    }
    for (name, radius) in
        [("radius-multi", "12px 24px / 50%".into()), ("radius-max", format!("12px{}", " ".repeat(SHAPE_RADIUS - 4)))]
    {
        let mut row = definition.clone();
        row["window"]["shape"] = radius.into();
        push(&mut out, name, &metadata, &row)?;
    }
    let mut path = definition.clone();
    let data = "M0 0 l10 0 h10 v10 c1 2 3 4 5 6 s3 4 5 6 q1 2 3 4 t3 4 a10 10 0 0 1 20 0 z";
    path["window"]["shape"] = json!({
        "path": format!("{data}{}", " ".repeat(SHAPE_PATH - data.len())),
        "viewBox": [SHAPE_VIEW_BOX, SHAPE_VIEW_BOX], "fillRule": "evenodd",
    });
    push(&mut out, "path-max-evenodd", &metadata, &path)?;
    path["window"]["shape"]["viewBox"] = json!([SHAPE_VIEW_BOX_MIN, SHAPE_VIEW_BOX_MIN]);
    path["window"]["shape"]["fillRule"] = "nonzero".into();
    push(&mut out, "path-min-nonzero", &metadata, &path)?;
    let mut row = definition.clone();
    let commands = format!("M0 0{}Z", "l1 0".repeat(SHAPE_COMMANDS - 2));
    row["window"]["shape"] = json!({"path": commands});
    push(&mut out, "path-command-max", &metadata, &row)?;
    // One expanded arc plus short lines reaches the normalized-segment bound without
    // exceeding either the source-command count or path-text budget.
    let mut radius = 16.0;
    let mut expanded_path = None;
    for _ in 0..100 {
        let data = format!("M0 0A{radius} {radius} 0 1 1 {radius} 0");
        let shape = json!({"path": data});
        if let Ok(crate::shape::Silhouette::Path { segments, .. }) =
            crate::shape::silhouette(Some(&shape), f64::from(OPEN_WINDOW_MIN_WIDTH), f64::from(OPEN_WINDOW_MIN_HEIGHT))
            && segments.len() >= SHAPE_SEGMENTS - (SHAPE_COMMANDS - 2)
        {
            expanded_path = Some(format!("{data}{}", "l0 0".repeat(SHAPE_SEGMENTS - segments.len())));
            break;
        }
        radius *= 2.0;
    }
    let shape = json!({"path": expanded_path.expect("an arc can reach the stored segment bound")});
    let silhouette =
        crate::shape::silhouette(Some(&shape), f64::from(OPEN_WINDOW_MIN_WIDTH), f64::from(OPEN_WINDOW_MIN_HEIGHT))?;
    let crate::shape::Silhouette::Path { segments, .. } = silhouette else { unreachable!() };
    assert_eq!(segments.len(), SHAPE_SEGMENTS);
    row["window"]["shape"] = shape;
    push(&mut out, "path-segment-max", &metadata, &row)?;
    let mut skin = definition.clone();
    skin["window"] = json!({
        "kind": "skin", "width": OPEN_WINDOW_MAX, "height": OPEN_WINDOW_MAX,
        "fullscreenable": true, "skin": format!("media/{}.png", "a".repeat(ATTACHMENT_ID_LENGTH)),
    });
    push(&mut out, "skin", &metadata, &skin)?;

    let mut row = definition.clone();
    let mut node = json!({"kind": "text"});
    for _ in 0..DESCRIPTOR_DEPTH {
        node = json!({"kind": "object", "properties": {"child": node}});
    }
    row["document"] = node;
    push(&mut out, "descriptor-depth-max", &metadata, &row)?;
    let mut fields = serde_json::Map::new();
    for index in 0..DESCRIPTOR_FIELDS {
        fields.insert(format!("field{index}"), json!({"kind": "boolean", "default": false}));
    }
    row["document"] = json!({"kind": "object", "properties": fields});
    push(&mut out, "descriptor-fields-max", &metadata, &row)?;
    row["document"] = json!({"kind": "object", "properties": {
        "choice": {"kind": "enum", "values": (0..ENUM_VALUES).map(|n| format!("value{n}")).collect::<Vec<_>>(),
            "description": "d".repeat(DESCRIPTOR_DESCRIPTION as usize)},
        "integer": {"kind": "integer", "min": -crate::MAX_SAFE, "max": crate::MAX_SAFE},
        "string": {"kind": "string", "minLength": 0, "maxLength": crate::MAX_SAFE},
    }});
    push(&mut out, "enum-description-integer-max", &metadata, &row)?;
    if include_large || APP_TEXT_BYTES <= LARGE_ACCEPTANCE_VECTOR_BYTES {
        out.push((
            "descriptor-bytes-max".into(),
            vector(encode(&metadata), {
                let raw = padded(&row["document"], APP_TEXT_BYTES);
                let mut row = row.clone();
                row["document"] = Value::Null;
                encode(&row).replace("\"document\":null", &format!("\"document\":{raw}"))
            })?,
        ));
    }

    let mut palette = definition.clone();
    palette["theme"] = json!((0..THEME_TOKENS).map(|n| {
        let prefix = format!("token{n}-");
        json!({"token": format!("{prefix}{}", "a".repeat(THEME_NAME_LIMIT - prefix.len())), "color": "#12345678"})
    }).collect::<Vec<_>>());
    push(&mut out, "theme-count-name-max", &metadata, &palette)?;

    let mut row = definition.clone();
    if include_large || ASSET_FILE_BYTES <= LARGE_ACCEPTANCE_VECTOR_BYTES {
        let args = json!({"kind": "object", "properties": fields});
        row["commands"] = json!(
            (0..COMMANDS)
                .map(|n| {
                    let prefix = format!("command{n}");
                    json!({"name": format!("{prefix}{}", "a".repeat(COMMAND_NAME_BYTES - prefix.len())),
            "description": "", "args": args})
                })
                .collect::<Vec<_>>()
        );
        let command_bytes = encode(&row["commands"]).len();
        row["commands"][0]["description"] = "d".repeat(ASSET_FILE_BYTES - command_bytes).into();
        assert_eq!(encode(&row["commands"]).len(), ASSET_FILE_BYTES);
        push(&mut out, "commands-count-name-args-bytes-max", &metadata, &row)?;
    }

    let mut future = definition.clone();
    future["future"] = json!({"nested": [true, "optional"]});
    future["window"]["future"] = json!({"frame": 2});
    future["window"]["shape"] =
        json!({"path": "M0 0L10 0L10 10Z", "viewBox": [10, 10], "fillRule": "evenodd", "future": true});
    future["views"]["future"] = true.into();
    future["views"]["widget"] = json!({"refreshMinutes": 1, "future": true});
    future["theme"][0]["future"] = true.into();
    future["commands"][0]["future"] = true.into();
    push(&mut out, "future-shaped", &metadata, &future)?;
    out.push(("metadata-bytes-max".into(), vector(padded(&metadata, MANIFEST_BYTES), encode(&definition))?));
    // Catalog JSON also reaches its bound with actual display text, not only whitespace.
    let mut large_metadata = metadata.clone();
    large_metadata["description"] =
        "d".repeat(MANIFEST_BYTES - encode(&large_metadata).len() + "Stored row".len()).into();
    assert_eq!(encode(&large_metadata).len(), MANIFEST_BYTES);
    push(&mut out, "metadata-display-bytes-max", &large_metadata, &definition)?;
    if include_large || DEFINITION_BYTES <= LARGE_ACCEPTANCE_VECTOR_BYTES {
        // The aggregate definition budget must be honored even for future optional members.
        row["theme"] = palette["theme"].clone();
        row["capabilities"] = json!(["c".repeat(MANIFEST_BYTES - 4)]);
        row["document"] = Value::Null;
        let document = padded(&definition["document"], APP_TEXT_BYTES);
        let mut aggregate = encode(&row).replace("\"document\":null", &format!("\"document\":{document}"));
        assert!(aggregate.len() <= DEFINITION_BYTES);
        aggregate.insert_str(1, &" ".repeat(DEFINITION_BYTES - aggregate.len()));
        out.push(("definition-bytes-max".into(), vector(encode(&metadata), aggregate)?));
    }
    // Containers whose known grammar cannot exhaust their byte budget still accept raw
    // rows with optional future fields up to that budget.
    let mut budgets = definition.clone();
    for (field, bytes) in [("window", MANIFEST_BYTES), ("theme", THEME_LIMIT)] {
        let target = if field == "theme" { &mut budgets[field][0] } else { &mut budgets[field] };
        target["future"] = "".into();
        let size = encode(&budgets[field]).len();
        let target = if field == "theme" { &mut budgets[field][0] } else { &mut budgets[field] };
        target["future"] = "f".repeat(bytes - size).into();
        assert_eq!(encode(&budgets[field]).len(), bytes);
    }
    budgets["capabilities"] = json!([""]);
    budgets["capabilities"][0] = "c".repeat(MANIFEST_BYTES - encode(&budgets["capabilities"]).len()).into();
    assert_eq!(encode(&budgets["capabilities"]).len(), MANIFEST_BYTES);
    push(&mut out, "window-theme-capability-bytes-max", &metadata, &budgets)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reencoding_retains_known_members_and_allows_later_optional_members() {
        let expected = json!({"tokens": [{"token": "accent", "color": "#123456"}]});
        assert!(retains(
            &expected,
            &json!({"tokens": [{"token": "accent", "color": "#123456", "future": true}], "future": true})
        ));
        assert!(!retains(&expected, &json!({"tokens": [{"token": "accent", "color": "#654321"}]})));
        assert!(!retains(&expected, &json!({"tokens": []})));
        assert!(!retains(&expected, &json!({"tokens": [{"token": "accent"}]})));
    }

    #[test]
    fn producing_reader_accepts_every_vector() {
        check_produced_vectors(false);
    }

    #[test]
    fn large_acceptance_vectors_producing_reader_accepts_every_vector() {
        check_produced_vectors(true);
    }

    fn check_produced_vectors(large: bool) {
        let vectors = acceptance_vectors_with_large(large).unwrap();
        assert!(!vectors.is_empty());
        let mut bytes = 0;
        let mut cases = 0;
        for (name, vector) in vectors {
            let stored = serde_json::to_string_pretty(&vector).unwrap();
            bytes += stored.len();
            if AppDefinition::acceptance_vector_is_large(&stored).unwrap() != large {
                continue;
            }
            cases += 1;
            vector.check().unwrap_or_else(|error| panic!("{name}: {error}"));
            AppDefinition::check_acceptance_vector(&stored).unwrap_or_else(|error| panic!("{name}: {error}"));
        }
        assert!(cases > 0, "no vectors selected");
        assert!(bytes <= 3 * 1024 * 1024, "acceptance vectors occupy {bytes} bytes; budget is 3 MiB");
    }
}
