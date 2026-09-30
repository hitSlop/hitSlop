//! Shared native/WASM document semantics.

#[rustfmt::skip]
#[path = "wire.generated.rs"]
mod wire;
use loro::{
    Container, ContainerID, ContainerTrait, ExportMode, Frontiers, Index, LoroDoc, ID, LoroMap, LoroMovableList,
    LoroText, ValueOrContainer, VersionVector,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
mod publication;
mod identity;
mod edit;
use publication::{Events, ListState};
use std::sync::Arc;
use wire::{Anchor, Batch, Intent, Segment};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_JSON: usize = 4 * 1024 * 1024;

#[derive(Debug, Serialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(rename = "opIndex", skip_serializing_if = "Option::is_none")]
    pub op_index: Option<usize>,
}
type Result<T> = std::result::Result<T, Error>;
fn err(code: &str, message: impl ToString) -> Error {
    Error {
        code: code.into(),
        message: message.to_string(),
        op_index: None,
    }
}
fn engine(e: impl ToString) -> Error {
    err("engine_error", e)
}
fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    if s.len() > MAX_JSON {
        return Err(err("too_large", "JSON exceeds size limit"));
    }
    serde_json::from_str(s).map_err(|e| err("invalid_request", e))
}
fn encode(v: &impl Serialize) -> Result<String> {
    serde_json::to_string(v).map_err(engine)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Result<Vec<u8>> {
    if s.len() > MAX_JSON || s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(err("invalid_version", "Expected an opaque version token"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(engine))
        .collect()
}
/// Version tokens are the document's frontiers: the IDs of its latest operations,
/// sorted, as 12-byte big-endian (peer, counter) records. They grow with concurrent
/// heads, not with every peer ever seen, and are stable across import and reopen.
fn version_token(frontiers: &Frontiers) -> String {
    let mut ids: Vec<ID> = frontiers.iter().collect();
    ids.sort();
    hex(&ids
        .iter()
        .flat_map(|id| id.peer.to_be_bytes().into_iter().chain(id.counter.to_be_bytes()))
        .collect::<Vec<_>>())
}
/// Decodes a token and proves every ID is in this document's history before any Loro
/// API sees it; unknown operations must never reach a panicking conversion.
fn decode_version(doc: &LoroDoc, s: &str) -> Result<(Frontiers, VersionVector)> {
    let bytes = unhex(s)?;
    if bytes.is_empty() || bytes.len() % 12 != 0 || bytes.len() > 12 * 1024 {
        return Err(err("invalid_version", "Expected an opaque version token"));
    }
    let known = doc.oplog_vv();
    let mut ids = Vec::with_capacity(bytes.len() / 12);
    for record in bytes.chunks_exact(12) {
        let peer = u64::from_be_bytes(record[..8].try_into().expect("8 bytes"));
        let counter = i32::from_be_bytes(record[8..].try_into().expect("4 bytes"));
        if counter < 0 {
            return Err(err("invalid_version", "Negative counter"));
        }
        let id = ID::new(peer, counter);
        if !known.includes_id(id) {
            return Err(err("stale_base", "Version names operations this document does not have"));
        }
        ids.push(id);
    }
    let frontiers = Frontiers::from(ids);
    let vv = doc
        .frontiers_to_vv(&frontiers)
        .ok_or_else(|| err("stale_base", "Version is not in this document's history"))?;
    Ok((frontiers, vv))
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
const MAX_SAFE: i64 = 9_007_199_254_740_991;
fn safe(n: i64) -> bool {
    (-MAX_SAFE..=MAX_SAFE).contains(&n)
}
/// The exact sum of a counter's stored contributions, or `None` when any
/// contribution is not a safe integer or the sum leaves the safe range.
fn counter_sum(raw: &Value) -> Option<i64> {
    raw.as_object()?.values().try_fold(0i64, |sum, v| {
        let n = v.as_i64().filter(|n| safe(*n))?;
        sum.checked_add(n).filter(|n| safe(*n))
    })
}
/// The application view of a raw stored value: counters become their sum.
/// Anomalous counters read as null; their stored contributions remain untouched.
fn project(node: Option<&Node>, value: Value) -> Value {
    match (node, value) {
        (Some(Node::Optional { inner }), value) => project(Some(inner), value),
        // JavaScript has one number type: an integral f64 reads as an integer, so a
        // snapshot compares equal in Rust, Swift and JavaScript.
        (Some(Node::Number { .. }), Value::Number(n)) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() <= MAX_SAFE as f64 => json!(f as i64),
            _ => Value::Number(n),
        },
        (Some(Node::Counter), raw) => match counter_sum(&raw) {
            Some(sum) => json!(sum),
            None => Value::Null,
        },
        (Some(Node::Object { properties }), Value::Object(mut map)) => {
            for (key, child) in properties {
                if let Some(v) = map.remove(key) {
                    map.insert(key.clone(), project(Some(child), v));
                }
            }
            Value::Object(map)
        }
        (Some(Node::List { item }), Value::Array(rows)) => {
            Value::Array(rows.into_iter().map(|r| project(Some(item), r)).collect())
        }
        (Some(Node::Record { value: entry }), Value::Object(map)) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, project(Some(entry), v))).collect())
        }
        (_, value) => value,
    }
}
/// Container-aware rendering supplies effective row IDs while preserving raw storage.
fn project_container(node: Option<&Node>, value: ValueOrContainer) -> Result<Value> {
    match (node, &value) {
        (Some(Node::Optional { inner }), ValueOrContainer::Container(_)) => project_container(Some(inner), value),
        (Some(Node::Object { properties }), ValueOrContainer::Container(Container::Map(map))) => {
            let mut result = serde_json::to_value(map.get_deep_value()).map_err(engine)?;
            for (key, child) in properties {
                if let Some(v) = map.get(key) { result[key] = project_container(Some(child), v)?; }
            }
            Ok(result)
        }
        (Some(Node::Record { value: entry }), ValueOrContainer::Container(Container::Map(map))) => {
            let mut result = serde_json::Map::new();
            for key in map.keys() {
                let key = key.to_string();
                if let Some(v) = map.get(&key) {
                    result.insert(key, project_container(Some(entry), v)?);
                }
            }
            Ok(Value::Object(result))
        }
        (Some(Node::List { item }), ValueOrContainer::Container(Container::MovableList(list))) if !is_scalar(item) => {
            let ids = identity::rows(list);
            let mut result = Vec::with_capacity(list.len());
            for (index, id) in ids.into_iter().enumerate() {
                let mut row = project_container(Some(item), list.get(index).unwrap())?;
                if let (Some(id), Some(map)) = (id, row.as_object_mut()) { map.insert("$id".into(), json!(id)); }
                result.push(row);
            }
            Ok(Value::Array(result))
        }
        _ => Ok(project(node, serde_json::to_value(value.get_deep_value()).map_err(engine)?)),
    }
}
fn project_at(doc: &LoroDoc, node: Option<&Node>, cid: &ContainerID) -> Result<Value> {
    let container = doc.get_container(cid.clone()).ok_or_else(|| err("path_not_found", "Container is absent"))?;
    project_container(node, ValueOrContainer::Container(container))
}
// Same 128 random bits / Crockford base32 representation as document/identity.ts.
fn application_id() -> Result<String> {
    const ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(engine)?;
    let mut buffer = 0u32;
    let mut bits = 0;
    let mut out = String::new();
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    Ok(out)
}

