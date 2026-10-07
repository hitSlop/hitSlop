//! Command arguments use a restricted document descriptor, checked by the same core.
//! JSON Schema is a projection for tool clients, never an accepted contract or checker.
use crate::{Code, MAX_SAFE, Node, Result, descriptor, err, is_scalar, json, parse};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Arguments {
    node: Node,
    descriptor: Value,
}

impl Arguments {
    pub fn parse(input: &str) -> Result<Self> {
        let descriptor: Value = parse(input)?;
        let node = descriptor::descriptor_value(descriptor.clone())?;
        subset(&node)?;
        Ok(Self { node, descriptor })
    }

    pub fn descriptor(&self) -> &Value {
        &self.descriptor
    }

    pub fn validate(&self, value: &Value) -> Result<()> {
        self.node.validate(value, false)
    }

    /// Output only. Includes implicit core bounds as well as authored constraints.
    pub fn json_schema(&self) -> Value {
        project(&self.node, &self.descriptor)
    }
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
        Node::Boolean {} => json!({"type":"boolean"}),
        Node::String { min_length, max_length } => {
            let mut value = json!({"type":"string"});
            if let Some(min) = min_length {
                value["minLength"] = (*min).into();
            }
            if let Some(max) = max_length {
                value["maxLength"] = (*max).into();
            }
            value
        }
        Node::Number { min, max } => {
            let mut value = json!({"type":"number"});
            if let Some(min) = min {
                value["minimum"] = (*min).into();
            }
            if let Some(max) = max {
                value["maximum"] = (*max).into();
            }
            value
        }
        Node::Integer { min, max } => {
            json!({"type":"integer","minimum":min.unwrap_or(-MAX_SAFE),"maximum":max.unwrap_or(MAX_SAFE)})
        }
        Node::Enum { values } => json!({"type":"string","enum":values}),
        Node::Optional { inner } => project(inner, &source["inner"]),
        Node::Object { properties } => {
            let mut required = vec![];
            let mut projected = serde_json::Map::new();
            for (key, child) in properties {
                projected.insert(key.clone(), project(child, &source["properties"][key]));
                if !matches!(child, Node::Optional { .. }) {
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
    if let Some(description) = source.get("description") {
        schema["description"] = description.clone();
    }
    schema
}
