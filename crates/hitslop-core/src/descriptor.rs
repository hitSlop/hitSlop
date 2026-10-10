use super::*;
use crate::wire::present_option as present;

// The descriptor is authored data, never executable application code. Parsed, two
// descriptors compare by meaning: key order and number spelling never matter.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "descriptor.generated.ts"))]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Node {
    // Empty struct variants, not unit variants: serde ignores unknown fields on unit
    // variants of an internally tagged enum, which would accept `{"kind":"text","x":1}`.
    Text {},
    /// Scalars may declare the `default` an object field takes when a write omits it.
    Boolean {
        #[serde(default, deserialize_with = "present")]
        default: Option<bool>,
    },
    /// A safe-integer total; concurrent increments add up (a Loro counter).
    Counter {},
    /// Last writer wins. Length bounds count Unicode code points.
    String {
        #[serde(default, rename = "minLength", deserialize_with = "present")]
        min_length: Option<u64>,
        #[serde(default, rename = "maxLength", deserialize_with = "present")]
        max_length: Option<u64>,
        #[serde(default, deserialize_with = "present")]
        default: Option<String>,
    },
    /// A finite f64. Integral values project as JSON integers.
    Number {
        #[serde(default, deserialize_with = "present")]
        min: Option<f64>,
        #[serde(default, deserialize_with = "present")]
        max: Option<f64>,
        #[serde(default, deserialize_with = "present")]
        default: Option<f64>,
    },
    /// A safe integer (±2^53−1), stored as i64.
    Integer {
        #[serde(default, deserialize_with = "present")]
        min: Option<i64>,
        #[serde(default, deserialize_with = "present")]
        max: Option<i64>,
        #[serde(default, deserialize_with = "present")]
        default: Option<i64>,
    },
    Enum {
        values: Vec<String>,
        #[serde(default, deserialize_with = "present")]
        default: Option<String>,
    },
    /// Absent until set; `clear` removes it.
    Optional {
        inner: Box<Node>,
    },
    Object {
        properties: BTreeMap<String, Node>,
    },
    /// Rows (object items with `$id`) or plain scalar elements addressed by index.
    List {
        item: Box<Node>,
    },
    /// Entries by string key; each entry behaves like an optional field.
    Record {
        value: Box<Node>,
    },
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
        Node::Boolean { .. } => value.as_bool().expect("validated boolean").into(),
        Node::Number { .. } => value.as_f64().expect("validated number").into(),
        Node::Integer { .. } => integer(value).expect("validated integer").into(),
        _ => value.as_str().expect("validated string or enum").into(),
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
    matches!(
        node,
        Node::Boolean { .. } | Node::String { .. } | Node::Number { .. } | Node::Integer { .. } | Node::Enum { .. }
    )
}
/// The scalar's declared default, as JSON.
fn declared_default(node: &Node) -> Option<Value> {
    match node {
        Node::Boolean { default } => default.map(Value::from),
        Node::String { default, .. } | Node::Enum { default, .. } => default.clone().map(Value::from),
        Node::Number { default, .. } => default.map(|n| json!(n)),
        Node::Integer { default, .. } => default.map(Value::from),
        _ => None,
    }
}
pub(super) const MAX_SCALAR_LIST: usize = 100_000;
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
                fields_only(item)?;
                item.check(depth + 1)?;
            }
            Self::Record { value } => {
                if !matches!(**value, Self::Object { .. }) && !is_scalar(value) {
                    return Err(err(Code::InvalidSchema, "Record values are scalars or objects"));
                }
                fields_only(value)?;
                value.check(depth + 1)?;
            }
            Self::String { min_length, max_length, .. } => {
                if min_length.is_some_and(|n| n > MAX_SAFE as u64)
                    || max_length.is_some_and(|n| n > MAX_SAFE as u64)
                    || matches!((min_length, max_length), (Some(a), Some(b)) if a > b)
                {
                    return Err(err(
                        Code::InvalidSchema,
                        "String bounds must be nonnegative safe integers with minLength ≤ maxLength",
                    ));
                }
            }
            Self::Number { min, max, .. } => {
                if min.is_some_and(|n| !n.is_finite())
                    || max.is_some_and(|n| !n.is_finite())
                    || matches!((min, max), (Some(a), Some(b)) if a > b)
                {
                    return Err(err(Code::InvalidSchema, "Number bounds must be finite with min ≤ max"));
                }
            }
            Self::Integer { min, max, .. } => {
                if min.is_some_and(|n| !safe(n))
                    || max.is_some_and(|n| !safe(n))
                    || matches!((min, max), (Some(a), Some(b)) if a > b)
                {
                    return Err(err(Code::InvalidSchema, "Integer bounds must be safe with min ≤ max"));
                }
            }
            Self::Enum { values, .. } => {
                let unique: BTreeSet<_> = values.iter().collect();
                if values.is_empty() || values.len() > 1024 || unique.len() != values.len() {
                    return Err(err(Code::InvalidSchema, "Enum needs 1–1024 unique values"));
                }
            }
            Self::Optional { inner } => {
                // Optional lists, records and counters are not implemented; no slop
                // needs them.
                if !is_scalar(inner) && !matches!(**inner, Self::Object { .. } | Self::Text {}) {
                    return Err(err(Code::InvalidSchema, "Optional holds a scalar, text or an object"));
                }
                if declared_default(inner).is_some() {
                    return Err(err(Code::InvalidSchema, "An optional field is absent until set; it has no default"));
                }
                if let Self::Object { properties } = &**inner
                    && (properties.contains_key("set") || properties.contains_key("clear"))
                {
                    return Err(err(Code::InvalidSchema, "Optional object fields cannot shadow set or clear"));
                }
                inner.check(depth + 1)?;
            }
            Self::Text {} | Self::Boolean { .. } | Self::Counter {} => {}
        }
        if let Some(default) = declared_default(self) {
            check::scalar(self, &default)
                .map_err(|e| err(Code::InvalidSchema, format!("A default must fit its field ({})", e.message)))?;
        }
        Ok(())
    }
    /// The value an object field takes when a write omits it: empty text, lists and
    /// records, a zero counter, a scalar's declared default, or an object of its own
    /// defaults. Optional fields stay absent.
    fn default_value(&self) -> Option<Value> {
        match self {
            Self::Text {} => Some(json!("")),
            Self::List { .. } => Some(json!([])),
            Self::Record { .. } => Some(json!({})),
            Self::Object { .. } if self.omittable() => Some(json!({})),
            Self::Counter {} => Some(json!(0)),
            Self::Object { .. } | Self::Optional { .. } => None,
            scalar => declared_default(scalar),
        }
    }
    /// Whether an object field of this kind may be left out of a write: it is optional,
    /// or it has a default (an object, when every field of it may be left out).
    pub(super) fn omittable(&self) -> bool {
        match self {
            Self::Optional { .. } => true,
            Self::Object { properties } => properties.values().all(Self::omittable),
            node => node.default_value().is_some(),
        }
    }
    /// Adds the defaults of the fields `value` omits, at every depth. A field without a
    /// default stays missing, so validation names it.
    pub(super) fn fill_defaults(&self, value: &mut Value) {
        match self {
            Self::Object { properties } => {
                let Some(map) = value.as_object_mut() else { return };
                for (key, node) in properties {
                    if !map.contains_key(key)
                        && let Some(default) = node.default_value()
                    {
                        map.insert(key.clone(), default);
                    }
                    if let Some(child) = map.get_mut(key) {
                        node.fill_defaults(child);
                    }
                }
            }
            Self::Optional { inner } => inner.fill_defaults(value),
            Self::List { item } => value.as_array_mut().into_iter().flatten().for_each(|row| item.fill_defaults(row)),
            Self::Record { value: entry } => {
                value.as_object_mut().into_iter().flat_map(|map| map.values_mut()).for_each(|v| entry.fill_defaults(v))
            }
            _ => {}
        }
    }
    /// `value` with its omitted fields' defaults.
    pub(super) fn with_defaults(&self, value: &Value) -> Value {
        let mut value = value.clone();
        self.fill_defaults(&mut value);
        value
    }
    pub(super) fn validate(&self, value: &Value, row: bool) -> Result<()> {
        match self {
            // Writes and stored values share one scalar rule.
            scalar if is_scalar(scalar) => check::scalar(scalar, value),
            Self::Text {} if value.is_string() => Ok(()),
            Self::Counter {} if value.as_i64().is_some_and(safe) => Ok(()),
            // `null` is never a value: an optional is set or absent.
            Self::Optional { inner } => inner.validate(value, false),
            Self::Object { properties } => {
                let map = value.as_object().ok_or_else(|| err(Code::TypeMismatch, "Expected object"))?;
                if let Some(key) = map.keys().find(|k| !(properties.contains_key(*k) || (row && *k == "$id"))) {
                    return Err(err(Code::TypeMismatch, "Unknown property").at(key));
                }
                for (key, node) in properties {
                    match map.get(key) {
                        Some(value) => node.validate(value, false).map_err(|e| e.at(key))?,
                        None if matches!(node, Self::Optional { .. }) => {}
                        None => return Err(err(Code::TypeMismatch, format!("Missing {key}")).at(key)),
                    }
                }
                if let Some(id) = map.get("$id")
                    && !id.as_str().is_some_and(valid_id)
                {
                    return Err(err(Code::InvalidId, "Expected a safe 1–64 character application ID").at("$id"));
                }
                Ok(())
            }
            Self::List { item } if is_scalar(item) => {
                let list = value.as_array().ok_or_else(|| err(Code::TypeMismatch, "Expected list"))?;
                if list.len() > MAX_SCALAR_LIST {
                    return Err(err(Code::TooLarge, "List is too long"));
                }
                list.iter().enumerate().try_for_each(|(i, element)| item.validate(element, false).map_err(|e| e.at(i)))
            }
            Self::Record { value: entry } => {
                let map = value.as_object().ok_or_else(|| err(Code::TypeMismatch, "Expected record"))?;
                for (key, value) in map {
                    if !valid_key(key) {
                        return Err(err(Code::InvalidKey, "Record keys are 1–256 characters and not reserved").at(key));
                    }
                    entry.validate(value, false).map_err(|e| e.at(key))?;
                }
                Ok(())
            }
            Self::List { item } => {
                let list = value.as_array().ok_or_else(|| err(Code::TypeMismatch, "Expected list"))?;
                let mut ids = BTreeSet::new();
                for (i, value) in list.iter().enumerate() {
                    item.validate(value, true).map_err(|e| e.at(i))?;
                    if let Some(id) = value.get("$id")
                        && !ids.insert(id.as_str().expect("validated row ID"))
                    {
                        return Err(err(Code::DuplicateId, "Duplicate row ID").at("$id").at(i));
                    }
                }
                Ok(())
            }
            _ => Err(err(Code::TypeMismatch, "Value does not match descriptor")),
        }
    }
}

