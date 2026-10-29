//! Read-only discovery shared by command-line presentations. Descriptor metadata stays
//! intact; operations describe the actual storage kinds rather than another schema DSL.
use serde_json::{Value, json};

pub fn describe(metadata: Value, state: Value, commands: Value) -> Value {
    let mut fields = Vec::new();
    fn visit(node: &Value, path: Vec<Value>, fields: &mut Vec<Value>) {
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
            fields.push(json!({"path":path,"kind":kind,"description":node.get("description"),"operations":operations}));
        }
        match kind {
            "object" => {
                if let Some(properties) = node["properties"].as_object() {
                    for (key, child) in properties {
                        let mut at = path.clone();
                        at.push(key.clone().into());
                        visit(child, at, fields);
                    }
                }
            }
            "optional" => visit(&node["inner"], path, fields),
            "list" => {
                let mut at = path;
                at.push(if node["item"]["kind"] == "object" { json!({"id":"$id"}) } else { json!({"index":0}) });
                visit(&node["item"], at, fields);
            }
            "record" => {
                let mut at = path;
                at.push("<key>".into());
                visit(&node["value"], at, fields);
            }
            _ => {}
        }
    }
    visit(&state["schema"], vec![], &mut fields);
    json!({"metadata":metadata,"schema":state["schema"],"version":state["version"],"value":state["value"],"theme":state["theme"],"fields":fields,"commands":commands})
}