// The descriptor is authored data, never executable application code.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Node {
    Text,
    Boolean,
    /// Stored as a map of writer key → integer contribution; projects to their sum.
    Counter,
    /// Last writer wins. `maxLength` counts UTF-16 units, as JavaScript does.
    String {
        #[serde(default, rename = "maxLength")]
        max_length: Option<u64>,
    },
    /// A finite f64. Integral values project as JSON integers.
    Number {
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
    },
    /// A safe integer (±2^53−1), stored as i64.
    Integer {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
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
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.encode_utf16().count() <= 256
        && !["$id", "__proto__", "constructor", "prototype"].contains(&key)
}
/// The stored form of a validated scalar.
fn loro_scalar(node: &Node, value: &Value) -> loro::LoroValue {
    match unwrap_optional(node) {
        Node::Boolean => value.as_bool().unwrap().into(),
        Node::Number { .. } => value.as_f64().unwrap().into(),
        Node::Integer { .. } => value.as_i64().unwrap().into(),
        _ => value.as_str().unwrap().into(),
    }
}
/// The value kind under an optional wrapper.
fn unwrap_optional(node: &Node) -> &Node {
    match node {
        Node::Optional { inner } => inner,
        node => node,
    }
}
fn utf16_len(s: &str) -> u64 {
    s.encode_utf16().count() as u64
}
fn is_scalar(node: &Node) -> bool {
    matches!(node, Node::Boolean | Node::String { .. } | Node::Number { .. } | Node::Integer { .. } | Node::Enum { .. })
}
/// Whether replacing a value of this kind would discard identity-bearing collections.
fn holds_collections(node: &Node) -> bool {
    match node {
        Node::Text | Node::Counter | Node::List { .. } | Node::Record { .. } => true,
        Node::Object { properties } => properties.values().any(holds_collections),
        Node::Optional { inner } => holds_collections(inner),
        _ => false,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    format: u32,
    root: Node,
}
impl Node {
    fn check(&self, depth: usize) -> Result<()> {
        if depth > 32 {
            return Err(err("too_large", "Descriptor depth"));
        }
        match self {
            Self::Object { properties } => {
                if properties.len() > 1024 {
                    return Err(err("too_large", "Descriptor fields"));
                }
                for (key, node) in properties {
                    if key.is_empty()
                        || key == "$id"
                        || ["__proto__", "constructor", "prototype"].contains(&key.as_str())
                    {
                        return Err(err("invalid_schema", "Reserved or empty key"));
                    }
                    node.check(depth + 1)?;
                }
            }
            Self::List { item } => {
                if !matches!(**item, Self::Object { .. }) && !is_scalar(item) {
                    return Err(err("invalid_schema", "Lists contain object rows or scalars"));
                }
                item.check(depth + 1)?;
            }
            Self::Record { value } => {
                if !matches!(**value, Self::Object { .. }) && !is_scalar(value) {
                    return Err(err("invalid_schema", "Record values are scalars or objects"));
                }
                value.check(depth + 1)?;
            }
            Self::String { max_length } => {
                if max_length.is_some_and(|n| n > MAX_JSON as u64) {
                    return Err(err("invalid_schema", "maxLength is too large"));
                }
            }
            Self::Number { min, max } => {
                if min.is_some_and(|n| !n.is_finite()) || max.is_some_and(|n| !n.is_finite())
                    || matches!((min, max), (Some(a), Some(b)) if a > b)
                {
                    return Err(err("invalid_schema", "Number bounds must be finite with min ≤ max"));
                }
            }
            Self::Integer { min, max } => {
                if min.is_some_and(|n| !safe(n)) || max.is_some_and(|n| !safe(n))
                    || matches!((min, max), (Some(a), Some(b)) if a > b)
                {
                    return Err(err("invalid_schema", "Integer bounds must be safe with min ≤ max"));
                }
            }
            Self::Enum { values } => {
                let unique: BTreeSet<_> = values.iter().collect();
                if values.is_empty() || values.len() > 1024 || unique.len() != values.len() {
                    return Err(err("invalid_schema", "Enum needs 1–1024 unique values"));
                }
            }
            Self::Optional { inner } => {
                // Optional text, lists and counters wait for a design that keeps their
                // identity when two replicas create them concurrently.
                if !is_scalar(inner) && !matches!(**inner, Self::Object { .. } | Self::Text) {
                    return Err(err("invalid_schema", "Optional holds a scalar, text or an object"));
                }
                inner.check(depth + 1)?;
            }
            Self::Text | Self::Boolean | Self::Counter => {}
        }
        Ok(())
    }
    fn validate(&self, value: &Value, row: bool) -> Result<()> {
        match self {
            Self::Text if value.is_string() => Ok(()),
            Self::Boolean if value.is_boolean() => Ok(()),
            Self::Counter if value.as_i64().is_some_and(safe) => Ok(()),
            Self::String { max_length } if value.is_string() => {
                if max_length.is_some_and(|max| utf16_len(value.as_str().unwrap()) > max) {
                    return Err(err("out_of_range", "Text is longer than maxLength"));
                }
                Ok(())
            }
            Self::Number { min, max } if value.as_f64().is_some_and(f64::is_finite) => {
                let n = value.as_f64().unwrap();
                if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
                    return Err(err("out_of_range", "Number is outside its bounds"));
                }
                Ok(())
            }
            Self::Integer { min, max } if value.as_i64().is_some_and(safe) => {
                let n = value.as_i64().unwrap();
                if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
                    return Err(err("out_of_range", "Integer is outside its bounds"));
                }
                Ok(())
            }
            Self::Enum { values } if value.as_str().is_some_and(|v| values.iter().any(|x| x == v)) => Ok(()),
            // `null` is never a value: an optional is set or absent.
            Self::Optional { inner } => inner.validate(value, false),
            Self::Object { properties } => {
                let map = value
                    .as_object()
                    .ok_or_else(|| err("type_mismatch", "Expected object"))?;
                if map
                    .keys()
                    .any(|k| !properties.contains_key(k) && !(row && k == "$id"))
                {
                    return Err(err("type_mismatch", "Unknown property"));
                }
                for (key, node) in properties {
                    match map.get(key) {
                        Some(value) => node.validate(value, false)?,
                        None if matches!(node, Self::Optional { .. }) => {}
                        None => return Err(err("type_mismatch", format!("Missing {key}"))),
                    }
                }
                if let Some(id) = map.get("$id") {
                    if !id.as_str().is_some_and(valid_id) {
                        return Err(err(
                            "invalid_id",
                            "Expected a safe 1–64 character application ID",
                        ));
                    }
                }
                Ok(())
            }
            Self::List { item } if is_scalar(item) => {
                let list = value
                    .as_array()
                    .ok_or_else(|| err("type_mismatch", "Expected list"))?;
                if list.len() > 100_000 {
                    return Err(err("too_large", "List is too long"));
                }
                list.iter().try_for_each(|element| item.validate(element, false))
            }
            Self::Record { value: entry } => {
                let map = value
                    .as_object()
                    .ok_or_else(|| err("type_mismatch", "Expected record"))?;
                for (key, value) in map {
                    if !valid_key(key) {
                        return Err(err("invalid_key", "Record keys are 1–256 characters and not reserved"));
                    }
                    entry.validate(value, false)?;
                }
                Ok(())
            }
            Self::List { item } => {
                let list = value
                    .as_array()
                    .ok_or_else(|| err("type_mismatch", "Expected list"))?;
                let mut ids = BTreeSet::new();
                for value in list {
                    item.validate(value, true)?;
                    if let Some(id) = value.get("$id") {
                        if !ids.insert(id.as_str().unwrap()) {
                            return Err(err("duplicate_id", "Duplicate row ID"));
                        }
                    }
                }
                Ok(())
            }
            _ => Err(err("type_mismatch", "Value does not match descriptor")),
        }
    }
}

