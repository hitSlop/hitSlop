#![cfg(feature = "storage")]
use hitslop_core::{Code, Document, file::build};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn input(root: &Path) -> Value {
    fs::create_dir_all(root.join("resources")).unwrap();
    fs::write(root.join("resources/ui.js"), b"export default {mount(){}};").unwrap();
    json!({
        "packageFormat":1,"runtimeABI":1,
        "declaration": {
            "metadata":{"slug":"fixture","title":"Fixture","description":"Build input","author":{"name":"Test"},"categories":["utilities"]},
            "window":{"kind":"standard","width":320,"height":240},
            "theme":[],"commands":[],"views":{"export":false,"icon":false},
            "document":{"kind":"object","properties":{"title":{"kind":"string"}}},
            "initial":{"title":"Original"}
        },
        "roles":{"ui":"ui.js"},
        "resources":[{"kind":"app","key":"ui.js","mediaType":"text/javascript","path":"resources/ui.js"}],
        "artwork":{}
    })
}
fn asset(root: &Path, input: &mut Value, bytes: &[u8], extension: &str, media: &str) -> String {
    let key = format!("media/{}.{extension}", data_encoding::HEXLOWER.encode(&Sha256::digest(bytes)));
    let path = format!("resources/{key}");
    fs::create_dir_all(root.join("resources/media")).unwrap();
    fs::write(root.join(&path), bytes).unwrap();
    input["resources"].as_array_mut().unwrap().push(json!({"kind":"app","key":key,"mediaType":media,"path":path}));
    key
}
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&vec![255; width as usize * height as usize * 4]).unwrap();
    bytes
}
fn refused(root: &Path, value: &Value) {
    if build::accept(&value.to_string(), root).is_ok() {
        panic!("accepted {value}");
    }
}

#[test]
fn acceptance_owns_the_explicit_inventory_and_initial_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let mut value = input(root.path());
    let key = asset(root.path(), &mut value, b"hello", "txt", "text/plain");
    fs::write(root.path().join("unlisted.txt"), "not packaged").unwrap();
    let accepted = build::accept(&value.to_string(), root.path()).unwrap();
    assert_eq!(accepted.requirements(), (1, 1));
    assert_eq!(accepted.assets().len(), 2);
    let document = Document::open(accepted.app().spec(), accepted.checkpoint(), &[]).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&document.value()).unwrap(), value["declaration"]["initial"]);
    fs::write(root.path().join(format!("resources/{key}")), "changed after acceptance").unwrap();
    assert_eq!(accepted.assets().iter().find(|a| a.key == key).unwrap().bytes, b"hello");
    assert_eq!(accepted.assets().iter().find(|a| a.key == key).unwrap().media_type, "text/plain");
    assert!(accepted.artwork().is_empty());
}

#[test]
fn a_seed_that_loro_would_round_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let mut value = input(root.path());
    value["declaration"]["document"] = json!({"kind":"object","properties":{"n":{"kind":"number"}}});
    value["declaration"]["initial"] = json!({"n":9_007_199_254_740_993u64});
    assert!(build::accept(&value.to_string(), root.path()).is_err(), "packing must not silently round initial values");
}

#[test]
fn initial_round_trip_keeps_number_meaning_and_names_nested_rows() {
    let root = tempfile::tempdir().unwrap();
    let mut value = input(root.path());
    value["declaration"]["document"] = json!({"kind":"object","properties":{
        "n":{"kind":"number"},"i":{"kind":"integer"},
        "rows":{"kind":"list","item":{"kind":"object","properties":{
            "title":{"kind":"text"},"count":{"kind":"counter"},
            "children":{"kind":"list","item":{"kind":"object","properties":{"enabled":{"kind":"boolean"}}}}
        }}}
    }});
    for n in [json!(1), json!(1.0), json!(-0.0), json!(1e20), json!(18_014_398_509_481_984u64), json!(i64::MIN)] {
        value["declaration"]["initial"] =
            json!({"n":n,"i":1.0,"rows":[{"title":"Original","count":2,"children":[{"enabled":true}]}]});
        let accepted = build::accept(&value.to_string(), root.path()).unwrap();
        let again = build::accept(&value.to_string(), root.path()).unwrap();
        assert_eq!(accepted.checkpoint(), again.checkpoint(), "template bytes remain deterministic");
        let saved: Value =
            serde_json::from_str(&Document::open(accepted.app().spec(), accepted.checkpoint(), &[]).unwrap().value())
                .unwrap();
        assert_eq!(saved["rows"][0]["title"], "Original");
        assert!(saved["rows"][0]["$id"].is_string());
        assert!(saved["rows"][0]["children"][0]["$id"].is_string());
    }
}

