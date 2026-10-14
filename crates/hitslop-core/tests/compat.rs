//! The compatibility corpus (`tests/compat`): every document file a release saved opens in
//! this core and reads as that release recorded, takes the release's recorded edit, saves,
//! trims its history on close and reopens to the recorded result. See docs/testing.md.
use crate::support;
use hitslop_core::store::{Mode, Store};
use hitslop_core::{
    Origin,
    file::{ResourceRoute, resource_response},
    owner::{Failure, Owner, Reply, Request},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Duration;
use support::ApplyJson;

fn corpus() -> PathBuf {
    std::env::var_os("HITSLOP_COMPAT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/compat"))
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
        "overrides": theme.overrides,
        "effective": theme.effective,
    });
    (state["value"].clone(), theme)
}

fn attachments(store: &Store) -> Value {
    json!(
        store
            .attachments()
            .unwrap()
            .iter()
            .map(|a| json!({
                "id": a.id, "byteLength": a.bytes, "mimeType": a.media_type,
            }))
            .collect::<Vec<_>>()
    )
}

/// Read every public resource through the same range reader the page uses. Private
/// capture/command programs must stay inaccessible in the app URL namespace.
fn resources(root: &Path, expected: &Value) {
    let store = Store::open(root, Mode::Snapshot).unwrap();
    assert_eq!(attachments(&store), expected["attachments"]);
    let reader = store.resource_reader().unwrap();
    let conn = rusqlite::Connection::open(root).unwrap();
    let mut assets = conn.prepare("SELECT key, media_type, encoding, bytes FROM assets ORDER BY key").unwrap();
    let rows = assets
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Vec<u8>>(3)?))
        })
        .unwrap();
    let check = |route, key: &str, media: &str, bytes: Vec<u8>| {
        let info = reader.info(route, key).unwrap().unwrap_or_else(|| panic!("missing resource {key}"));
        assert_eq!(info.media_type, media, "{key}: type");
        assert_eq!(info.size, bytes.len() as u64, "{key}: size");
        assert_eq!(reader.read_range(route, key, 0, info.size).unwrap().unwrap(), bytes, "{key}: bytes");
        for range in [None, Some("bytes=0-0"), Some("bytes=-1"), Some("bytes=999999999999-")] {
            let plan = resource_response(&info, route, range);
            assert_eq!(plan.headers["Content-Type"], media);
            assert_eq!(plan.headers["X-Content-Type-Options"], "nosniff");
            if route == ResourceRoute::Attachment {
                assert_eq!(plan.headers["Content-Security-Policy"], "sandbox");
            }
            let status = match range {
                None => 200,
                Some("bytes=0-0" | "bytes=-1") if info.size > 0 => 206,
                _ => 416,
            };
            assert_eq!(plan.status, status, "{key}: range response");
            if let Some(span) = plan.span {
                let part = reader.read_range(route, key, span.offset, span.length).unwrap().unwrap();
                assert_eq!(part, bytes[span.offset as usize..(span.offset + span.length) as usize]);
                assert_eq!(plan.headers["Content-Length"], part.len().to_string());
            } else {
                assert_eq!(plan.headers["Content-Range"], format!("bytes */{}", info.size));
            }
        }
        assert!(reader.read_range(route, key, info.size, 1).unwrap().unwrap().is_empty());
    };
    for row in rows {
        let (key, media, encoding, stored) = row.unwrap();
        if matches!(key.as_str(), "commands.js" | "widget.js" | "icon.js" | "export.js" | "preview.js") {
            assert!(reader.info(ResourceRoute::App, &key).unwrap().is_none());
            assert!(reader.read_range(ResourceRoute::App, &key, 0, 1).unwrap().is_none());
            continue;
        }
        let bytes = if encoding == "br" {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut brotli::Decompressor::new(stored.as_slice(), 4096), &mut bytes).unwrap();
            bytes
        } else {
            assert_eq!(encoding, "identity");
            stored
        };
        if let Some(name) = key.strip_prefix("media/") {
            assert_eq!(name.split('.').next().unwrap(), data_encoding::HEXLOWER.encode(&Sha256::digest(&bytes)));
        }
        check(ResourceRoute::App, &key, &media, bytes);
    }
    for a in expected["attachments"].as_array().unwrap() {
        let id = a["id"].as_str().unwrap();
        let bytes = store.attachment(id).unwrap();
        assert_eq!(data_encoding::HEXLOWER.encode(&Sha256::digest(&bytes)), id);
        check(ResourceRoute::Attachment, id, a["mimeType"].as_str().unwrap(), bytes);
    }
}
fn owner_request(owner: &Owner, request: Request) -> Result<Reply, Failure> {
    let (tx, rx) = mpsc::channel();
    owner.submit(
        request,
        None,
        Box::new(move |reply| {
            tx.send(reply).unwrap();
        }),
    );
    rx.recv_timeout(Duration::from_secs(10)).expect("owner did not reply")
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

            resources(&root, &expected);
            let scenario = json_file(&entry.join(format!("scenarios/{name}.json")))
                .unwrap_or_else(|| panic!("{label}: missing CLI scenario"));
            {
                let store = Store::open(&root, Mode::Document).unwrap();
                let mut doc = store.document().unwrap();
                doc.apply_json(&json!({"intents": scenario["ops"]}).to_string(), Origin::Agent)
                    .unwrap_or_else(|e| panic!("{label}: the recorded edit was refused: {e}"));
                assert_eq!(
                    serde_json::from_str::<Value>(&doc.value()).unwrap(),
                    scenario["value"],
                    "{label}: the edit differs"
                );
                let job = store.job(&mut doc).unwrap().expect("an edit to save");
                store.write(&job).unwrap();
                // Attachment references are strings, including strings in nested values.
                fn references(value: &Value, id: &str) -> bool {
                    match value {
                        Value::String(s) => s == id,
                        Value::Array(a) => a.iter().any(|v| references(v, id)),
                        Value::Object(o) => o.values().any(|v| references(v, id)),
                        _ => false,
                    }
                }
                let edited: Value = serde_json::from_str(&doc.value()).unwrap();
                let before = store.attachments().unwrap();
                if let Some(job) = store.close_job(&mut doc).unwrap() {
                    store.write(&job).unwrap();
                }
                store.reclaim_attachments().unwrap();
                store.close().unwrap();
                let (state, _) = read(&root);
                assert_eq!(state, scenario["value"], "{label}: the edit did not reopen");
                let reopened = Store::open(&root, Mode::Snapshot).unwrap();
                assert_eq!(
                    reopened.attachments().unwrap(),
                    before.into_iter().filter(|a| references(&edited, &a.id)).collect::<Vec<_>>(),
                    "{label}: attachments after close"
                );
            }
            cases += 1;
        }
    }
    assert!(cases > 0, "tests/compat has no documents to replay");
}

