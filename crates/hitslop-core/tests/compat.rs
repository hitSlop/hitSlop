//! The compatibility corpus (`tests/compat`): every document a release saved opens in this
//! core and reads as that release recorded, takes the release's recorded edit, saves, trims
//! its history on close and reopens to the recorded result. See docs/testing.md.
use hitslop_core::store::{Mode, Store};
use hitslop_core::theme::Change;
use hitslop_core::Origin;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/compat")
}
fn entries(path: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(path).map(|dir| dir.map(|e| e.unwrap().path()).collect()).unwrap_or_default();
    found.sort();
    found
}
fn json_file(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path).ok().map(|text| serde_json::from_str(&text).unwrap())
}
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in entries(from) {
        let target = to.join(entry.file_name().unwrap());
        if entry.is_dir() { copy(&entry, &target) } else { std::fs::copy(&entry, &target).unwrap(); }
    }
}
struct Package { root: PathBuf, key: String, initial: String, theme: String }
impl Package {
    fn new(root: &Path) -> Self {
        let read = |file: &str| std::fs::read_to_string(root.join(file)).unwrap();
        let initial = read("initial.json");
        Self { root: root.into(), key: hitslop_core::validate(&read("state.schema.json"), &initial).unwrap(), initial, theme: read("assets/theme.json") }
    }
    /// The saved document as a reader that owns nothing sees it: value, issues and theme.
    fn read(&self) -> (Value, Value) {
        let store = Store::open(&self.root, Mode::Snapshot).unwrap();
        let doc = store.document(&self.key, &self.initial, &self.theme).unwrap();
        let state: Value = serde_json::from_str(&doc.snapshot().unwrap()).unwrap();
        let (theme, _) = store.theme(Change::Get).unwrap();
        let theme = json!({
            "overrides": serde_json::from_str::<Value>(&theme.overrides).unwrap(),
            "effective": serde_json::from_str::<Value>(&theme.effective).unwrap(),
        });
        (json!({"value": state["value"], "issues": state["issues"]}), theme)
    }
}

#[test]
fn every_saved_document_reads_edits_and_reopens_as_its_release_recorded() {
    let mut cases = 0;
    for entry in entries(&corpus()).into_iter().filter(|path| path.join("release.json").exists()) {
        for saved in entries(&entry.join("documents")) {
            let name = saved.file_stem().unwrap().to_string_lossy().into_owned();
            let label = format!("{}/{name}", entry.file_name().unwrap().to_string_lossy());
            let expected = json_file(&entry.join(format!("expected/{name}.json"))).unwrap_or_else(|| panic!("{label}: no expected state"));
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("Document.slop");
            copy(&saved, &root);
            #[cfg(feature = "schema-validation")]
            hitslop_core::manifest::validate(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
                .unwrap_or_else(|e| panic!("{label}: the package's manifest is refused: {e}"));
            let package = Package::new(&root);
            let (state, theme) = package.read();
            assert_eq!(state, json!({"value": expected["value"], "issues": expected["issues"]}), "{label}: the saved document reads differently");
            assert_eq!(theme["overrides"], expected["theme"]["overrides"], "{label}: theme overrides");
            assert_eq!(theme["effective"], expected["theme"]["effective"], "{label}: effective theme");

            if let Some(scenario) = json_file(&entry.join(format!("scenarios/{name}.json"))) {
                let store = Store::open(&root, Mode::Document).unwrap();
                let mut doc = store.document(&package.key, &package.initial, &package.theme).unwrap();
                doc.apply_batch(&json!({"intents": scenario["ops"]}).to_string(), Origin::Agent)
                    .unwrap_or_else(|e| panic!("{label}: the recorded edit was refused: {e}"));
                assert_eq!(serde_json::from_str::<Value>(&doc.value().unwrap()).unwrap(), scenario["value"], "{label}: the edit differs");
                let job = store.job(&mut doc, false).unwrap().expect("an edit to save");
                store.write(&job).unwrap();
                if let Some(job) = store.close_job(&mut doc).unwrap() {
                    store.write(&job).unwrap();
                }
                store.close().unwrap();
                let (state, _) = package.read();
                assert_eq!(state, json!({"value": scenario["value"], "issues": scenario["issues"]}), "{label}: the edit did not reopen");
            }
            cases += 1;
        }
    }
    assert!(cases > 0, "tests/compat has no documents to replay");
}
