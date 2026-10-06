//! The compatibility corpus (`tests/compat`): every document file a release saved opens in
//! this core and reads as that release recorded, takes the release's recorded edit, saves,
//! trims its history on close and reopens to the recorded result. See docs/testing.md.
use hitslop_core::Origin;
use hitslop_core::store::{Mode, Store};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
mod support;

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/compat")
}
fn entries(path: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> =
        std::fs::read_dir(path).map(|dir| dir.map(|e| e.unwrap().path()).collect()).unwrap_or_default();
    found.sort();
    found
}
fn json_file(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path).ok().map(|text| serde_json::from_str(&text).unwrap())
}
/// The saved document as a reader that owns nothing sees it: value and theme.
fn read(root: &Path) -> (Value, Value) {
    let store = Store::open(root, Mode::Snapshot).unwrap();
    let doc = store.document().unwrap();
    let state: Value = serde_json::from_str(&doc.state().unwrap()).unwrap();
    let theme = doc.theme_state().unwrap();
    let theme = json!({
        "overrides": serde_json::from_str::<Value>(&theme.overrides).unwrap(),
        "effective": serde_json::from_str::<Value>(&theme.effective).unwrap(),
    });
    (state["value"].clone(), theme)
}

#[test]
fn every_saved_document_reads_edits_and_reopens_as_its_release_recorded() {
    support::isolate_registry();
    let mut cases = 0;
    for entry in entries(&corpus()).into_iter().filter(|path| path.join("release.json").exists()) {
        for saved in
            entries(&entry.join("documents")).into_iter().filter(|p| p.extension().is_some_and(|e| e == "slop"))
        {
            let name = saved.file_stem().unwrap().to_string_lossy().into_owned();
            let label = format!("{}/{name}", entry.file_name().unwrap().to_string_lossy());
            let expected = json_file(&entry.join(format!("expected/{name}.json")))
                .unwrap_or_else(|| panic!("{label}: no expected state"));
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("Document.slop");
            std::fs::copy(&saved, &root).unwrap();
            hitslop_core::file::open(&root, true).unwrap_or_else(|e| panic!("{label}: the document is refused: {e}"));
            let (state, theme) = read(&root);
            assert_eq!(state, expected["value"], "{label}: the saved document reads differently");
            assert_eq!(theme["overrides"], expected["theme"]["overrides"], "{label}: theme overrides");
            assert_eq!(theme["effective"], expected["theme"]["effective"], "{label}: effective theme");

            if let Some(scenario) = json_file(&entry.join(format!("scenarios/{name}.json"))) {
                let store = Store::open(&root, Mode::Document).unwrap();
                let mut doc = store.document().unwrap();
                doc.apply_batch(&json!({"intents": scenario["ops"]}).to_string(), Origin::Agent)
                    .unwrap_or_else(|e| panic!("{label}: the recorded edit was refused: {e}"));
                assert_eq!(
                    serde_json::from_str::<Value>(&doc.value()).unwrap(),
                    scenario["value"],
                    "{label}: the edit differs"
                );
                let job = store.job(&mut doc, false).unwrap().expect("an edit to save");
                store.write(&job).unwrap();
                if let Some(job) = store.close_job(&mut doc).unwrap() {
                    store.write(&job).unwrap();
                }
                store.close().unwrap();
                let (state, _) = read(&root);
                assert_eq!(state, scenario["value"], "{label}: the edit did not reopen");
            }
            cases += 1;
        }
    }
    assert!(cases > 0, "tests/compat has no documents to replay");
}