#[test]
fn no_recorded_open_limit_becomes_stricter_or_disappears() {
    let mut cases = 0;
    for entry in entries(&corpus()).into_iter().filter(|path| path.join("release.json").exists()) {
        let release = json_file(&entry.join("release.json")).unwrap();
        hitslop_core::app::AppDefinition::check_recorded_limits(&release["limits"].to_string()).unwrap_or_else(
            |error| {
                panic!("{}: {error}; recapture the pre-launch dev entry if its records are missing", entry.display())
            },
        );
        cases += 1;
    }
    assert!(cases > 0, "tests/compat has no recorded limits");
}

#[test]
fn every_stored_acceptance_vector_still_decodes_and_reencodes_equally() {
    check_stored_acceptance_vectors(false);
}

#[test]
fn large_acceptance_vectors_still_decode_and_reencode_equally() {
    check_stored_acceptance_vectors(true);
}

fn check_stored_acceptance_vectors(large: bool) {
    let mut cases = 0;
    for entry in entries(&corpus()).into_iter().filter(|path| path.join("release.json").exists()) {
        let vectors: Vec<_> = entries(&entry.join("acceptance"))
            .into_iter()
            .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
            .collect();
        assert!(
            !vectors.is_empty(),
            "{}: missing acceptance vectors; recapture the pre-launch dev entry",
            entry.display()
        );
        for path in vectors {
            let stored = std::fs::read_to_string(&path).unwrap();
            if hitslop_core::app::AppDefinition::acceptance_vector_is_large(&stored)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                != large
            {
                continue;
            }
            hitslop_core::app::AppDefinition::check_acceptance_vector(&stored)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            cases += 1;
        }
    }
    assert!(cases > 0, "tests/compat has no acceptance vectors");
}

