use serde_json::{Value, json};
pub fn isolate_registry() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        pop_storage::registry::use_folder(&std::env::temp_dir().join("pop-library-test-registry")).unwrap()
    });
}
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
) -> Result<pop_runtime::engine::PackedArtwork, pop_storage::store::Error> {
    pop_storage::file::pack(&std::fs::read_to_string(stage.join("input.json")).unwrap(), stage, destination, render)
}
