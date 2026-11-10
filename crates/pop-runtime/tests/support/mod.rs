#![allow(dead_code, reason = "runtime suites share a small set of infrastructure fixture helpers")]
pub mod generate;
pub mod relay;
use serde_json::{Value, json};
/// Pin the prepared snapshot for this test process. Standalone tests use the last
/// resolved inventory; `verify` passes a freshly resolved, fixed snapshot explicitly.
pub fn compat_root() -> std::path::PathBuf {
    static ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let path = std::env::var_os("POP_COMPAT_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.pop/compat/current"));
        std::fs::canonicalize(path)
            .expect("No prepared compatibility corpus; run bun run compat:resolve before standalone tests")
    })
    .clone()
}
/// The writer-lock registry test runs use, so they never fill `~/.pop/live`.
pub fn registry_folder() -> std::path::PathBuf {
    std::env::temp_dir().join("pop-test-registry")
}
/// Points this process's registry at `registry_folder`; every test that takes a writer
/// lock calls it first.
pub fn isolate_registry() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| pop_runtime::registry::use_folder(&registry_folder()).unwrap());
}
/// This test binary running only its ignored test `name` (module-qualified: `capture::…`), as
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
            format: pop_runtime::PACKAGE_FORMAT,
            abi: pop_runtime::RUNTIME_ABI,
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
pub fn pack(stage: &std::path::Path, destination: &std::path::Path) -> Result<(), pop_runtime::store::Error> {
    pack_rendering(stage, destination, None).map(|_| ())
}
/// Packs, rendering the artwork the stage does not supply with `render`.
pub fn pack_rendering(
    stage: &std::path::Path,
    destination: &std::path::Path,
    render: Option<pop_runtime::file::RenderArtwork>,
) -> Result<pop_runtime::engine::PackedArtwork, pop_runtime::store::Error> {
    pop_runtime::file::pack(&std::fs::read_to_string(stage.join("input.json")).unwrap(), stage, destination, render)
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
pub fn renderer(before: &str, bytes: &[u8]) -> pop_runtime::owner::Evaluator {
    renderer_outputs(before, bytes, bytes)
}
pub fn renderer_outputs(before: &str, bytes: &[u8], pdf: &[u8]) -> pop_runtime::owner::Evaluator {
    let reply = json!({"ok": true, "images": [data_encoding::BASE64.encode(bytes)]});
    let pdf = json!({"ok":true,"images":[data_encoding::BASE64.encode(pdf)]});
    let batch = json!({"frames":[{"kind":"preview","width":512,"height":512,"html":"","css":""}],"fonts":[]});
    let script = format!(
        r#"input=$(/bin/cat); {before}; case "$input" in *'"mode":"markup"'*) printf '%s' '{batch}';; *'"mode":"pdf"'*) printf '%s' '{pdf}';; *) printf '%s' '{reply}';; esac"#
    );
    pop_runtime::owner::Evaluator::process("/bin/sh".into(), vec!["-c".into(), script]).unwrap()
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
/// Test inputs exercise the JSON admission boundary before typed core application.
pub fn media_key(bytes: &[u8], extension: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("media/{}.{}", data_encoding::HEXLOWER.encode(&Sha256::digest(bytes)), extension)
}
pub trait ApplyJson {
    fn apply_json(
        &mut self,
        json: &str,
        origin: pop_runtime::Origin,
    ) -> Result<pop_runtime::Applied, pop_runtime::Error>;
}
impl ApplyJson for pop_runtime::Document {
    fn apply_json(
        &mut self,
        json: &str,
        origin: pop_runtime::Origin,
    ) -> Result<pop_runtime::Applied, pop_runtime::Error> {
        pop_runtime::Batch::decode(json).and_then(|batch| self.apply_batch(batch, origin))
    }
}
