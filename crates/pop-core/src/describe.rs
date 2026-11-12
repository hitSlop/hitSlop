//! Read-only discovery shared by command-line presentations. Descriptor metadata stays
//! intact; operations describe the actual storage kinds rather than another schema DSL.
use crate::{app::AppDefinition, descriptor::Node};
use pop_contracts::wire::engine::{DescribeState, DescribedField};
use pop_contracts::wire::{Reading, Segment};

pub fn describe(app: &AppDefinition, reading: Reading) -> DescribeState {
    let mut fields = Vec::new();
    fn visit(node: &Node, path: Vec<Segment>, fields: &mut Vec<DescribedField>) {
        let (kind, operations): (&str, &[&str]) = match node {
            Node::Counter { .. } => ("counter", &["increment"]),
            Node::List { item, .. } if matches!(**item, Node::Object { .. }) => {
                ("list", &["insert", "remove", "move", "replace"])
            }
            Node::List { .. } => ("list", &["insert", "remove", "replace"]),
            Node::Record { .. } => ("record", &["set", "clear", "replace"]),
            Node::Optional { .. } => ("optional", &["set", "clear"]),
            Node::Object { .. } => ("object", &["replace"]),
            Node::Text { .. } => ("text", &["set"]),
            Node::Boolean { .. } => ("boolean", &["set"]),
            Node::String { .. } => ("string", &["set"]),
            Node::Number { .. } => ("number", &["set"]),
            Node::Integer { .. } => ("integer", &["set"]),
            Node::Enum { .. } => ("enum", &["set"]),
        };
        if !path.is_empty() {
            fields.push(DescribedField {
                path: path.clone(),
                kind: kind.into(),
                description: node.description().map(str::to_owned),
                operations: operations.iter().map(|operation| (*operation).into()).collect(),
            });
        }
        match node {
            Node::Object { properties, .. } => {
                for (key, child) in properties {
                    let mut at = path.clone();
                    at.push(Segment::Key(key.clone()));
                    visit(child, at, fields);
                }
            }
            Node::Optional { inner, .. } => visit(inner, path, fields),
            Node::List { item, .. } => {
                let mut at = path;
                at.push(if matches!(**item, Node::Object { .. }) {
                    Segment::Id { id: "$id".into() }
                } else {
                    Segment::Index { index: 0 }
                });
                visit(item, at, fields);
            }
            Node::Record { value, .. } => {
                let mut at = path;
                at.push(Segment::Key("<key>".into()));
                visit(value, at, fields);
            }
            _ => {}
        }
    }
    visit(&app.spec().schema, vec![], &mut fields);
    DescribeState {
        metadata: app.metadata().clone(),
        schema: serde_json::from_str(app.document_json()).expect("accepted descriptor"),
        version: reading.version,
        value: reading.value,
        theme: reading.theme,
        fields,
        commands: app.command_metadata(),
    }
}
