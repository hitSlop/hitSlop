//! Documents a released engine wrote, opened by this core. Every corpus entry (resolved release assets)
//! resolves its candidate writer by hash: the darwin-arm64 engine its release built and wrote its
//! documents with. That engine creates documents from the entry's templates and applies
//! generated batches through its own request protocol (these requests are the only adapter
//! for older protocols, and they live here, in the test); this core must then read exactly
//! what the released engine reads, and edit, save, trim and reopen the document. Beside the fixed documents
//! `compat.rs` replays, this covers generated edits by every released writer.
//! Failure: a saved shape some release writes that this build reads differently or refuses.
//! Oracle: the released engine's own reading of its own document.
//! Runs where the shipped engine runs (darwin-arm64); `TINYS_COMPAT_SEEDS` and
//! `TINYS_COMPAT_STEPS` size it.
use crate::support;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use support::ApplyJson;
use support::generate::intent;
use support::next;
use tinys_core::Origin;
use tinys_core::store::{Mode, Store};

fn sorted(path: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> =
        std::fs::read_dir(path).map(|dir| dir.map(|e| e.unwrap().path()).collect()).unwrap_or_default();
    found.sort();
    found
}
/// The hash-resolved writer, copied into `into` and made executable. Every published
/// release must resolve (run `bun scripts/compat/writers.ts`).
fn released_engine(entry: &Path, into: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let release: Value = serde_json::from_slice(&std::fs::read(entry.join("release.json")).unwrap()).unwrap();
    let hash = release["writer"]["sha256"].as_str().expect("missing writer SHA-256");
    assert!(hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "invalid writer SHA-256");
    let cache = std::env::var_os("TINYS_COMPAT_WRITERS").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").expect("HOME is required for the writer cache"))
            .join(".cache/tinys/compat-writers")
    });
    let writer = cache.join(hash);
    let bytes = match std::fs::read(&writer) {
        Ok(bytes) => bytes,
        Err(error) => panic!(
            "{}: missing resolved writer {}: {error}; run bun scripts/compat/writers.ts before Rust replay",
            entry.display(),
            writer.display()
        ),
    };
    use sha2::{Digest, Sha256};
    assert_eq!(
        Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        hash,
        "cached writer hash differs from release.json"
    );
    let copy = into.join("tiny-engine");
    std::fs::copy(&writer, &copy).unwrap_or_else(|e| panic!("{}: no candidate writer: {e}", entry.display()));
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o755)).unwrap();
    copy
}
/// The command protocol the entry's release speaks, as it recorded it.
fn released_protocol(entry: &Path) -> String {
    let release: Value = serde_json::from_slice(&std::fs::read(entry.join("release.json")).unwrap()).unwrap();
    release["markers"]["protocol"]
        .as_u64()
        .unwrap_or_else(|| panic!("{}: no protocol marker", entry.display()))
        .to_string()
}
/// One request in the released engine's protocol, and its reply.
fn request(engine: &Path, protocol: &str, body: Value) -> Value {
    let mut child = Command::new(engine)
        .args(["--client-protocol", protocol])
        .env("TINYS_TEST_REGISTRY", support::registry_folder())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(body.to_string().as_bytes()).unwrap();
    serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap()
}
fn released_value(engine: &Path, protocol: &str, document: &Path) -> Value {
    let reply = request(engine, protocol, json!({"method":"get","documentPath":document}));
    assert_eq!(reply["ok"], true, "the released engine cannot read its own document: {reply}");
    reply["state"]["value"].clone()
}
/// The saved value as this core reads it.
fn value_here(document: &Path) -> Value {
    let store = Store::open(document, Mode::Snapshot).unwrap_or_else(|e| panic!("this core refuses the document: {e}"));
    let state: Value = serde_json::from_str(&store.document().unwrap().state().unwrap()).unwrap();
    state["value"].clone()
}

#[test]
fn documents_each_release_writes_read_the_same_here_and_take_edits() {
    if !(cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")) {
        eprintln!(
            "SKIP: compatibility writer replay requires darwin-arm64; shipped Mac writers cannot run on this platform"
        );
        return;
    }
    support::isolate_registry();
    let (seeds, steps) = (support::workload("TINYS_COMPAT_SEEDS", 2), support::workload("TINYS_COMPAT_STEPS", 8));
    let mut cases = 0;
    for entry in sorted(&support::compat_root()).into_iter().filter(|path| path.join("release.json").exists()) {
        let dir = tempfile::tempdir().unwrap();
        let engine = released_engine(&entry, dir.path());
        let protocol = released_protocol(&entry);
        for template in sorted(&entry.join("templates")) {
            let slug = template.file_stem().unwrap().to_string_lossy().into_owned();
            for seed in 1..=seeds {
                let label = format!("{}/{slug}, seed {seed}", entry.file_name().unwrap().to_string_lossy());
                let document = dir.path().join(format!("{slug}-{seed}.tiny"));
                let (template, output) = (template.to_str().unwrap(), document.to_str().unwrap());
                assert_eq!(
                    request(&engine, &protocol, json!({"method":"create","from":template,"output":output}))["ok"],
                    true
                );
                let descriptor =
                    request(&engine, &protocol, json!({"method":"schema","file":document}))["schema"].clone();
                let (mut rng, mut serial) = (seed as u64 * 0x9e3779b1 + 11, 0);
                for _ in 0..steps {
                    let current = released_value(&engine, &protocol, &document);
                    let ops: Vec<Value> = (0..1 + next(&mut rng) % 3)
                        .map(|_| intent(&mut rng, &mut serial, &descriptor, &current))
                        .collect();
                    // A deliberately invalid value is refused, changing nothing.
                    request(
                        &engine,
                        &protocol,
                        json!({"method":"batch","documentPath":document,"batch":{"intents":ops}}),
                    );
                }
                let released = released_value(&engine, &protocol, &document);
                assert_eq!(
                    value_here(&document),
                    released,
                    "{label}: this core reads the released engine's document differently"
                );

                // And it edits it: a generated batch, saved, trimmed on close, reopened.
                let store = Store::open(&document, Mode::Document).unwrap();
                let mut doc = store.document().unwrap();
                let mut edited = None;
                for _ in 0..16 {
                    let op = intent(&mut rng, &mut serial, &descriptor, &released);
                    if doc.apply_json(&json!({"intents":[op]}).to_string(), Origin::Agent).is_ok() {
                        let value = serde_json::from_str::<Value>(&doc.value()).unwrap();
                        // An accepted edit can leave the value as it was (clearing an absent key).
                        if value != released {
                            edited = Some(value);
                            break;
                        }
                    }
                }
                let edited = edited.unwrap_or_else(|| panic!("{label}: no generated edit changed the document"));
                let job = store.job(&mut doc).unwrap().expect("an edit to save");
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
    assert!(cases > 0, "Prepared compatibility corpus has no released engines to replay");
}
