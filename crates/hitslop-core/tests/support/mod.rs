#![allow(dead_code)]
use hitslop_core::{AppSpec, Document, Error};
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
/// An app of this descriptor (JSON) that declares no colors.
pub fn app(schema: impl AsRef<str>) -> AppSpec {
    AppSpec::data(schema.as_ref()).unwrap()
}
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
/// A document's snapshot (value, theme and version), parsed.
pub fn snapshot(d: &Document) -> Value {
    serde_json::from_str(&d.snapshot().unwrap()).unwrap()
}
/// A document's value.
pub fn value(d: &Document) -> Value {
    snapshot(d)["value"].clone()
}
/// The updates `d`'s history holds beyond `seed`, as a save appends them.
pub fn updates_since(seed: &[u8], d: &Document) -> Vec<u8> {
    let (base, full) = (loro::LoroDoc::new(), loro::LoroDoc::new());
    base.import(seed).unwrap();
    full.import(&d.checkpoint().unwrap()).unwrap();
    full.export(loro::ExportMode::updates(&base.oplog_vv())).unwrap()
}
/// Whether `d` still accepts `version` as a text base: a no-change edit of `["title"]`
/// (holding `text`) from it is refused as stale or invalid otherwise.
pub fn knows(d: &mut Document, version: &str, text: &str) -> bool {
    let request = json!({"base":version,"path":["title"],"from":text,"to":text,"selectionStart":0,"selectionEnd":0});
    match d.edit_text(&request.to_string()) {
        Ok(_) => true,
        Err(e) => {
            assert!(["stale_base", "invalid_version"].contains(&e.code.as_str()), "{}", e.code.as_str());
            false
        }
    }
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
}
fn unchanged(publication: Option<String>) -> String {
    publication.unwrap_or_else(|| json!({ "ops": [] }).to_string())
}
impl Edit for Document {
    fn apply(&mut self, batch: &str) -> Result<String, Error> {
        self.apply_batch(batch, Origin::Page).map(|applied| unchanged(applied.publication))
    }
}
/// The page's view of a document: publications apply their ops, and replace the palette
/// when they carry it.
pub struct View {
    pub value: Value,
    pub theme: Value,
}
impl View {
    pub fn of(doc: &Document) -> Self {
        let state: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        Self { value: state["value"].clone(), theme: state["theme"].clone() }
    }
    pub fn publish(&mut self, publication: &str) {
        let publication: Value = serde_json::from_str(publication).unwrap();
        apply_patches(&mut self.value, &publication["ops"]);
        if let Some(theme) = publication.get("theme") {
            self.theme = theme.clone();
        }
    }
    /// Equal to a fresh snapshot, value and palette.
    pub fn check(&self, doc: &Document, context: &str) {
        let fresh: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        assert_eq!(self.value, fresh["value"], "{context}: projection diverged");
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
