//! Command arguments use a restricted document descriptor, checked by the same core.
//! JSON Schema is a projection for tool clients, never an accepted contract or checker.
use crate::{Code, MAX_SAFE, Node, Result, descriptor, err, is_scalar, json, parse, valid_id};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Arguments {
    node: Node,
    descriptor: Value,
    rows: Vec<Row>,
}

/// `s.row("tasks")`: the `$id` of a row in one of the document's top-level object lists.
/// The core checks its spelling; the command program resolves it against the snapshot.
#[derive(Clone, Debug)]
struct Row {
    /// Field names from the argument object down to the row argument.
    path: Vec<String>,
    list: String,
}

impl Arguments {
    pub fn parse(input: &str) -> Result<Self> {
        let descriptor: Value = parse(input)?;
        let mut rows = vec![];
        let mut checked = descriptor.clone();
        row_nodes(&mut checked, &mut vec![], &mut rows, true)?;
        let node = descriptor::descriptor_value(checked)?;
        subset(&node)?;
        Ok(Self { node, descriptor, rows })
    }

    pub fn descriptor(&self) -> &Value {
        &self.descriptor
    }

    /// The lists row arguments name; the app checks each against its document.
    pub fn row_lists(&self) -> impl Iterator<Item = &str> {
        self.rows.iter().map(|row| row.list.as_str())
    }

    /// Whether `value` is accepted once its omitted defaults are filled.
    pub fn validate(&self, value: &Value) -> Result<()> {
        self.prepare(&mut value.clone())
    }

    /// Fills omitted defaults into `value`, then checks it.
    pub fn prepare(&self, value: &mut Value) -> Result<()> {
        self.node.fill_defaults(value);
        self.node.validate(value, false)?;
        for row in &self.rows {
            let found = row.path.iter().try_fold(&*value, |value, name| value.get(name));
            if let Some(id) = found
                && !id.as_str().is_some_and(valid_id)
            {
                return Err(at(err(Code::InvalidId, format!("Expected the $id of a row in {}", row.list)), &row.path));
            }
        }
        Ok(())
    }

    /// Output only. Includes implicit core bounds as well as authored constraints.
    pub fn json_schema(&self) -> Value {
        project(&self.node, &self.descriptor)
    }
}

/// Replaces each row node with the string it is checked as, recording where it is.
fn row_nodes(node: &mut Value, path: &mut Vec<String>, rows: &mut Vec<Row>, required: bool) -> Result<()> {
    let Some(map) = node.as_object_mut() else { return Ok(()) };
    match map.get("kind").and_then(Value::as_str) {
        Some("row") => {
            if path.len() != 1 || !required {
                return Err(at(err(Code::InvalidSchema, "A row argument must be a required top-level field"), path));
            }
            let list = map.get("list").and_then(Value::as_str).unwrap_or_default().to_string();
            if list.is_empty() || map.keys().any(|key| !["kind", "list", "description"].contains(&key.as_str())) {
                return Err(at(err(Code::InvalidSchema, "A row argument is s.row(listName)"), path));
            }
            rows.push(Row { path: path.clone(), list });
            map.remove("list");
            map.insert("kind".into(), "string".into());
        }
        Some("object") => {
            if let Some(Value::Object(properties)) = map.get_mut("properties") {
                for (name, child) in properties {
                    path.push(name.clone());
                    row_nodes(child, path, rows, true)?;
                    path.pop();
                }
            }
        }
        Some("optional") => {
            if let Some(inner) = map.get_mut("inner") {
                row_nodes(inner, path, rows, false)?;
            }
        }
        Some("list") if map.get("item").and_then(|item| item.get("kind")).and_then(Value::as_str) == Some("row") => {
            return Err(at(err(Code::InvalidSchema, "A row argument names one row"), path));
        }
        // A row inside a list item or a record value is refused at its own path, not later
        // by the descriptor parser, which knows no row kind.
        Some("list") | Some("record") => {
            for key in ["item", "value"] {
                if let Some(child) = map.get_mut(key) {
                    path.push(key.into());
                    row_nodes(child, path, rows, false)?;
                    path.pop();
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn at(error: crate::Error, path: &[String]) -> crate::Error {
    path.iter().rev().fold(error, |error, name| error.at(name))
}

fn subset(node: &Node) -> Result<()> {
    match node {
        scalar if is_scalar(scalar) => Ok(()),
        Node::Object { properties } => {
            properties.iter().try_for_each(|(name, node)| subset(node).map_err(|e| e.at(name)))
        }
        Node::Optional { inner } => subset(inner),
        Node::List { item } if is_scalar(item) => Ok(()),
        Node::Text {} => Err(err(Code::InvalidSchema, "use s.string() for command arguments")),
        Node::Counter {} => Err(err(Code::InvalidSchema, "use s.integer() for command arguments")),
        Node::Record { .. } => Err(err(Code::InvalidSchema, "Records are not command arguments")),
        Node::List { .. } => Err(err(Code::InvalidSchema, "Command argument lists contain scalars only")),
        _ => unreachable!("all scalar kinds handled above"),
    }
}

fn project(node: &Node, source: &Value) -> Value {
    let mut schema = match node {
        // A row argument is checked as an application ID.
        Node::String { .. } if source["kind"] == "row" => json!({
            "type":"string", "minLength":1, "maxLength":64, "pattern":"^[A-Za-z0-9_-]+$",
            "description": format!("The $id of a row in {}", source["list"].as_str().unwrap_or_default()),
        }),
        Node::Boolean { .. } => json!({"type":"boolean"}),
        Node::String { min_length, max_length, .. } => {
            let mut value = json!({"type":"string"});
            if let Some(min) = min_length {
                value["minLength"] = (*min).into();
            }
            if let Some(max) = max_length {
                value["maxLength"] = (*max).into();
            }
            value
        }
        Node::Number { min, max, .. } => {
            let mut value = json!({"type":"number"});
            if let Some(min) = min {
                value["minimum"] = (*min).into();
            }
            if let Some(max) = max {
                value["maximum"] = (*max).into();
            }
            value
        }
        Node::Integer { min, max, .. } => {
            json!({"type":"integer","minimum":min.unwrap_or(-MAX_SAFE),"maximum":max.unwrap_or(MAX_SAFE)})
        }
        Node::Enum { values, .. } => json!({"type":"string","enum":values}),
        Node::Optional { inner } => project(inner, &source["inner"]),
        Node::Object { properties } => {
            let mut required = vec![];
            let mut projected = serde_json::Map::new();
            for (key, child) in properties {
                projected.insert(key.clone(), project(child, &source["properties"][key]));
                if !child.omittable() {
                    required.push(key);
                }
            }
            let mut value = json!({"type":"object","properties":projected,"additionalProperties":false});
            if !required.is_empty() {
                value["required"] = json!(required);
            }
            value
        }
        Node::List { item } => json!({
            "type":"array", "items":project(item, &source["item"]),
            "maxItems":descriptor::MAX_SCALAR_LIST,
        }),
        _ => unreachable!("argument subset checked at construction"),
    };
    if let Some(default) = descriptor::declared(node) {
        schema["default"] = default;
    }
    if let Some(description) = source.get("description") {
        schema["description"] = description.clone();
    }
    schema
}
