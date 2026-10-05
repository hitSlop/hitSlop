//! Documents a released engine wrote, opened by this core. Every corpus entry (`tests/compat`)
//! keeps its release's npm CLI, which carries the file engine that release shipped. That
//! engine creates documents from the entry's templates and applies generated batches
//! through its own request protocol; this core must then read exactly what the released
//! engine reads, and edit, save, trim and reopen the document. Beside the fixed documents
//! `compat.rs` replays, this covers generated edits by every released writer.
//! Failure: a saved shape some release writes that this build reads differently or refuses.
//! Oracle: the released engine's own reading of its own document.
//! Runs where the shipped engine runs (darwin-arm64); `HITSLOP_COMPAT_SEEDS` and
//! `HITSLOP_COMPAT_STEPS` size it.
use hitslop_core::store::{Mode, Store};
use hitslop_core::Origin;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
mod support;
use support::generate::intent;
use support::next;

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/compat")
}
fn sorted(path: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(path).map(|dir| dir.map(|e| e.unwrap().path()).collect()).unwrap_or_default();
    found.sort();
    found
}
/// The file engine the entry's CLI tarball ships for this platform, unpacked into `into`.
fn released_engine(entry: &Path, into: &Path) -> PathBuf {
    let tarball = sorted(&entry.join("cli"))
        .into_iter()
        .find(|path| path.file_name().unwrap().to_string_lossy().starts_with("hitslop-cli-"))
        .unwrap_or_else(|| panic!("{}: no released CLI", entry.display()));
    let member = "package/engine/darwin-arm64/slop-engine";
    let status = Command::new("/usr/bin/tar").arg("-xzf").arg(&tarball).arg("-C").arg(into).arg(member).status().unwrap();
    assert!(status.success(), "{}: the released CLI ships no darwin-arm64 engine", entry.display());
    into.join(member)
}
/// `engine` run with `args`: its standard output, which must report success.
fn run(engine: &Path, args: &[&str]) -> String {
    let output = Command::new(engine).args(args).env("HITSLOP_TEST_REGISTRY", support::registry_folder()).output().unwrap();
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}
/// One request in the released engine's protocol, and its reply.
fn request(engine: &Path, body: Value) -> Value {
    let mut child = Command::new(engine)
        .arg("request")
        .env("HITSLOP_TEST_REGISTRY", support::registry_folder())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(body.to_string().as_bytes()).unwrap();
    serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap()
}
fn released_value(engine: &Path, document: &Path) -> Value {
    let reply = request(engine, json!({"method":"get","documentPath":document}));
    assert_eq!(reply["ok"], true, "the released engine cannot read its own document: {reply}");
    reply["state"]["state"]["value"].clone()
}
/// The saved value as this core reads it.
fn value_here(document: &Path) -> Value {
    let store = Store::open(document, Mode::Snapshot).unwrap_or_else(|e| panic!("this core refuses the document: {e}"));
    let state: Value = serde_json::from_str(&store.document().unwrap().snapshot().unwrap()).unwrap();
    state["value"].clone()
}

#[test]
fn documents_each_release_writes_read_the_same_here_and_take_edits() {
    if !(cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")) {
        return;
    }
    support::isolate_registry();
    let (seeds, steps) = (support::workload("HITSLOP_COMPAT_SEEDS", 2), support::workload("HITSLOP_COMPAT_STEPS", 8));
    let mut cases = 0;
    for entry in sorted(&corpus()).into_iter().filter(|path| path.join("release.json").exists()) {
        let dir = tempfile::tempdir().unwrap();
        let engine = released_engine(&entry, dir.path());
        for template in sorted(&entry.join("templates")) {
            let slug = template.file_stem().unwrap().to_string_lossy().into_owned();
            for seed in 1..=seeds {
                let label = format!("{}/{slug}, seed {seed}", entry.file_name().unwrap().to_string_lossy());
                let document = dir.path().join(format!("{slug}-{seed}.slop"));
                run(&engine, &["create", "--from", template.to_str().unwrap(), "--output", document.to_str().unwrap()]);
                let descriptor: Value = serde_json::from_str(&run(&engine, &["schema", document.to_str().unwrap()])).unwrap();
                let (mut rng, mut serial) = (seed as u64 * 0x9e3779b1 + 11, 0);
                for _ in 0..steps {
                    let current = released_value(&engine, &document);
                    let ops: Vec<Value> = (0..1 + next(&mut rng) % 3).map(|_| intent(&mut rng, &mut serial, &descriptor, &current)).collect();
                    // A deliberately invalid value is refused, changing nothing.
                    request(&engine, json!({"method":"batch","documentPath":document,"ops":Value::Array(ops).to_string()}));
                }
                let released = released_value(&engine, &document);
                assert_eq!(value_here(&document), released, "{label}: this core reads the released engine's document differently");

                // And it edits it: a generated batch, saved, trimmed on close, reopened.
                let store = Store::open(&document, Mode::Document).unwrap();
                let mut doc = store.document().unwrap();
                let mut edited = None;
                for _ in 0..16 {
                    let op = intent(&mut rng, &mut serial, &descriptor, &released);
                    if doc.apply_batch(&json!({"intents":[op]}).to_string(), Origin::Agent).is_ok() {
                        let value = serde_json::from_str::<Value>(&doc.value().unwrap()).unwrap();
                        // An accepted edit can leave the value as it was (clearing an absent key).
                        if value != released {
                            edited = Some(value);
                            break;
                        }
                    }
                }
                let edited = edited.unwrap_or_else(|| panic!("{label}: no generated edit changed the document"));
                let job = store.job(&mut doc, false).unwrap().expect("an edit to save");
                store.write(&job).unwrap();
                if let Some(job) = store.close_job(&mut doc).unwrap() {
                    store.write(&job).unwrap();
                }
                store.close().unwrap();
                assert_eq!(value_here(&document), edited, "{label}: the edit did not reopen");
                cases += 1;
            }
        }
    }
    assert!(cases > 0, "tests/compat has no released engines to replay");
}