fn descriptor(s: &str) -> Result<Node> {
    let d: Descriptor = parse(s)?;
    if d.format != 1 || !matches!(d.root, Node::Object { .. }) {
        return Err(err(
            "invalid_schema",
            "Expected descriptor format 1 with object root",
        ));
    }
    d.root.check(0)?;
    Ok(d.root)
}

fn fill(map: &LoroMap, node: &Node, value: &Value, writer: &str) -> Result<()> {
    let Node::Object { properties } = node else {
        return Err(err("type_mismatch", "Expected object"));
    };
    for (key, child) in properties {
        match value.get(key) {
            Some(value) => put(map, key, child, value, writer)?,
            None if matches!(child, Node::Optional { .. }) => {}
            None => return Err(err("type_mismatch", format!("Missing {key}"))),
        }
    }
    Ok(())
}
/// Stores a validated value at `map[key]` in the representation its kind uses.
fn put(map: &LoroMap, key: &str, node: &Node, value: &Value, writer: &str) -> Result<()> {
    match node {
        Node::Optional { inner } => put(map, key, inner, value, writer),
        Node::Boolean => map.insert(key, value.as_bool().unwrap()).map_err(engine),
        Node::String { .. } | Node::Enum { .. } => map.insert(key, value.as_str().unwrap()).map_err(engine),
        Node::Number { .. } => map.insert(key, value.as_f64().unwrap()).map_err(engine),
        Node::Integer { .. } => map.insert(key, value.as_i64().unwrap()).map_err(engine),
        Node::Counter => {
            let counter = map.insert_container(key, LoroMap::new()).map_err(engine)?;
            let initial = value.as_i64().unwrap();
            if initial != 0 {
                counter.insert(writer, initial).map_err(engine)?;
            }
            Ok(())
        }
        Node::Text => {
            let text = map.insert_container(key, LoroText::new()).map_err(engine)?;
            text.insert_utf16(0, value.as_str().unwrap()).map_err(engine)
        }
        Node::Object { .. } => {
            let child = map.insert_container(key, LoroMap::new()).map_err(engine)?;
            fill(&child, node, value, writer)
        }
        Node::List { item } if is_scalar(item) => {
            let list = map.insert_container(key, LoroMovableList::new()).map_err(engine)?;
            for (index, element) in value.as_array().unwrap().iter().enumerate() {
                list.insert(index, loro_scalar(item, element)).map_err(engine)?;
            }
            Ok(())
        }
        Node::Record { value: entry } => {
            let record = map.insert_container(key, LoroMap::new()).map_err(engine)?;
            for (key, value) in value.as_object().unwrap() {
                put(&record, key, entry, value, writer)?;
            }
            Ok(())
        }
        Node::List { item } => {
            let list = map.insert_container(key, LoroMovableList::new()).map_err(engine)?;
            for (index, row) in value.as_array().unwrap().iter().enumerate() {
                let id = row
                    .get("$id")
                    .and_then(Value::as_str)
                    .map(String::from)
                    .map(Ok)
                    .unwrap_or_else(application_id)?;
                insert_unchecked(&list, item, index, &id, row, writer)?;
            }
            Ok(())
        }
    }
}
fn insert_unchecked(
    list: &LoroMovableList,
    item: &Node,
    index: usize,
    id: &str,
    value: &Value,
    writer: &str,
) -> Result<()> {
    item.validate(value, true)?;
    if !valid_id(id) {
        return Err(err(
            "invalid_id",
            "Expected a safe 1–64 character application ID",
        ));
    }
    if value.get("$id").is_some_and(|v| v.as_str() != Some(id)) {
        return Err(err("invalid_id", "Conflicting IDs"));
    }
    let row = list
        .insert_container(index, LoroMap::new())
        .map_err(engine)?;
    row.insert("$id", id).map_err(engine)?;
    fill(&row, item, value, writer)
}
/// Row lookup during one batch. Untouched lists use the persistent index published
/// from Loro events; a list changed earlier in the same batch is scanned live,
/// because its index only advances when the committed events are published.
pub(crate) struct Rows<'a> {
    lists: &'a HashMap<ContainerID, ListState>,
    touched: HashSet<ContainerID>,
}
impl<'a> Rows<'a> {
    fn new(lists: &'a HashMap<ContainerID, ListState>) -> Self {
        Self {
            lists,
            touched: HashSet::new(),
        }
    }
    fn indexed(&self, list: &LoroMovableList) -> Option<&ListState> {
        let id = list.id();
        if self.touched.contains(&id) {
            None
        } else {
            self.lists.get(&id).filter(|state| state.clean())
        }
    }
    fn scan(list: &LoroMovableList, id: &str) -> Result<(usize, LoroMap)> {
        let ids = identity::rows(list);
        for (index, effective) in ids.iter().enumerate() {
            if effective.as_deref() == Some(id) {
                if let Some(ValueOrContainer::Container(Container::Map(map))) = list.get(index) { return Ok((index, map)); }
            }
        }
        Err(err("path_not_found", "Row is absent"))
    }
    fn unique<'s>(state: &'s ListState, id: &str) -> Result<&'s ContainerID> {
        match state.by_id.get(id).map(Vec::as_slice) {
            Some([cid]) => Ok(cid),
            Some(_) => Err(err("duplicate_id", "Ambiguous row ID")),
            None => Err(err("path_not_found", "Row is absent")),
        }
    }
    fn map(&self, doc: &LoroDoc, list: &LoroMovableList, id: &str) -> Result<LoroMap> {
        match self.indexed(list) {
            Some(state) => Ok(doc.get_map(Self::unique(state, id)?.clone())),
            None => Ok(Self::scan(list, id)?.1),
        }
    }
    fn index(&self, list: &LoroMovableList, id: &str) -> Result<usize> {
        match self.indexed(list) {
            Some(state) => {
                let cid = Self::unique(state, id)?;
                state
                    .order
                    .iter()
                    .position(|c| c.as_ref() == Some(cid))
                    .ok_or_else(|| err("engine_error", "Row index out of sync"))
            }
            None => Ok(Self::scan(list, id)?.0),
        }
    }
    fn touch(&mut self, list: &LoroMovableList) {
        self.touched.insert(list.id());
    }
}
fn position(list: &LoroMovableList, anchor: &Option<Anchor>, rows: &Rows) -> Result<usize> {
    match anchor {
        None => Ok(list.len()),
        Some(Anchor::Before { before }) => rows.index(list, before),
        Some(Anchor::After { after }) => Ok(rows.index(list, after)? + 1),
    }
}
struct Location {
    node: Node,
    value: ValueOrContainer,
    parent: Option<(LoroMap, String)>,
    /// The final segment names an optional field or record entry that is not set
    /// (`value` is null).
    absent: bool,
    /// The final segment is a record key: the entry is created by `set`, removed by `clear`.
    entry: bool,
    /// The final segment is a scalar-list element: the list and its index.
    element: Option<(LoroMovableList, usize)>,
}
fn resolve(doc: &LoroDoc, schema: &Node, path: &[Segment], rows: &Rows) -> Result<Location> {
    if path.is_empty() || path.len() > 64 {
        return Err(err("invalid_path", "Path length"));
    }
    let mut node = schema;
    let mut value = ValueOrContainer::Container(Container::Map(doc.get_map("data")));
    let mut parent = None;
    let mut absent = false;
    let mut entry = false;
    let mut element = None;
    for (index, segment) in path.iter().enumerate() {
        entry = false;
        element = None;
        if absent {
            return Err(err("path_not_found", "Optional field is not set"));
        }
        // A set optional behaves as its inner kind when a path continues through it.
        let current = if index == 0 { node } else { unwrap_optional(node) };
        match (segment, current, &value) {
            (
                Segment::Key(key),
                Node::Object { properties },
                ValueOrContainer::Container(Container::Map(map)),
            ) => {
                let next = properties
                    .get(key)
                    .ok_or_else(|| err("path_not_found", "Unknown field"))?;
                let child = match map.get(key) {
                    Some(child) => child,
                    None if matches!(next, Node::Optional { .. }) => {
                        absent = true;
                        ValueOrContainer::Value(loro::LoroValue::Null)
                    }
                    None => return Err(err("path_not_found", "Missing field")),
                };
                parent = Some((map.clone(), key.clone()));
                node = next;
                value = child;
            }
            (
                Segment::Key(key),
                Node::Record { value: next },
                ValueOrContainer::Container(Container::Map(map)),
            ) => {
                if !valid_key(key) {
                    return Err(err("invalid_key", "Record keys are 1–256 characters and not reserved"));
                }
                let child = match map.get(key) {
                    Some(child) => child,
                    None => {
                        absent = true;
                        ValueOrContainer::Value(loro::LoroValue::Null)
                    }
                };
                parent = Some((map.clone(), key.clone()));
                entry = true;
                node = next;
                value = child;
            }
            (
                Segment::Index { index },
                Node::List { item },
                ValueOrContainer::Container(Container::MovableList(list)),
            ) if is_scalar(item) => {
                let child = list
                    .get(*index)
                    .ok_or_else(|| err("path_not_found", "No element at that index"))?;
                element = Some((list.clone(), *index));
                node = item;
                value = child;
                parent = None;
            }
            (
                Segment::Id { id },
                Node::List { item },
                ValueOrContainer::Container(Container::MovableList(list)),
            ) if !is_scalar(item) => {
                let map = rows.map(doc, list, id)?;
                node = item;
                value = ValueOrContainer::Container(Container::Map(map));
                parent = None;
            }
            _ => return Err(err("type_mismatch", "Path traverses an incompatible value")),
        }
    }
    Ok(Location {
        node: node.clone(),
        value,
        parent,
        absent,
        entry,
        element,
    })
}