/// List elements and record entries are always written whole, so their scalars have no
/// default.
fn fields_only(node: &Node) -> Result<()> {
    if declared_default(node).is_some() {
        return Err(err(Code::InvalidSchema, "Defaults apply to object fields"));
    }
    Ok(())
}

/// The longest description, in UTF-16 units.
const DESCRIPTION: u64 = 500;
/// Checks a node's `description` and its children's, then removes them. Descriptions tell
/// people and agents what a field means; they stay in the stored descriptor and never
/// change what the descriptor means, so two that differ only in descriptions compare equal.
fn strip_descriptions(node: &mut Value) -> Result<()> {
    let Some(map) = node.as_object_mut() else { return Ok(()) };
    if let Some(description) = map.remove("description")
        && !description.as_str().is_some_and(|d| !d.is_empty() && utf16_len(d) <= DESCRIPTION)
    {
        return Err(err(Code::InvalidSchema, "A description is 1–500 characters"));
    }
    if let Some(Value::Object(properties)) = map.get_mut("properties") {
        properties.values_mut().try_for_each(strip_descriptions)?;
    }
    for child in ["item", "value", "inner"] {
        if let Some(child) = map.get_mut(child) {
            strip_descriptions(child)?;
        }
    }
    Ok(())
}
pub(super) fn descriptor(s: &str) -> Result<Node> {
    descriptor_value(parse(s)?)
}
/// The scalar default an argument or field declares, for projections.
pub(super) fn declared(node: &Node) -> Option<Value> {
    declared_default(node)
}
pub(super) fn descriptor_value(mut root: Value) -> Result<Node> {
    strip_descriptions(&mut root)?;
    let root: Node = serde_json::from_value(root).map_err(|e| err(Code::InvalidRequest, e))?;
    if !matches!(root, Node::Object { .. }) {
        return Err(err(Code::InvalidSchema, "Expected object descriptor root"));
    }
    root.check(0)?;
    Ok(root)
}

/// Validates authoring input without creating a CRDT or executing authored code.
pub fn validate(schema: &str, initial: &str) -> Result<()> {
    checked(schema, initial).map(|_| ())
}
/// The parsed descriptor, once the initial values are checked against it.
pub(crate) fn checked(schema: &str, initial: &str) -> Result<Node> {
    let schema = descriptor(schema)?;
    let mut initial: Value = parse(initial)?;
    schema.fill_defaults(&mut initial);
    schema.validate(&initial, false)?;
    Ok(schema)
}
