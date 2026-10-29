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

    /// Whether `value` is accepted once its omitted defaults are filled.
    pub fn validate(&self, value: &Value) -> Result<()> {
        self.prepare(&mut value.clone())
    }

    /// Fills omitted defaults into `value`, then checks it.
    pub fn prepare(&self, value: &mut Value) -> Result<()> {
        self.node.fill_defaults(value);
        self.node.validate(value, false)?;
        Ok(())
    }

    /// Output only. Includes implicit core bounds as well as authored constraints.
    pub fn json_schema(&self) -> Value {
        project(&self.node, &self.descriptor)
    }
}

/// Only a flat object of scalars (optionally absent) crosses the command boundary.
fn subset(node: &Node) -> Result<()> {
    let Node::Object { properties } = node else {
        return Err(err(Code::InvalidSchema, "Command arguments must be an object"));
    };
    for (name, node) in properties {
        let scalar = match node {
            Node::Optional { inner } => inner,
            node => node,
        };
        if !is_scalar(scalar) {
            return Err(err(Code::InvalidSchema, "Command arguments must be scalars or optional scalars").at(name));
        }
    }
    Ok(())
}

fn project(node: &Node, source: &Value) -> Value {
    let mut schema = match node {
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
