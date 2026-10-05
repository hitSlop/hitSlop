#![allow(dead_code)]
use hitslop_core::{Document, Error};
use serde_json::{json, Value};
use hitslop_core::Origin;
/// The writer-lock registry test runs use, so they never fill `~/.hitslop/live`.
pub fn registry_folder() -> std::path::PathBuf {
    std::env::temp_dir().join("hitslop-test-registry")
}
/// Points this process's registry at `registry_folder`; every test that takes a writer
/// lock calls it first.
#[cfg(feature = "storage")]
pub fn isolate_registry() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| hitslop_core::registry::use_folder(&registry_folder()).unwrap());
}
/// This test binary running only its ignored test `name`, as a separate process, with
/// `env`: a child test returns at once unless its variables are set. Output is discarded.
pub fn child(name: &str, env: &[(&str, &str)]) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--ignored", "--nocapture"]);
    command.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    command
}
/// A fixture app's manifest and declared colors.
pub const MANIFEST: &str = r#"{"author":{"name":"Fixture"},"slug":"checklist","title":"Checklist","description":"A test document.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#;
pub const THEME: &str = r##"{"accent":"#335577"}"##;
/// A build's `app.json`: the `app` row, each part the JSON text a build writes.
#[derive(Clone, Copy)]
pub struct App<'a> {
    pub format: u64,
    pub abi: u64,
    pub manifest: &'a str,
    pub descriptor: &'a str,
    pub initial: &'a str,
    pub theme: &'a str,
}
impl<'a> App<'a> {
    /// The fixture app with this descriptor and initial values, for this build's markers.
    pub const fn new(descriptor: &'a str, initial: &'a str) -> Self {
        App { format: hitslop_core::PACKAGE_FORMAT, abi: hitslop_core::RUNTIME_ABI, manifest: MANIFEST, descriptor, initial, theme: THEME }
    }
}
/// Writes `app` as the stage's `app.json`.
pub fn write_app(stage: &std::path::Path, app: App) {
    let App { format, abi, manifest, descriptor, initial, theme } = app;
    let json = format!(r#"{{"packageFormat":{format},"runtimeABI":{abi},"manifest":{manifest},"descriptor":{descriptor},"initial":{initial},"theme":{theme}}}"#);
    std::fs::write(stage.join("app.json"), json).unwrap();
}
/// A shared fixture (`fixtures/<name>.json`): a descriptor, its initial values and, for
/// some, conformance cases.
pub fn fixture(name: &str) -> Value {
    serde_json::from_str(match name {
        "checklist" => include_str!("../../fixtures/checklist.json"),
        "collections" => include_str!("../../fixtures/collections.json"),
        "scalars" => include_str!("../../fixtures/scalars.json"),
        "nested" => include_str!("../../fixtures/nested.json"),
        other => panic!("no fixture {other}"),
    })
    .unwrap()
}
/// A document's snapshot (value, issues and version), parsed.
pub fn snapshot(d: &Document) -> Value {
    serde_json::from_str(&d.snapshot().unwrap()).unwrap()
}
/// A document's value.
pub fn value(d: &Document) -> Value {
    snapshot(d)["value"].clone()
}
/// Two replicas of a fixture's document that edit concurrently, then `exchange` updates,
/// and the version they share.
pub fn pair(f: &Value) -> (Document, Document, String) {
    let a = Document::create(&f["schema"].to_string(), &f["initial"].to_string()).unwrap();
    let b = Document::open(&f["schema"].to_string(), &a.checkpoint().unwrap(), &[]).unwrap();
    let base = a.version();
    (a, b, base)
}
/// Each replica imports what the other wrote since `base`.
pub fn exchange(a: &mut Document, b: &mut Document, base: &str) {
    let (left, right) = (a.export_since(base).unwrap(), b.export_since(base).unwrap());
    a.import(&right).unwrap();
    b.import(&left).unwrap();
}
/// A checkpoint saved again without its history, as trimming does.
pub fn trimmed(checkpoint: &[u8]) -> Vec<u8> {
    let loro = loro::LoroDoc::new();
    loro.import(checkpoint).unwrap();
    loro.export(loro::ExportMode::shallow_snapshot(&loro.oplog_frontiers())).unwrap()
}
/// The next number of a xorshift sequence: seeded workloads that replay exactly.
pub fn next(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}
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
        self.apply_batch(batch, Origin::Page).map(|applied| unchanged(applied.publication))
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
    pub theme: Value,
}
impl View {
    pub fn of(doc: &Document) -> Self {
        let state: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        Self { value: state["value"].clone(), issues: state["issues"].clone(), theme: state["theme"].clone() }
    }
    pub fn publish(&mut self, publication: &str) {
        let publication: Value = serde_json::from_str(publication).unwrap();
        apply_patches(&mut self.value, &publication["ops"]);
        if let Some(issues) = publication.get("issues") {
            self.issues = issues.clone();
        }
        if let Some(theme) = publication.get("theme") {
            self.theme = theme.clone();
        }
    }
    /// Equal to a fresh snapshot, value and issues.
    pub fn check(&self, doc: &Document, context: &str) {
        let fresh: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        assert_eq!(self.value, fresh["value"], "{context}: projection diverged");
        assert_eq!(self.issues, fresh["issues"], "{context}: issues diverged");
        assert_eq!(self.theme, fresh["theme"], "{context}: theme diverged");
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
