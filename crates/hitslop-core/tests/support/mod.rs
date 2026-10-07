#![allow(dead_code)]
pub mod generate;
use hitslop_core::Origin;
use hitslop_core::{AppSpec, Applied, Document, Error};
use serde_json::{Value, json};
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
/// A fixture declaration, independent of source filenames or legacy manifests.
pub const METADATA: &str = r#"{"author":{"name":"Fixture"},"slug":"checklist","title":"Checklist","description":"A test document.","categories":["utilities"]}"#;
pub const WINDOW: &str = r#"{"kind":"standard","width":320,"height":240}"#;
pub const THEME: &str = r##"{"accent":"#335577"}"##;
#[derive(Clone, Copy)]
pub struct App<'a> {
    pub format: u64,
    pub abi: u64,
    pub metadata: &'a str,
    pub window: &'a str,
    pub descriptor: &'a str,
    pub initial: &'a str,
    pub theme: &'a str,
    pub commands: &'a str,
}
impl<'a> App<'a> {
    pub const fn new(descriptor: &'a str, initial: &'a str) -> Self {
        Self {
            format: hitslop_core::PACKAGE_FORMAT,
            abi: hitslop_core::RUNTIME_ABI,
            metadata: METADATA,
            window: WINDOW,
            descriptor,
            initial,
            theme: THEME,
            commands: "[]",
        }
    }
}
/// Test-only IPC fixture. Production passes BuildInput directly to pack, never scans a stage.
pub fn write_app(stage: &std::path::Path, app: App) {
    let parse = |value: &str| serde_json::from_str::<Value>(value).unwrap();
    let theme: serde_json::Map<String, Value> = serde_json::from_str(app.theme).unwrap();
    let input = json!({"packageFormat":app.format,"runtimeABI":app.abi,
        "declaration":{"metadata":parse(app.metadata),"window":parse(app.window),"document":parse(app.descriptor),"initial":parse(app.initial),
            "theme":theme.into_iter().map(|(token,color)|json!({"token":token,"color":color})).collect::<Vec<_>>(),"commands":parse(app.commands),"views":{"export":false,"icon":false}},
        "roles":{"ui":"ui.js"},"resources":[{"kind":"app","key":"ui.js","mediaType":"text/javascript","path":"assets/ui.js"}],"artwork":{}});
    std::fs::write(stage.join("input.json"), input.to_string()).unwrap();
}
#[cfg(feature = "storage")]
pub fn pack(stage: &std::path::Path, destination: &std::path::Path) -> Result<(), hitslop_core::store::Error> {
    hitslop_core::file::pack(&std::fs::read_to_string(stage.join("input.json")).unwrap(), stage, destination)
}
pub fn edit_input(stage: &std::path::Path, edit: impl FnOnce(&mut Value)) {
    let path = stage.join("input.json");
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    edit(&mut value);
    std::fs::write(path, value.to_string()).unwrap();
}
pub fn artwork(stage: &std::path::Path, name: &str, bytes: &[u8]) {
    std::fs::create_dir_all(stage.join("artwork")).unwrap();
    std::fs::write(stage.join(format!("artwork/{name}.png")), bytes).unwrap();
    edit_input(stage, |input| input["artwork"][name] = format!("artwork/{name}.png").into());
}
#[cfg(feature = "storage")]
pub fn add_asset(stage: &std::path::Path, key: &str, media_type: &str, bytes: &[u8]) {
    let path = format!("assets/{key}");
    std::fs::create_dir_all(stage.join(&path).parent().unwrap()).unwrap();
    std::fs::write(stage.join(&path), bytes).unwrap();
    edit_input(stage, |input| {
        let resources = input["resources"].as_array_mut().unwrap();
        resources.retain(|r| r["key"] != key);
        resources.push(
            json!({"kind":if key=="commands.js" {"command"} else {"app"},"key":key,"mediaType":media_type,"path":path}),
        );
        if key == "ui.css" {
            input["roles"]["style"] = key.into();
        }
        if key == "commands.js" {
            input["roles"]["commands"] = key.into();
        }
    });
}
#[cfg(feature = "storage")]
pub fn media_key(bytes: &[u8], extension: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("media/{}.{}", data_encoding::HEXLOWER.encode(&Sha256::digest(bytes)), extension)
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
    serde_json::from_str(&d.state().unwrap()).unwrap()
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
/// The page's text edit as a batch: `path` went from `from`, its text at `base`, to `to`,
/// with the caret at a UTF-16 offset of `to`.
pub fn typed(base: &str, path: Value, from: &str, to: &str, caret: usize) -> String {
    json!({"base":base,"intents":[{"type":"set","path":path,"value":to,"from":from,"selection":{"start":caret,"end":caret}}]})
        .to_string()
}
/// Applies the page's text edit (`typed`).
pub fn type_text(
    d: &mut Document,
    base: &str,
    path: Value,
    from: &str,
    to: &str,
    caret: usize,
) -> Result<Applied, Error> {
    d.apply_batch(&typed(base, path, from, to, caret), Origin::Page)
}
/// Whether `d` still accepts `version` as a text base: a no-change edit of `["title"]`
/// (holding `text`) from it is refused as stale or invalid otherwise.
pub fn knows(d: &mut Document, version: &str, text: &str) -> bool {
    match type_text(d, version, json!(["title"]), text, text, 0) {
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
        let state: Value = serde_json::from_str(&doc.state().unwrap()).unwrap();
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
        let fresh: Value = serde_json::from_str(&doc.state().unwrap()).unwrap();
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
                target = target.as_array_mut().unwrap().iter_mut().find(|v| v["$id"] == segment["id"]).unwrap();
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
                target.as_object_mut().unwrap().remove(path.last().unwrap().as_str().unwrap());
            }
            "insertRow" => {
                target.as_array_mut().unwrap().insert(op["index"].as_u64().unwrap() as usize, op["value"].clone())
            }
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