fn evaluate(input: &Value) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let engine = std::env::var_os("HITSLOP_ENGINE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/slop-engine"));
    let mut child = Command::new(&engine)
        .arg("--evaluate-command")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("build slop-engine or set HITSLOP_ENGINE ({}): {e}", engine.display()));
    child.stdin.take().unwrap().write_all(input.to_string().as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "evaluator failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

/// Evaluate frozen command bytes in the real realm, then pass that exact reply through
/// the owner admission/save path. Child process fencing is covered by owner_commands.
#[test]
fn every_stored_command_probe_evaluates_and_passes_owner_admission() {
    support::isolate_registry();
    let mut cases = 0;
    for entry in entries(&corpus()).into_iter().filter(|p| p.join("release.json").exists()) {
        for saved in
            entries(&entry.join("documents")).into_iter().filter(|p| p.extension().is_some_and(|e| e == "slop"))
        {
            let name = saved.file_stem().unwrap().to_string_lossy();
            let probes = json_file(&entry.join(format!("commands/{name}.json")));
            if name.ends_with(".page") && probes.is_none() {
                eprintln!("SKIP {name}: commands are recorded for CLI-saved documents; page edits replay separately");
                continue;
            }
            let probes = probes.unwrap_or_else(|| panic!("{name}: missing command probes"));
            let probes = probes.as_array().expect("command records must be probe arrays; recapture dev");
            assert!(!probes.is_empty(), "{name}: no command probes");
            for probe in probes {
                let dir = tempfile::tempdir().unwrap();
                let root = dir.path().join("Document.slop");
                std::fs::copy(&saved, &root).unwrap();
                let store = Store::open(&root, Mode::Snapshot).unwrap();
                let before = read(&root).0;
                let app = store.app();
                let mut args = probe["args"].clone();
                let channel = probe["channel"].as_str().expect("probe channel");
                assert!(matches!(channel, "call" | "evaluate"));
                let prepared = app
                    .app
                    .commands()
                    .iter()
                    .find(|c| c.name == probe["name"].as_str().unwrap())
                    .unwrap()
                    .args
                    .prepare(&mut args);
                let output = if channel == "call" && prepared.is_err() {
                    // The owner must reject these arguments before evaluating anything.
                    None
                } else {
                    let input = json!({"runtimeABI": app.runtime_abi, "bundle": app.commands.as_deref().expect("stored commands"), "request": json!({
                        "name":probe["name"], "args":if channel == "call" { &args } else { &probe["args"] },
                        "now":probe["now"], "seed":probe["seed"], "descriptor":app.app.document_raw(), "value":before
                    }).to_string()});
                    let output: Value = serde_json::from_str(&evaluate(&input)).unwrap();
                    if channel == "evaluate" {
                        assert_eq!(output, probe["evaluated"], "{name}: {} evaluator reply", probe["name"]);
                    } else {
                        assert_eq!(output["result"], probe["evaluated"]["result"]);
                    }
                    Some(output)
                };
                drop(store);
                let reply_path = dir.path().join("reply.json");
                let input_path = dir.path().join("owner-input.json");
                std::fs::write(
                    &reply_path,
                    output.as_ref().unwrap_or(&json!({"ok":false,"error":"bad args reached evaluator"})).to_string(),
                )
                .unwrap();
                let evaluator = hitslop_runner::Evaluator::new(
                    "/bin/sh".into(),
                    vec![
                        "-c".into(),
                        "/bin/cat > \"$2\"; /bin/cat \"$1\"".into(),
                        "compat".into(),
                        reply_path.to_string_lossy().into_owned(),
                        input_path.to_string_lossy().into_owned(),
                    ],
                )
                .unwrap();
                let owner =
                    Owner::open_with_evaluator(&root, Mode::Document, Arc::new(|_| {}), Some(evaluator)).unwrap();
                let reply = owner_request(
                    &owner,
                    Request::Command {
                        name: probe["name"].as_str().unwrap().into(),
                        args_json: probe["args"].to_string(),
                        origin: Origin::Agent,
                    },
                );
                assert_eq!(reply.is_ok(), probe["expected"]["ok"].as_bool().unwrap(), "{name}: {reply:?}");
                if output.is_some() {
                    let input = json_file(&input_path).expect("owner did not invoke evaluator");
                    assert_eq!(input["runtimeABI"], owner.app().runtime_abi);
                    assert_eq!(input["bundle"], owner.app().commands.as_deref().unwrap().as_str());
                    let request: Value = serde_json::from_str(input["request"].as_str().unwrap()).unwrap();
                    assert_eq!(request["name"], probe["name"]);
                    assert_eq!(request["args"], args, "owner did not prepare the recorded arguments");
                    assert_eq!(request["value"], before);
                } else {
                    assert!(!input_path.exists(), "invalid arguments reached the evaluator");
                }
                match reply {
                    Ok(Reply::Command { result_json, .. }) => {
                        assert_eq!(serde_json::from_str::<Value>(&result_json).unwrap(), probe["evaluated"]["result"])
                    }
                    Ok(other) => panic!("unexpected command reply: {other:?}"),
                    Err(error) => {
                        if probe["expected"]["code"] == "rejected" {
                            assert_eq!(error.kind, hitslop_core::owner::FailureKind::Rejected);
                        }
                        if probe["expected"]["refused"] == true {
                            assert_eq!(error.reason, Some(hitslop_core::Code::Refused));
                        }
                        if let Some(text) = probe["expected"]["errorIncludes"].as_str() {
                            assert!(error.message.contains(text), "{error:?}");
                        }
                    }
                }
                owner_request(&owner, Request::Close { preview: None, icon: None }).unwrap();
                assert_eq!(read(&root).0, probe["value"], "{name}: command saved value");
                if probe["expected"]["ok"] == false {
                    assert_eq!(read(&root).0, before, "refusal changed state");
                }
                cases += 1;
            }
        }
    }
    assert!(cases > 0, "tests/compat has no stored command probes");
}
