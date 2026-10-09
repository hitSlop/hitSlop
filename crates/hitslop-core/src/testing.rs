//! Fixtures for this crate's unit tests.
use std::path::{Path, PathBuf};

/// A document of `descriptor` with `initial` values in `dir`, packed and created the way
/// hosts make one. Writer locks go to a test registry, never `~/.hitslop/live`.
pub(crate) fn document(dir: &Path, descriptor: &str, initial: &str) -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| crate::registry::use_folder(&std::env::temp_dir().join("hitslop-test-registry")).unwrap());
    let stage = dir.join("stage");
    std::fs::create_dir_all(stage.join("assets")).unwrap();
    std::fs::write(stage.join("assets/ui.js"), "export default {}").unwrap();
    let input = serde_json::json!({
        "packageFormat": crate::PACKAGE_FORMAT,
        "runtimeABI": crate::RUNTIME_ABI,
        "declaration": {
            "metadata": {"author":{"name":"Fixture"},"slug":"fixture","title":"Fixture","description":"A test document.","categories":["utilities"]},
            "window": {"kind":"standard","width":320,"height":240},
            "document": serde_json::from_str::<serde_json::Value>(descriptor).unwrap(),
            "initial": serde_json::from_str::<serde_json::Value>(initial).unwrap(),
            "theme": [{"token":"accent","color":"#335577"}],
            "commands": [],
            "views": {"export": false, "icon": false}
        },
        "roles": {"ui": "ui.js"},
        "resources": [{"kind":"app","key":"ui.js","mediaType":"text/javascript","path":"assets/ui.js"}],
        "artwork": {}
    });
    let template = dir.join("template.slop");
    crate::file::pack(&input.to_string(), &stage, &template).unwrap();
    let document = dir.join("document.slop");
    crate::file::create_document(&template, &document).unwrap();
    document
}
