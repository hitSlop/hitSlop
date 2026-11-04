#![allow(dead_code, reason = "a build without `storage` uses a subset of these fixtures")]
use serde_json::{Value, json};
/// The writer-lock registry test runs use, so they never fill `~/.pop/live`.
pub fn registry_folder() -> std::path::PathBuf {
    std::env::temp_dir().join("pop-test-registry")
}
/// Points this process's registry at `registry_folder`; every test that takes a writer
/// lock calls it first.
pub fn isolate_registry() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| pop_storage::registry::use_folder(&registry_folder()).unwrap());
}
/// This test binary running only its ignored test `name` (module-qualified: `file::…`), as
/// a separate process, with `env`: a child test returns at once unless its variables are
/// set. Output is discarded.
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
            format: pop_core::PACKAGE_FORMAT,
            abi: pop_core::RUNTIME_ABI,
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
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(
        stage.join("assets/export.js"),
        include_str!("../../../../tests/fixtures/checklist/document/assets/export.js"),
    )
    .unwrap();
    std::fs::copy(stage.join("assets/export.js"), stage.join("assets/preview.js")).unwrap();
    std::fs::copy(stage.join("assets/export.js"), stage.join("assets/icon.js")).unwrap();
    let input = json!({"packageFormat":app.format,"runtimeABI":app.abi,
        "declaration":{"metadata":parse(app.metadata),"window":parse(app.window),"document":parse(app.descriptor),"initial":parse(app.initial),
            "theme":theme.into_iter().map(|(token,color)|json!({"token":token,"color":color})).collect::<Vec<_>>(),"commands":parse(app.commands),"views":{"preview":true,"export":true,"icon":true}},
        "roles":{"ui":"ui.js","preview":"preview.js","icon":"icon.js","export":"export.js"},"resources":[{"kind":"app","key":"ui.js","mediaType":"text/javascript","path":"assets/ui.js"},{"kind":"app","key":"preview.js","mediaType":"text/javascript","path":"assets/preview.js"},{"kind":"app","key":"icon.js","mediaType":"text/javascript","path":"assets/icon.js"},{"kind":"app","key":"export.js","mediaType":"text/javascript","path":"assets/export.js"}],"artwork":{}});
    std::fs::write(stage.join("input.json"), input.to_string()).unwrap();
}
pub fn pack(stage: &std::path::Path, destination: &std::path::Path) -> Result<(), pop_storage::store::Error> {
    pack_rendering(stage, destination, None).map(|_| ())
}
/// Packs, rendering the artwork the stage does not supply with `render`.
pub fn pack_rendering(
    stage: &std::path::Path,
    destination: &std::path::Path,
    render: Option<pop_storage::file::RenderArtwork>,
) -> Result<pop_core::engine::PackedArtwork, pop_storage::store::Error> {
    pop_storage::file::pack(&std::fs::read_to_string(stage.join("input.json")).unwrap(), stage, destination, render)
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
/// A stand-in capture renderer: it reads its job, runs the shell command `before`, then
/// replies with one image, `bytes`, whatever it was asked for.
pub fn renderer(before: &str, bytes: &[u8]) -> pop_runner::Evaluator {
    renderer_outputs(before, bytes, bytes)
}
pub fn renderer_outputs(before: &str, bytes: &[u8], pdf: &[u8]) -> pop_runner::Evaluator {
    let reply = json!({"ok": true, "images": [data_encoding::BASE64.encode(bytes)]});
    let pdf = json!({"ok":true,"images":[data_encoding::BASE64.encode(pdf)]});
    let batch = json!({"frames":[{"kind":"preview","width":512,"height":512,"html":"","css":""}],"fonts":[]});
    let script = format!(
        r#"input=$(/bin/cat); {before}; case "$input" in *'"mode":"markup"'*) printf '%s' '{batch}';; *'"mode":"pdf"'*) printf '%s' '{pdf}';; *) printf '%s' '{reply}';; esac"#
    );
    pop_runner::Evaluator::process("/bin/sh".into(), vec!["-c".into(), script]).unwrap()
}
pub fn media_key(bytes: &[u8], extension: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("media/{}.{}", data_encoding::HEXLOWER.encode(&Sha256::digest(bytes)), extension)
}
/// Test inputs exercise the JSON admission boundary before typed core application.
pub trait ApplyJson {
    fn apply_json(&mut self, json: &str, origin: pop_core::Origin) -> Result<pop_core::Applied, pop_core::Error>;
}
impl ApplyJson for pop_core::Document {
    fn apply_json(&mut self, json: &str, origin: pop_core::Origin) -> Result<pop_core::Applied, pop_core::Error> {
        pop_core::Batch::decode(json).and_then(|batch| self.apply_batch(batch, origin))
    }
}
