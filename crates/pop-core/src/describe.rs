//! Read-only discovery shared by command-line presentations. Descriptor metadata stays
//! intact; operations describe the actual storage kinds rather than another schema DSL.
use pop_contracts::app::AppMetadata;
use pop_contracts::wire::engine::{DescribeState, DescribedField};
use pop_contracts::wire::{Reading, Segment};
use serde_json::Value;

pub fn describe(metadata: AppMetadata, schema: Value, reading: Reading, commands: Value) -> DescribeState {
    let mut fields = Vec::new();
    fn visit(node: &Value, path: Vec<Segment>, fields: &mut Vec<DescribedField>) {
        let kind = node["kind"].as_str().unwrap_or("");
        let operations: &[&str] = match kind {
            "counter" => &["increment"],
            "list" if node["item"]["kind"] == "object" => &["insert", "remove", "move", "replace"],
            "list" => &["insert", "remove", "replace"],
            "record" => &["set", "clear", "replace"],
            "optional" => &["set", "clear"],
            "object" => &["replace"],
            _ => &["set"],
        };
        if !path.is_empty() {
            fields.push(DescribedField {
                path: path.clone(),
                kind: kind.into(),
                description: node.get("description").and_then(Value::as_str).map(str::to_owned),
                operations: operations.iter().map(|operation| (*operation).into()).collect(),
            });
        }
        match kind {
            "object" => {
                if let Some(properties) = node["properties"].as_object() {
                    for (key, child) in properties {
                        let mut at = path.clone();
                        at.push(Segment::Key(key.clone()));
                        visit(child, at, fields);
                    }
                }
            }
            "optional" => visit(&node["inner"], path, fields),
            "list" => {
                let mut at = path;
                at.push(if node["item"]["kind"] == "object" {
                    Segment::Id { id: "$id".into() }
                } else {
                    Segment::Index { index: 0 }
                });
                visit(&node["item"], at, fields);
            }
            "record" => {
                let mut at = path;
                at.push(Segment::Key("<key>".into()));
                visit(&node["value"], at, fields);
            }
            _ => {}
        }
    }
    visit(&schema, vec![], &mut fields);
    DescribeState {
        metadata,
        schema,
        version: reading.version,
        value: reading.value,
        theme: reading.theme,
        fields,
        commands,
    }
}