#[test]
fn resource_keys_roles_hashes_and_media_types_agree() {
    let root = tempfile::tempdir().unwrap();
    let baseline = input(root.path());
    for (pointer, wrong) in [
        ("/roles/ui", json!("other.js")),
        ("/roles/style", json!("ui.css")),
        ("/roles/commands", json!("commands.js")),
        ("/resources/0/kind", json!("command")),
        ("/resources/0/key", json!("../ui.js")),
        ("/resources/0/mediaType", json!("text/html")),
    ] {
        let mut value = baseline.clone();
        // roles are optional, so insert them when the pointer is absent.
        if let Some(slot) = value.pointer_mut(pointer) {
            *slot = wrong;
        } else {
            value["roles"][pointer.rsplit('/').next().unwrap()] = wrong;
        }
        refused(root.path(), &value);
    }
    let mut duplicate = baseline.clone();
    duplicate["resources"].as_array_mut().unwrap().push(baseline["resources"][0].clone());
    refused(root.path(), &duplicate);
    let mut wrong_hash = baseline.clone();
    let key = asset(root.path(), &mut wrong_hash, b"hello", "txt", "text/plain");
    fs::write(root.path().join(format!("resources/{key}")), "wrong").unwrap();
    refused(root.path(), &wrong_hash);
    let mut false_png = baseline.clone();
    asset(root.path(), &mut false_png, b"<script>evil()</script>", "png", "image/png");
    refused(root.path(), &false_png);
    let mut command = baseline.clone();
    command["declaration"]["commands"] =
        json!([{"name":"clear","description":"Clear","args":{"kind":"object","properties":{}}}]);
    refused(root.path(), &command);
    fs::write(root.path().join("resources/commands.js"), "globalThis.__slopCommands={};").unwrap();
    command["roles"]["commands"] = "commands.js".into();
    command["resources"].as_array_mut().unwrap().push(
        json!({"kind":"command","key":"commands.js","mediaType":"text/javascript","path":"resources/commands.js"}),
    );
    assert!(build::accept(&command.to_string(), root.path()).is_ok());
}

#[test]
fn only_regular_files_beneath_the_stage_are_read() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let baseline = input(root.path());
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("ui.js"), "external").unwrap();
    symlink(outside.path(), root.path().join("linked-directory")).unwrap();
    symlink(outside.path().join("ui.js"), root.path().join("linked-file")).unwrap();
    for path in [
        "../ui.js",
        "/etc/hosts",
        "resources/../resources/ui.js",
        "resources\\ui.js",
        "linked-file",
        "linked-directory/ui.js",
        "resources",
        "missing",
    ] {
        let mut value = baseline.clone();
        value["resources"][0]["path"] = path.into();
        refused(root.path(), &value);
    }
    let path = root.path().join("pipe");
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: a NUL-terminated path in the owned temporary directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let mut pipe = baseline.clone();
    pipe["resources"][0]["path"] = "pipe".into();
    refused(root.path(), &pipe); // O_NONBLOCK prevents a FIFO from hanging acceptance.
    symlink(root.path(), outside.path().join("root-link")).unwrap();
    refused(&outside.path().join("root-link"), &baseline);
}

#[test]
fn skin_and_artwork_bytes_use_the_full_png_checker() {
    let root = tempfile::tempdir().unwrap();
    let mut value = input(root.path());
    let key = asset(root.path(), &mut value, &png(640, 480), "png", "image/png");
    value["declaration"]["window"] = json!({"kind":"skin","width":320,"height":240,"image":format!("/assets/{key}")});
    value["roles"]["skin"] = key.clone().into();
    fs::write(root.path().join("preview.png"), png(8, 8)).unwrap();
    value["artwork"]["preview"] = "preview.png".into();
    let accepted = build::accept(&value.to_string(), root.path()).unwrap();
    assert_eq!(accepted.artwork().len(), 1);
    assert_eq!(accepted.assets().len(), 2, "artwork is not silently copied into assets");
    let good = value.clone();
    value["resources"].as_array_mut().unwrap().pop();
    refused(root.path(), &value);
    fs::write(root.path().join("preview.png"), &png(8, 8)[..33]).unwrap();
    refused(root.path(), &good);
}

#[test]
fn markers_and_budgets_precede_reads_and_allocations() {
    let future = r#"{"packageFormat":2,"runtimeABI":1,"future":1e999}"#;
    let error = build::accept(future, Path::new("/does-not-exist")).err().unwrap();
    assert!(matches!(error, hitslop_core::store::Error::Rejected(e) if e.code == Code::RequiresUpdate));
    let root = tempfile::tempdir().unwrap();
    let value = input(root.path());
    fs::OpenOptions::new()
        .write(true)
        .open(root.path().join("resources/ui.js"))
        .unwrap()
        .set_len(hitslop_core::ASSET_FILE_BYTES as u64 + 1)
        .unwrap();
    refused(root.path(), &value);
    let mut many = value.clone();
    many["resources"] = vec![value["resources"][0].clone(); hitslop_core::ASSET_COUNT + 1].into();
    refused(Path::new("/does-not-exist"), &many);
}