/// Validates each intent completely before its first Loro mutation. A failure in a
/// later intent can still leave earlier intents applied; `Document::abort` owns that.
fn execute(
    doc: &LoroDoc,
    schema: &Node,
    op: &Intent,
    ids: &mut Vec<String>,
    rows: &mut Rows,
) -> Result<()> {
    let at = resolve(doc, schema, op.path(), rows)?;
    match op {
        Intent::Set { value, .. } => {
            let kind = unwrap_optional(&at.node);
            // One scalar-list element: last writer wins.
            if let Some((list, index)) = &at.element {
                kind.validate(value, false)?;
                if let ValueOrContainer::Value(stored) = &at.value {
                    let stored = serde_json::to_value(stored).map_err(engine)?;
                    if scalar_issue(kind, &stored) == Some("type_mismatch") {
                        return Err(err("type_mismatch", "Cannot edit anomalous element"));
                    }
                }
                list.set(*index, loro_scalar(kind, value)).map_err(engine)?;
                return Ok(());
            }
            // Whole-field text replaces the text as it is at execution. The script exists
            // before the first mutation, so a slow diff never leaves a batch half applied.
            if let (Node::Text, ValueOrContainer::Container(Container::Text(text))) = (kind, &at.value) {
                let to = value.as_str().ok_or_else(|| err("type_mismatch", "Expected text"))?;
                let delta = edit::script(&text.to_string(), to, to.chars().count());
                if !delta.is_empty() {
                    text.apply_delta(&delta).map_err(engine)?;
                }
                return Ok(());
            }
            // A whole scalar list: rewrite it, keeping unchanged positions.
            if let Node::List { item } = kind {
                if !is_scalar(item) {
                    return Err(err("type_mismatch", "Rows are edited with insert, remove and move"));
                }
                kind.validate(value, false)?;
                let ValueOrContainer::Container(Container::MovableList(list)) = &at.value else {
                    return Err(err("type_mismatch", "Cannot edit anomalous list"));
                };
                return rewrite_list(list, item, value.as_array().unwrap());
            }
            // Optional fields and record entries can be absent, created and replaced.
            let optional = matches!(at.node, Node::Optional { .. }) || at.entry;
            let replaces_object = matches!(kind, Node::Object { .. }) && optional;
            let creates_text = matches!(kind, Node::Text) && optional && at.absent;
            if !is_scalar(kind) && !replaces_object && !creates_text {
                return Err(err("type_mismatch", "set accepts text, scalars, optional values and record entries"));
            }
            kind.validate(value, false)?;
            if !at.absent {
                // A stored value of the wrong type is a preserved anomaly: never overwritten.
                let anomalous = match (&at.value, replaces_object) {
                    (ValueOrContainer::Container(Container::Map(_)), true) => false,
                    (ValueOrContainer::Container(_), _) | (_, true) => true,
                    (ValueOrContainer::Value(stored), false) => {
                        let stored = serde_json::to_value(stored).map_err(engine)?;
                        scalar_issue(kind, &stored) == Some("type_mismatch")
                    }
                };
                if anomalous {
                    return Err(err("type_mismatch", "Cannot edit anomalous field"));
                }
                // Replacing an object must not discard identity-bearing collections.
                if replaces_object && holds_collections(kind) {
                    return Err(err("exists", "Object is already set; edit its fields"));
                }
            }
            let (map, key) = at
                .parent
                .ok_or_else(|| err("type_mismatch", "Cannot replace a row"))?;
            put(&map, &key, kind, value, &writer(doc))?;
        }
        Intent::Clear { .. } => {
            if !matches!(at.node, Node::Optional { .. }) && !at.entry {
                return Err(err("type_mismatch", "Only optional fields and record entries can be cleared"));
            }
            if !at.absent {
                let (map, key) = at
                    .parent
                    .ok_or_else(|| err("type_mismatch", "Cannot clear a row"))?;
                map.delete(&key).map_err(engine)?;
            }
        }
        Intent::Insert {
            id,
            value,
            at: anchor,
            index,
            ..
        } => {
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) =
                (at.node, at.value)
            else {
                return Err(err("type_mismatch", "Expected list"));
            };
            if is_scalar(&item) {
                if id.is_some() || anchor.is_some() {
                    return Err(err("invalid_request", "Scalar lists insert by index"));
                }
                item.validate(value, false)?;
                let index = index.unwrap_or(list.len());
                if index > list.len() {
                    return Err(err("out_of_range", "Insert index is past the end"));
                }
                list.insert(index, loro_scalar(&item, value)).map_err(engine)?;
                return Ok(());
            }
            if index.is_some() {
                return Err(err("invalid_request", "Rows insert by anchor, not index"));
            }
            let id = id.clone().map(Ok).unwrap_or_else(application_id)?;
            match rows.map(doc, &list, &id) {
                Err(e) if e.code == "path_not_found" => {}
                _ => return Err(err("duplicate_id", "Row already exists or is ambiguous")),
            }
            let index = position(&list, anchor, rows)?;
            insert_row(&list, &item, index, &id, value, &writer(doc))?;
            rows.touch(&list);
            ids.push(id);
        }
        Intent::Increment { by, .. } => {
            let (Node::Counter, ValueOrContainer::Container(Container::Map(counter))) =
                (&at.node, &at.value)
            else {
                return Err(err("type_mismatch", "Expected counter"));
            };
            if *by == 0 || !safe(*by) {
                return Err(err("out_of_range", "Increment must be a nonzero safe integer"));
            }
            let raw = serde_json::to_value(counter.get_deep_value()).map_err(engine)?;
            let sum = counter_sum(&raw)
                .ok_or_else(|| err("type_mismatch", "Cannot edit anomalous counter"))?;
            let key = writer(doc);
            let mine = raw.get(&key).and_then(Value::as_i64).unwrap_or(0);
            let next = mine.checked_add(*by).filter(|n| safe(*n));
            let total = sum.checked_add(*by).filter(|n| safe(*n));
            let (Some(next), Some(_)) = (next, total) else {
                return Err(err("out_of_range", "Counter would leave the safe integer range"));
            };
            counter.insert(&key, next).map_err(engine)?;
        }
        Intent::Remove { id, index, count, .. } => {
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) =
                (&at.node, at.value)
            else {
                return Err(err("type_mismatch", "Expected list"));
            };
            if is_scalar(item) {
                let (Some(index), None) = (index, id) else {
                    return Err(err("invalid_request", "Scalar lists remove by index"));
                };
                let count = count.unwrap_or(1);
                if index.checked_add(count).is_none_or(|end| end > list.len()) {
                    return Err(err("out_of_range", "Remove range is past the end"));
                }
                list.delete(*index, count).map_err(engine)?;
                return Ok(());
            }
            let (Some(id), None, None) = (id, index, count) else {
                return Err(err("invalid_request", "Rows are removed by id"));
            };
            list.delete(rows.index(&list, id)?, 1).map_err(engine)?;
            rows.touch(&list);
        }
        Intent::Move { id, at: anchor, .. } => {
            let (Node::List { item }, ValueOrContainer::Container(Container::MovableList(list))) =
                (&at.node, at.value)
            else {
                return Err(err("type_mismatch", "Expected list"));
            };
            if is_scalar(item) {
                return Err(err("type_mismatch", "Scalar lists are edited by index"));
            }
            let from = rows.index(&list, id)?;
            let mut to = position(&list, anchor, rows)?;
            if to > from {
                to -= 1;
            }
            if from != to {
                list.mov(from, to).map_err(engine)?;
                rows.touch(&list);
            }
        }
    }
    Ok(())
}
// Separate name avoids shadowing the splice's insert string in pattern matches.
use insert_unchecked as insert_row;

