#![allow(dead_code)]
use hitslop_core::{Document, Error};
use serde_json::{json, Value};
/// Keep the everyday tier bounded; use the same knobs for extended stress runs.
pub fn workload(name: &str, default: usize) -> usize {
    std::env::var(name).map(|v| v.parse::<usize>().expect("positive test workload")).unwrap_or(default).max(1)
}
/// Publications as JSON text; a change that altered nothing reads as `{"ops": []}`.
pub trait Edit {
    fn apply(&mut self, batch: &str) -> Result<String, Error>;
    fn merge(&mut self, bytes: &[u8]) -> Result<String, Error>;
}
fn unchanged(publication: Option<String>) -> String {
    publication.unwrap_or_else(|| json!({ "ops": [] }).to_string())
}
impl Edit for Document {
    fn apply(&mut self, batch: &str) -> Result<String, Error> {
        self.apply_batch(batch).map(|applied| unchanged(applied.publication))
    }
    fn merge(&mut self, bytes: &[u8]) -> Result<String, Error> {
        self.import(bytes).map(unchanged)
    }
}
/// The page's view of a document: publications apply their ops, and replace the issues
/// when they carry them.
pub struct View {
    pub value: Value,
    pub issues: Value,
}
impl View {
    pub fn of(doc: &Document) -> Self {
        let state: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        Self { value: state["value"].clone(), issues: state["issues"].clone() }
    }
    pub fn publish(&mut self, publication: &str) {
        let publication: Value = serde_json::from_str(publication).unwrap();
        apply_patches(&mut self.value, &publication["ops"]);
        if let Some(issues) = publication.get("issues") {
            self.issues = issues.clone();
        }
    }
    /// Equal to a fresh snapshot, value and issues.
    pub fn check(&self, doc: &Document, context: &str) {
        let fresh: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        assert_eq!(self.value, fresh["value"], "{context}: projection diverged");
        assert_eq!(self.issues, fresh["issues"], "{context}: issues diverged");
    }
}
// Independent test consumer, not the publisher implementation.
pub fn apply_patches(value: &mut Value, ops: &Value) {
    for op in ops.as_array().unwrap() {
        let path = op["path"].as_array().unwrap();
        let mut target = &mut *value;
        let walk = if op["type"] == "remove" { &path[..path.len() - 1] } else { &path[..] };
        for segment in walk {
            if let Some(key) = segment.as_str() {
                target = &mut target[key];
            } else if let Some(index) = segment.get("index") {
                target = &mut target[index.as_u64().unwrap() as usize];
            } else {
                target = target
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|v| v["$id"] == segment["id"])
                    .unwrap();
            }
        }
        match op["type"].as_str().unwrap() {
            "set" => *target = op["value"].clone(),
            "text" => {
                // Hunks count Unicode code points of the previous text.
                let old: Vec<char> = target.as_str().unwrap().chars().collect();
                let (mut next, mut at) = (String::new(), 0);
                for hunk in op["delta"].as_array().unwrap() {
                    if let Some(n) = hunk["retain"].as_u64() {
                        next.extend(&old[at..at + n as usize]);
                        at += n as usize;
                    } else if let Some(n) = hunk["delete"].as_u64() {
                        at += n as usize;
                    } else {
                        next.push_str(hunk["insert"].as_str().unwrap());
                    }
                }
                next.extend(&old[at..]);
                *target = Value::String(next);
            }
            "remove" => {
                target
                    .as_object_mut()
                    .unwrap()
                    .remove(path.last().unwrap().as_str().unwrap());
            }
            "insertRow" => target
                .as_array_mut()
                .unwrap()
                .insert(op["index"].as_u64().unwrap() as usize, op["value"].clone()),
            kind => {
                let rows = target.as_array_mut().unwrap();
                let i = rows.iter().position(|v| v["$id"] == op["id"]).unwrap();
                let row = rows.remove(i);
                if kind == "moveRow" {
                    rows.insert(op["index"].as_u64().unwrap() as usize, row);
                } else {
                    assert_eq!(kind, "deleteRow");
                }
            }
        }
    }
}
