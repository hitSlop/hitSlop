use super::*;
use crate::wire::present_option as present;

// The descriptor is authored data, never executable application code. Parsed, two
// descriptors compare by meaning: key order and number spelling never matter.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "descriptor.generated.ts"))]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Node {
    Text {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// Scalars may declare the `default` an object field takes when a write omits it.
    Boolean {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, deserialize_with = "present")]
        default: Option<bool>,
    },
    /// A safe-integer total; concurrent increments add up (a Loro counter).
    Counter {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// Last writer wins. Length bounds count Unicode code points.
    String {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, rename = "minLength", deserialize_with = "present")]
        min_length: Option<u64>,
        #[serde(default, rename = "maxLength", deserialize_with = "present")]
        max_length: Option<u64>,
        #[serde(default, deserialize_with = "present")]
        default: Option<String>,
    },
    /// A finite f64. Integral values project as JSON integers.
    Number {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, deserialize_with = "present")]
        min: Option<f64>,
        #[serde(default, deserialize_with = "present")]
        max: Option<f64>,
        #[serde(default, deserialize_with = "present")]
        default: Option<f64>,
    },
    /// A safe integer (±2^53−1), stored as i64.
    Integer {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, deserialize_with = "present")]
        min: Option<i64>,
        #[serde(default, deserialize_with = "present")]
        max: Option<i64>,
        #[serde(default, deserialize_with = "present")]
        default: Option<i64>,
    },
    Enum {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        values: Vec<String>,
        #[serde(default, deserialize_with = "present")]
        default: Option<String>,
    },
    /// Absent until set; `clear` removes it.
    Optional {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        inner: Box<Node>,
    },
    Object {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        properties: BTreeMap<String, Node>,
    },
    /// Rows (object items with `$id`) or plain scalar elements addressed by index.
    List {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        item: Box<Node>,
    },
    /// Entries by string key; each entry behaves like an optional field.
    Record {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        value: Box<Node>,
    },
}
/// A record key: 1–256 UTF-16 units, not a reserved name.
pub(super) fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.encode_utf16().count() <= crate::wire::RECORD_KEY_MAX
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
        Node::Optional { inner, .. } => inner,
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
        Node::Boolean { default, .. } => default.map(Value::from),
        Node::String { default, .. } | Node::Enum { default, .. } => default.clone().map(Value::from),
        Node::Number { default, .. } => default.map(|n| json!(n)),
        Node::Integer { default, .. } => default.map(Value::from),
        _ => None,
    }
}
pub(super) use crate::wire::MAX_SCALAR_LIST;
/// Names no field can take: `constructor`, `prototype` and the `Object.prototype` methods,
/// which a field of that name would shadow on a plain object. A handle's own members start
/// with `$`, which no field name can, so no member name is reserved. The list does not
/// grow: a stored descriptor is checked against it when its app opens.
const RESERVED_FIELDS: [&str; 8] = [
    "constructor",
    "prototype",
    "toString",
    "toLocaleString",
    "valueOf",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
];
impl Node {
    pub(super) fn check(&self, depth: usize) -> Result<()> {
        if depth > crate::wire::DESCRIPTOR_DEPTH {
            return Err(err(Code::TooLarge, "Descriptor depth"));
        }
        match self {
            Self::Object { properties, .. } => {
                if properties.len() > crate::wire::DESCRIPTOR_FIELDS {
                    return Err(err(Code::TooLarge, "Descriptor fields"));
                }
                for (key, node) in properties {
                    if !key.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        || RESERVED_FIELDS.contains(&key.as_str())
                    {
                        return Err(err(Code::InvalidSchema, "Field names are identifiers and not reserved"));
                    }
                    node.check(depth + 1)?;
                }
            }
            Self::List { item, .. } => {
                if !matches!(**item, Self::Object { .. }) && !is_scalar(item) {
                    return Err(err(Code::InvalidSchema, "Lists contain object rows or scalars"));
                }
                fields_only(item)?;
                item.check(depth + 1)?;
            }
            Self::Record { value, .. } => {
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
                if values.is_empty() || values.len() > crate::wire::ENUM_VALUES || unique.len() != values.len() {
                    return Err(err(Code::InvalidSchema, "Enum needs 1–1024 unique values"));
                }
            }
            Self::Optional { inner, .. } => {
                // Optional lists, records and counters are not implemented; no pop
                // needs them.
                if !is_scalar(inner) && !matches!(**inner, Self::Object { .. } | Self::Text { .. }) {
                    return Err(err(Code::InvalidSchema, "Optional holds a scalar, text or an object"));
                }
                if declared_default(inner).is_some() {
                    return Err(err(Code::InvalidSchema, "An optional field is absent until set; it has no default"));
                }
                inner.check(depth + 1)?;
            }
            Self::Text { .. } | Self::Boolean { .. } | Self::Counter { .. } => {}
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
    pub(super) fn default_value(&self) -> Option<Value> {
        match self {
            Self::Text { .. } => Some(json!("")),
            Self::List { .. } => Some(json!([])),
            Self::Record { .. } => Some(json!({})),
            Self::Object { .. } if self.omittable() => Some(json!({})),
            Self::Counter { .. } => Some(json!(0)),
            Self::Object { .. } | Self::Optional { .. } => None,
            scalar => declared_default(scalar),
        }
    }
    /// Whether an object field of this kind may be left out of a write: it is optional,
    /// or it has a default (an object, when every field of it may be left out).
    pub(super) fn omittable(&self) -> bool {
        match self {
            Self::Optional { .. } => true,
            Self::Object { properties, .. } => properties.values().all(Self::omittable),
            node => node.default_value().is_some(),
        }
    }
    /// Adds the defaults of the fields `value` omits, at every depth. A field without a
    /// default stays missing, so validation names it.
    pub(super) fn fill_defaults(&self, value: &mut Value) {
        match self {
            Self::Object { properties, .. } => {
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
            Self::Optional { inner, .. } => inner.fill_defaults(value),
            Self::List { item, .. } => {
                value.as_array_mut().into_iter().flatten().for_each(|row| item.fill_defaults(row))
            }
            Self::Record { value: entry, .. } => {
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
            Self::Text { .. } if value.is_string() => Ok(()),
            Self::Counter { .. } if value.as_i64().is_some_and(safe) => Ok(()),
            // `null` is never a value: an optional is set or absent.
            Self::Optional { inner, .. } => inner.validate(value, false),
            Self::Object { properties, .. } => {
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
            Self::List { item, .. } if is_scalar(item) => {
                let list = value.as_array().ok_or_else(|| err(Code::TypeMismatch, "Expected list"))?;
                if list.len() > MAX_SCALAR_LIST {
                    return Err(err(Code::TooLarge, "List is too long"));
                }
                list.iter().enumerate().try_for_each(|(i, element)| item.validate(element, false).map_err(|e| e.at(i)))
            }
            Self::Record { value: entry, .. } => {
                let map = value.as_object().ok_or_else(|| err(Code::TypeMismatch, "Expected record"))?;
                for (key, value) in map {
                    if !valid_key(key) {
                        return Err(err(Code::InvalidKey, "Record keys are 1–256 characters and not reserved").at(key));
                    }
                    entry.validate(value, false).map_err(|e| e.at(key))?;
                }
                Ok(())
            }
            Self::List { item, .. } => {
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
use crate::wire::DESCRIPTOR_DESCRIPTION as DESCRIPTION;
fn check_descriptions(node: &Value) -> Result<()> {
    let Some(map) = node.as_object() else { return Ok(()) };
    if let Some(description) = map.get("description")
        && !description.as_str().is_some_and(|d| !d.is_empty() && utf16_len(d) <= DESCRIPTION)
    {
        return Err(err(Code::InvalidSchema, "A description is 1–500 characters"));
    }
    if let Some(Value::Object(properties)) = map.get("properties") {
        properties.values().try_for_each(check_descriptions)?;
    }
    for child in ["item", "value", "inner"] {
        if let Some(child) = map.get(child) {
            check_descriptions(child)?;
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
pub(super) fn descriptor_value(root: Value) -> Result<Node> {
    check_descriptions(&root)?;
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

impl Node {
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Text { description, .. }
            | Self::Boolean { description, .. }
            | Self::Counter { description, .. }
            | Self::String { description, .. }
            | Self::Number { description, .. }
            | Self::Integer { description, .. }
            | Self::Enum { description, .. }
            | Self::Optional { description, .. }
            | Self::Object { description, .. }
            | Self::List { description, .. }
            | Self::Record { description, .. } => description.as_deref(),
        }
    }
}

// Descriptions are discovery metadata, not part of schema equality at mount.
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text { .. }, Self::Text { .. }) => true,
            (Self::Boolean { default: default_a, .. }, Self::Boolean { default: default_b, .. }) => {
                default_a == default_b
            }
            (Self::Counter { .. }, Self::Counter { .. }) => true,
            (
                Self::String { min_length: min_length_a, max_length: max_length_a, default: default_a, .. },
                Self::String { min_length: min_length_b, max_length: max_length_b, default: default_b, .. },
            ) => min_length_a == min_length_b && max_length_a == max_length_b && default_a == default_b,
            (
                Self::Number { min: min_a, max: max_a, default: default_a, .. },
                Self::Number { min: min_b, max: max_b, default: default_b, .. },
            ) => min_a == min_b && max_a == max_b && default_a == default_b,
            (
                Self::Integer { min: min_a, max: max_a, default: default_a, .. },
                Self::Integer { min: min_b, max: max_b, default: default_b, .. },
            ) => min_a == min_b && max_a == max_b && default_a == default_b,
            (
                Self::Enum { values: values_a, default: default_a, .. },
                Self::Enum { values: values_b, default: default_b, .. },
            ) => values_a == values_b && default_a == default_b,
            (Self::Optional { inner: inner_a, .. }, Self::Optional { inner: inner_b, .. }) => inner_a == inner_b,
            (Self::Object { properties: properties_a, .. }, Self::Object { properties: properties_b, .. }) => {
                properties_a == properties_b
            }
            (Self::List { item: item_a, .. }, Self::List { item: item_b, .. }) => item_a == item_b,
            (Self::Record { value: value_a, .. }, Self::Record { value: value_b, .. }) => value_a == value_b,
            _ => false,
        }
    }
}