/// Rewrites a scalar list to `values`, keeping the common prefix and suffix and setting
/// overlapping positions, so concurrent edits outside the changed span survive.
fn rewrite_list(list: &LoroMovableList, item: &Node, values: &[Value]) -> Result<()> {
    let current: Vec<Value> = (0..list.len())
        .map(|i| match list.get(i) {
            Some(ValueOrContainer::Value(v)) => serde_json::to_value(v).unwrap_or(Value::Null),
            _ => Value::Null,
        })
        .map(|v| project(Some(item), v))
        .collect();
    let target: Vec<Value> = values.iter().map(|v| project(Some(item), v.clone())).collect();
    let prefix = current.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let suffix = current[prefix..]
        .iter()
        .rev()
        .zip(target[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (old_mid, new_mid) = (current.len() - prefix - suffix, target.len() - prefix - suffix);
    for offset in 0..old_mid.min(new_mid) {
        if current[prefix + offset] != target[prefix + offset] {
            list.set(prefix + offset, loro_scalar(item, &values[prefix + offset])).map_err(engine)?;
        }
    }
    if old_mid > new_mid {
        list.delete(prefix + new_mid, old_mid - new_mid).map_err(engine)?;
    }
    for offset in old_mid..new_mid {
        list.insert(prefix + offset, loro_scalar(item, &values[prefix + offset])).map_err(engine)?;
    }
    Ok(())
}

/// Each session writes only its own counter contribution: no key ever has
/// concurrent writers, so contributions merge without loss.
fn writer(doc: &LoroDoc) -> String {
    doc.peer_id().to_string()
}
fn raw(doc: &LoroDoc) -> Result<Value> {
    serde_json::to_value(doc.get_map("data").get_deep_value()).map_err(engine)
}
/// A scalar's stored value against its descriptor: `None`, or the issue code.
fn scalar_issue(node: &Node, value: &Value) -> Option<&'static str> {
    let bounded = |n: f64, min: Option<f64>, max: Option<f64>| {
        if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) { Some("out_of_range") } else { None }
    };
    match (node, value) {
        (Node::Boolean, Value::Bool(_)) => None,
        (Node::String { max_length }, Value::String(s)) => {
            max_length.filter(|max| utf16_len(s) > *max).map(|_| "out_of_range")
        }
        (Node::Enum { values }, Value::String(s)) => {
            (!values.iter().any(|v| v == s)).then_some("type_mismatch")
        }
        (Node::Number { min, max }, Value::Number(n)) => match n.as_f64() {
            Some(f) if f.is_finite() => bounded(f, *min, *max),
            _ => Some("type_mismatch"),
        },
        (Node::Integer { min, max }, Value::Number(n)) => match n.as_i64().filter(|n| safe(*n)) {
            Some(i) => bounded(i as f64, min.map(|m| m as f64), max.map(|m| m as f64)),
            None => Some("type_mismatch"),
        },
        _ => Some("type_mismatch"),
    }
}
fn issues(node: &Node, value: &Value, path: &mut Vec<Value>, result: &mut Vec<Value>) {
    match (node, value) {
        (Node::Text, Value::String(_)) => {}
        (Node::Optional { inner }, value) => issues(inner, value, path, result),
        (node, value) if is_scalar(node) => {
            if let Some(code) = scalar_issue(node, value) {
                result.push(json!({"code":code,"path":path}));
            }
        }
        (Node::Counter, raw @ Value::Object(_)) => {
            if counter_sum(raw).is_none() {
                result.push(json!({"code":"type_mismatch","path":path}));
            }
        }
        (Node::Object { properties }, Value::Object(map)) => {
            for (key, child) in properties {
                let value = map.get(key);
                // An absent optional is valid; a stored null is not.
                if value.is_none() && matches!(child, Node::Optional { .. }) {
                    continue;
                }
                path.push(json!(key));
                issues(child, value.unwrap_or(&Value::Null), path, result);
                path.pop();
            }
            for key in map.keys().filter(|key| key.as_str() != "$id" && !properties.contains_key(*key)) {
                path.push(json!(key));
                result.push(json!({"code":"unknown_field","path":path}));
                path.pop();
            }
        }
        (Node::List { item }, Value::Array(elements)) if is_scalar(item) => {
            for (i, element) in elements.iter().enumerate() {
                if let Some(code) = scalar_issue(item, element) {
                    path.push(json!(i));
                    result.push(json!({"code":code,"path":path}));
                    path.pop();
                }
            }
        }
        (Node::Record { value: entry }, Value::Object(map)) => {
            for (key, value) in map {
                path.push(json!(key));
                if valid_key(key) {
                    issues(entry, value, path, result);
                } else {
                    result.push(json!({"code":"invalid_key","path":path}));
                }
                path.pop();
            }
        }
        (Node::List { item }, Value::Array(rows)) => {
            let mut seen = BTreeSet::new();
            for (i, row) in rows.iter().enumerate() {
                path.push(json!(i));
                match row.get("$id").and_then(Value::as_str) {
                    Some(id) if valid_id(id) => {
                        if !seen.insert(id) {
                            result.push(json!({"code":"duplicate_id","path":path}));
                        }
                    }
                    _ => result.push(json!({"code":"invalid_id","path":path})),
                }
                issues(item, row, path, result);
                path.pop();
            }
        }
        _ => result.push(json!({"code":"type_mismatch","path":path})),
    }
}
fn container_issues(
    node: &Node,
    value: Option<ValueOrContainer>,
    path: &mut Vec<Value>,
    result: &mut Vec<Value>,
) -> Result<()> {
    match (node, value) {
        (Node::Text, Some(ValueOrContainer::Container(Container::Text(_)))) => {}
        (Node::Optional { .. }, None) => {}
        (Node::Optional { inner }, value @ Some(_)) => container_issues(inner, value, path, result)?,
        (Node::Object { properties }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            for (key, child) in properties {
                path.push(json!(key));
                container_issues(child, map.get(key), path, result)?;
                path.pop();
            }
            let mut keys: Vec<_> = map.keys().map(|key| key.to_string()).collect();
            keys.sort();
            for key in keys.iter().filter(|key| key.as_str() != "$id" && !properties.contains_key(*key)) {
                path.push(json!(key));
                result.push(json!({"code":"unknown_field","path":path}));
                path.pop();
            }
        }
        // A scalar list is a movable list of plain values; its elements are checked as JSON.
        (Node::List { item }, Some(ValueOrContainer::Container(Container::MovableList(list)))) if is_scalar(item) => {
            let json = serde_json::to_value(list.get_deep_value()).map_err(engine)?;
            issues(node, &json, path, result);
        }
        (Node::List { item }, Some(ValueOrContainer::Container(_))) if is_scalar(item) => {
            result.push(json!({"code":"type_mismatch","path":path}));
        }
        (Node::Record { value: entry }, Some(ValueOrContainer::Container(Container::Map(map)))) => {
            let mut keys: Vec<_> = map.keys().map(|key| key.to_string()).collect();
            keys.sort();
            for key in keys {
                path.push(json!(key));
                if valid_key(&key) {
                    container_issues(entry, map.get(&key), path, result)?;
                } else {
                    result.push(json!({"code":"invalid_key","path":path}));
                }
                path.pop();
            }
        }
        (Node::List { item }, Some(ValueOrContainer::Container(Container::MovableList(list)))) => {
            let mut seen = HashSet::new();
            for i in 0..list.len() {
                path.push(json!(i));
                let row = list.get(i);
                let id = match &row {
                    Some(ValueOrContainer::Container(Container::Map(map))) => match map.get("$id") {
                        Some(ValueOrContainer::Value(loro::LoroValue::String(id))) => {
                            Some(id.to_string())
                        }
                        _ => None,
                    },
                    // A plain value (or other container) row: defer to the JSON oracle below.
                    Some(other) => serde_json::to_value(other.get_deep_value())
                        .map_err(engine)?
                        .get("$id")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    None => None,
                };
                match id {
                    Some(id) if valid_id(&id) => {
                        if !seen.insert(id) {
                            result.push(json!({"code":"duplicate_id","path":path}));
                        }
                    }
                    _ => result.push(json!({"code":"invalid_id","path":path})),
                }
                container_issues(item, row, path, result)?;
                path.pop();
            }
        }
        // Plain values and unusual container kinds: exact JSON semantics, small or rare.
        (node, value) => {
            let json = match value {
                Some(v) => serde_json::to_value(v.get_deep_value()).map_err(engine)?,
                None => Value::Null,
            };
            issues(node, &json, path, result);
        }
    }
    Ok(())
}
fn subscribe(doc: &LoroDoc, events: &Events) {
    // The subscription lives exactly as long as this LoroDoc; a replaced doc drops it.
    publication::subscribe(doc, events).detach();
}

