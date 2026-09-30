use super::*;

// A missing bound is optional; an explicitly null bound is invalid. In particular,
// JSON.stringify turns authored NaN/Infinity into null, which must not erase a rule.
fn present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(deserializer: D)
    -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

// The descriptor is authored data, never executable application code.
// Serialize gives the canonical form used for schema identity: fields in declaration
// order, properties sorted by the BTreeMap, numbers in their parsed type.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(super) enum Node {
    // Empty struct variants, not unit variants: serde ignores unknown fields on unit
    // variants of an internally tagged enum, which would accept `{"kind":"text","x":1}`.
    Text {},
    Boolean {},
    /// Stored as a map of writer key → integer contribution; projects to their sum.
    Counter {},
    /// Last writer wins. `maxLength` counts UTF-16 units, as JavaScript does.
    String {
        #[serde(default, rename = "maxLength", deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        max_length: Option<u64>,
    },
    /// A finite f64. Integral values project as JSON integers.
    Number {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
    },
    /// A safe integer (±2^53−1), stored as i64.
    Integer {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    Enum { values: Vec<String> },
    /// Absent until set; `clear` removes it.
    Optional { inner: Box<Node> },
    Object { properties: BTreeMap<String, Node> },
    /// Rows (object items with `$id`) or plain scalar elements addressed by index.
    List { item: Box<Node> },
    /// Entries by string key; each entry behaves like an optional field.
    Record { value: Box<Node> },
}
/// A record key: 1–256 UTF-16 units, not a reserved name.
pub(super) fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.encode_utf16().count() <= 256
        && !["$id", "__proto__", "constructor", "prototype"].contains(&key)
}
/// The stored form of a validated scalar.
pub(super) fn loro_scalar(node: &Node, value: &Value) -> loro::LoroValue {
    match unwrap_optional(node) {
        Node::Boolean {} => value.as_bool().unwrap().into(),
        Node::Number { .. } => value.as_f64().unwrap().into(),
        Node::Integer { .. } => value.as_i64().unwrap().into(),
        _ => value.as_str().unwrap().into(),
    }
}
/// The value kind under an optional wrapper.
pub(super) fn unwrap_optional(node: &Node) -> &Node {
    match node {
        Node::Optional { inner } => inner,
        node => node,
    }
}
pub(super) fn utf16_len(s: &str) -> u64 {
    s.encode_utf16().count() as u64
}
pub(super) fn is_scalar(node: &Node) -> bool {
    matches!(node, Node::Boolean {} | Node::String { .. } | Node::Number { .. } | Node::Integer { .. } | Node::Enum { .. })
}
/// Whether replacing a value of this kind would discard identity-bearing collections.
pub(super) fn holds_collections(node: &Node) -> bool {
    match node {
        Node::Text {} | Node::Counter {} | Node::List { .. } | Node::Record { .. } => true,
        Node::Object { properties } => properties.values().any(holds_collections),
        Node::Optional { inner } => holds_collections(inner),
        _ => false,
    }
}
impl Node {
    pub(super) fn check(&self, depth: usize) -> Result<()> {
        if depth > 16 {
            return Err(err(Code::TooLarge, "Descriptor depth"));
        }
        match self {
            Self::Object { properties } => {
                if properties.len() > 1024 {
                    return Err(err(Code::TooLarge, "Descriptor fields"));
                }
                for (key, node) in properties {
                    if !key.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        || ["path", "node", "__proto__", "constructor", "prototype"].contains(&key.as_str())
                    {
                        return Err(err(Code::InvalidSchema, "Field names are identifiers and not reserved"));
                    }
                    node.check(depth + 1)?;
                }
            }
            Self::List { item } => {
                if !matches!(**item, Self::Object { .. }) && !is_scalar(item) {
                    return Err(err(Code::InvalidSchema, "Lists contain object rows or scalars"));
                }
                item.check(depth + 1)?;
            }
            Self::Record { value } => {
                if !matches!(**value, Self::Object { .. }) && !is_scalar(value) {
                    return Err(err(Code::InvalidSchema, "Record values are scalars or objects"));
                }
                value.check(depth + 1)?;
            }
            Self::String { max_length } => {
                if max_length.is_some_and(|n| n > MAX_JSON as u64) {
                    return Err(err(Code::InvalidSchema, "maxLength is too large"));
                }
            }
            Self::Number { min, max } => {
                if min.is_some_and(|n| !n.is_finite()) || max.is_some_and(|n| !n.is_finite())
                    || matches!((min, max), (Some(a), Some(b)) if a > b)
                {
                    return Err(err(Code::InvalidSchema, "Number bounds must be finite with min ≤ max"));
                }
            }
            Self::Integer { min, max } => {
                if min.is_some_and(|n| !safe(n)) || max.is_some_and(|n| !safe(n))
                    || matches!((min, max), (Some(a), Some(b)) if a > b)
                {
                    return Err(err(Code::InvalidSchema, "Integer bounds must be safe with min ≤ max"));
                }
            }
            Self::Enum { values } => {
                let unique: BTreeSet<_> = values.iter().collect();
                if values.is_empty() || values.len() > 1024 || unique.len() != values.len() {
                    return Err(err(Code::InvalidSchema, "Enum needs 1–1024 unique values"));
                }
            }
            Self::Optional { inner } => {
                // Optional text, lists and counters wait for a design that keeps their
                // identity when two replicas create them concurrently.
                if !is_scalar(inner) && !matches!(**inner, Self::Object { .. } | Self::Text {}) {
                    return Err(err(Code::InvalidSchema, "Optional holds a scalar, text or an object"));
                }
                if let Self::Object { properties } = &**inner {
                    if properties.contains_key("set") || properties.contains_key("clear") {
                        return Err(err(Code::InvalidSchema, "Optional object fields cannot shadow set or clear"));
                    }
                }
                inner.check(depth + 1)?;
            }
            Self::Text {} | Self::Boolean {} | Self::Counter {} => {}
        }
        Ok(())
    }
    pub(super) fn validate(&self, value: &Value, row: bool) -> Result<()> {
        match self {
            // Writes and stored values share one scalar rule.
            scalar if is_scalar(scalar) => match issues::scalar_issue(scalar, value) {
                None => Ok(()),
                Some(IssueCode::OutOfRange) => Err(err(Code::OutOfRange, "Value is outside its bounds")),
                Some(_) => Err(err(Code::TypeMismatch, "Value does not match descriptor")),
            },
            Self::Text {} if value.is_string() => Ok(()),
            Self::Counter {} if value.as_i64().is_some_and(safe) => Ok(()),
            // `null` is never a value: an optional is set or absent.
            Self::Optional { inner } => inner.validate(value, false),
            Self::Object { properties } => {
                let map = value
                    .as_object()
                    .ok_or_else(|| err(Code::TypeMismatch, "Expected object"))?;
                if map
                    .keys()
                    .any(|k| !properties.contains_key(k) && !(row && k == "$id"))
                {
                    return Err(err(Code::TypeMismatch, "Unknown property"));
                }
                for (key, node) in properties {
                    match map.get(key) {
                        Some(value) => node.validate(value, false)?,
                        None if matches!(node, Self::Optional { .. }) => {}
                        None => return Err(err(Code::TypeMismatch, format!("Missing {key}"))),
                    }
                }
                if let Some(id) = map.get("$id") {
                    if !id.as_str().is_some_and(valid_id) {
                        return Err(err(
                            Code::InvalidId,
                            "Expected a safe 1–64 character application ID",
                        ));
                    }
                }
                Ok(())
            }
            Self::List { item } if is_scalar(item) => {
                let list = value
                    .as_array()
                    .ok_or_else(|| err(Code::TypeMismatch, "Expected list"))?;
                if list.len() > 100_000 {
                    return Err(err(Code::TooLarge, "List is too long"));
                }
                list.iter().try_for_each(|element| item.validate(element, false))
            }
            Self::Record { value: entry } => {
                let map = value
                    .as_object()
                    .ok_or_else(|| err(Code::TypeMismatch, "Expected record"))?;
                for (key, value) in map {
                    if !valid_key(key) {
                        return Err(err(Code::InvalidKey, "Record keys are 1–256 characters and not reserved"));
                    }
                    entry.validate(value, false)?;
                }
                Ok(())
            }
            Self::List { item } => {
                let list = value
                    .as_array()
                    .ok_or_else(|| err(Code::TypeMismatch, "Expected list"))?;
                let mut ids = BTreeSet::new();
                for value in list {
                    item.validate(value, true)?;
                    if let Some(id) = value.get("$id") {
                        if !ids.insert(id.as_str().unwrap()) {
                            return Err(err(Code::DuplicateId, "Duplicate row ID"));
                        }
                    }
                }
                Ok(())
            }
            _ => Err(err(Code::TypeMismatch, "Value does not match descriptor")),
        }
    }
}

pub(super) fn descriptor(s: &str) -> Result<Node> {
    let root: Node = parse(s)?;
    if !matches!(root, Node::Object { .. }) {
        return Err(err(
            Code::InvalidSchema,
            "Expected object descriptor root",
        ));
    }
    root.check(0)?;
    Ok(root)
}

/// Validates authoring input without creating a CRDT or executing authored code, and
/// returns the descriptor's schema key.
pub fn validate(schema: &str, initial: &str) -> Result<String> {
    let schema = descriptor(schema)?;
    let initial: Value = parse(initial)?;
    schema.validate(&initial, false)?;
    encode(&schema)
}

/// Stable descriptor identity: the parsed descriptor serialized canonically, so key
/// order and number spelling in the authored JSON never change it.
pub fn schema_key(schema: &str) -> Result<String> {
    encode(&descriptor(schema)?)
}