/// Identifies the core build (a hash of its sources and the lockfile), so a release can
/// prove the app and its helper embed the same core.
pub const BUILD_ID: &str = env!("HITSLOP_CORE_BUILD_ID");

/// A committed batch: its publication sequence, the IDs of inserted rows, and the
/// publication to deliver.
#[derive(Debug)]
pub struct Applied {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub publication: String,
}
/// A stateless text edit. `authored` is the version right after this edit on its own
/// branch; the page sends it as the next `base`. Selections are UTF-16 offsets in the
/// merged text. A caret-only request publishes nothing.
#[derive(Debug)]
pub struct TextEdit {
    pub sequence: u64,
    pub authored: String,
    pub selection_start: usize,
    pub selection_end: usize,
    pub publication: Option<String>,
}

/// Exactly one host executor owns this value. Neither binding contains semantics.
pub struct Document {
    doc: LoroDoc,
    schema: Node,
    sequence: u64,
    /// Every movable list's order and row identities as of the last publication.
    lists: HashMap<ContainerID, ListState>,
    /// Issues as of the last publication. Empty is the common case and enables
    /// change-proportional validation; a document with anomalies rescans on publish.
    issues: Vec<Value>,
    events: Events,
}
impl Document {
    fn from_doc(doc: LoroDoc, schema: Node) -> Result<Self> {
        let events = Events::default();
        subscribe(&doc, &events);
        let mut this = Self {
            lists: publication::index_all(&doc),
            doc,
            schema,
            sequence: 0,
            issues: vec![],
            events,
        };
        this.issues = this.scan_issues()?;
        Ok(this)
    }
    pub fn create(schema: &str, initial: &str) -> Result<Self> {
        let schema = descriptor(schema)?;
        let initial: Value = parse(initial)?;
        schema.validate(&initial, false)?;
        let doc = LoroDoc::new();
        fill(&doc.get_map("data"), &schema, &initial, &writer(&doc))?;
        doc.commit();
        Self::from_doc(doc, schema)
    }
    pub fn open(schema: &str, checkpoint: &[u8], updates: &[Vec<u8>]) -> Result<Self> {
        let schema = descriptor(schema)?;
        let total = updates
            .iter()
            .try_fold(checkpoint.len(), |n, b| n.checked_add(b.len()))
            .ok_or_else(|| err("too_large", "Input bytes"))?;
        if total > MAX_BYTES {
            return Err(err("too_large", "Input bytes"));
        }
        let doc = LoroDoc::new();
        checked_import(&doc, checkpoint)?;
        for bytes in updates {
            checked_import(&doc, bytes)?;
        }
        Self::from_doc(doc, schema)
    }
    /// Same result as `issues` over the full JSON value, without materializing it:
    /// containers are walked directly and only plain or unexpected values become JSON.
    fn scan_issues(&self) -> Result<Vec<Value>> {
        let mut found = vec![];
        let root = ValueOrContainer::Container(Container::Map(self.doc.get_map("data")));
        container_issues(&self.schema, Some(root), &mut vec![], &mut found)?;
        Ok(found)
    }
    pub fn version(&self) -> String {
        version_token(&self.doc.oplog_frontiers())
    }
    pub fn snapshot(&self) -> Result<String> {
        // Deliberately recomputed from the full value: this is the oracle that
        // incremental publications and issues are tested against.
        let value = raw(&self.doc)?;
        let mut found = vec![];
        issues(&self.schema, &value, &mut vec![], &mut found);
        let value = project_at(&self.doc, Some(&self.schema), &self.doc.get_map("data").id())?;
        encode(
            &json!({"version":self.version(),"value":value,"issues":found,"sequence":self.sequence}),
        )
    }
    pub fn apply(&mut self, batch: &str) -> Result<String> {
        self.apply_batch(batch).map(|applied| applied.publication)
    }
    /// Loro transactions cannot be rolled back. Validation happens before each
    /// intent's first mutation, so a batch rejected at its first intent left nothing
    /// pending. When an earlier intent already mutated, rebuild the owner at the
    /// pre-batch frontiers (O(document), only on this rejection path). The pending
    /// operations were never exported; the rebuilt replica uses a fresh peer.
    fn abort(&mut self, before: &loro::Frontiers) -> Result<()> {
        if self.doc.get_pending_txn_len() == 0 {
            return Ok(());
        }
        let fresh = self.doc.fork_at(before).map_err(engine)?;
        self.events.lock().unwrap().clear();
        subscribe(&fresh, &self.events);
        // Container IDs survive the snapshot, so the published list indexes stay valid.
        self.doc = fresh;
        Ok(())
    }
    pub fn import(&mut self, bytes: &[u8]) -> Result<String> {
        if bytes.len() > MAX_BYTES {
            return Err(err("too_large", "Import bytes"));
        }
        // Refuse a batch with missing dependencies before Loro buffers any of it.
        let meta = LoroDoc::decode_import_blob_meta(bytes, true)
            .map_err(|e| err("invalid_bytes", e))?;
        let known = self.doc.oplog_vv();
        if meta
            .partial_start_vv
            .iter()
            .any(|(peer, start)| known.get(peer).copied().unwrap_or(0) < *start)
        {
            return Err(err(
                "missing_dependencies",
                "Durable pending-import buffering is not implemented",
            ));
        }
        self.doc
            .import(bytes)
            .map_err(|e| err("invalid_bytes", e))?;
        self.publish()
    }
    /// The publication sequence: the number of published changes since open.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Like `apply`, with the result as a record so hosts never parse the reply.
    pub fn apply_batch(&mut self, batch: &str) -> Result<Applied> {
        let batch: Batch = parse(batch)?;
        if batch.intents.len() > 1000 {
            return Err(err("too_large", "Batch exceeds 1000 intents"));
        }
        let before = self.doc.state_frontiers();
        let mut ids = vec![];
        let mut failure = None;
        {
            let mut rows = Rows::new(&self.lists);
            for (index, op) in batch.intents.iter().enumerate() {
                if let Err(mut e) = execute(&self.doc, &self.schema, op, &mut ids, &mut rows)
                {
                    e.op_index = Some(index);
                    failure = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = failure {
            self.abort(&before)?; // Atomicity sensitivity removes only this call in a disposable copy.
            return Err(e);
        }
        self.doc.commit();
        let publication = self.publish()?;
        Ok(Applied { sequence: self.sequence, ids, publication })
    }
    /// Publishes the committed events as one change: `{previous, sequence, version, ops,
    /// issues}`. Applying it to the previous snapshot yields a fresh snapshot.
    fn publish(&mut self) -> Result<String> {
        let events = std::mem::take(&mut *self.events.lock().unwrap());
        let published = publication::publish(&self.doc, &self.schema, &mut self.lists, events)?;
        if published.rescan || !self.issues.is_empty() {
            self.issues = self.scan_issues()?;
        }
        let next = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| err("too_large", "Publication sequence"))?;
        let response = encode(&json!({"previous":self.sequence,"sequence":next,"version":self.version(),
            "ops":published.ops,"issues":self.issues}))?;
        self.sequence = next;
        Ok(response)
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        self.doc.export(ExportMode::Snapshot).map_err(engine)
    }
    pub fn export_since(&self, version: &str) -> Result<Vec<u8>> {
        let (_, vv) = decode_version(&self.doc, version)?;
        self.doc.export(ExportMode::updates(&vv)).map_err(engine)
    }
}
fn checked_import(doc: &LoroDoc, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_BYTES {
        return Err(err("too_large", "Import bytes"));
    }
    let status = doc.import(bytes).map_err(|e| err("invalid_bytes", e))?;
    if status.pending.as_ref().is_some_and(|v| !v.is_empty()) {
        return Err(err(
            "missing_dependencies",
            "Durable pending-import buffering is not implemented",
        ));
    }
    Ok(())
}
