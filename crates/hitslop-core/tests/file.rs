//! The document file: packing a build, the checks every open runs, creating and copying
//! documents, the writer lock and discovery, attachments, artwork, assets and recovery.
use hitslop_core::file::{self, Kind};
use hitslop_core::registry::{self, Lease};
use hitslop_core::store::{self, Error, Mode, Store};
use hitslop_core::{Code, Origin};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
mod support;
use support::{isolate_registry, registry_folder};

const MANIFEST: &str = r#"{"author":{"name":"Fixture"},"slug":"checklist","title":"Checklist","description":"A test document.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#;
const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#;
const INITIAL: &str = r#"{"title":"Initial"}"#;
const THEME: &str = r##"{"accent":"#335577"}"##;

/// A build's `app.json`: the `app` row, each part the JSON text a build writes.
#[derive(Clone, Copy)]
struct App<'a> {
    format: u64,
    abi: u64,
    manifest: &'a str,
    descriptor: &'a str,
    initial: &'a str,
    theme: &'a str,
}
const APP: App = App { format: hitslop_core::PACKAGE_FORMAT, abi: hitslop_core::RUNTIME_ABI, manifest: MANIFEST, descriptor: SCHEMA, initial: INITIAL, theme: THEME };
fn write_app(stage: &Path, app: App) {
    let App { format, abi, manifest, descriptor, initial, theme } = app;
    let json = format!(r#"{{"packageFormat":{format},"runtimeABI":{abi},"manifest":{manifest},"descriptor":{descriptor},"initial":{initial},"theme":{theme}}}"#);
    fs::write(stage.join("app.json"), json).unwrap();
}
/// What a crashed owner leaves behind: its discovery file, with its lock released.
fn crashed_owner(doc: &Path, json: &str) {
    let lease = Lease::acquire(doc).unwrap();
    lease.publish(json).unwrap();
    let file = fs::read_dir(registry_folder())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|e| e == "json") && fs::read_to_string(path).is_ok_and(|text| text == json))
        .unwrap();
    drop(lease);
    fs::write(file, json).unwrap();
}
/// A build's stage: its `app.json` and assets.
fn stage(dir: &Path) -> PathBuf {
    isolate_registry();
    let stage = dir.join("stage");
    fs::create_dir_all(stage.join("assets/fonts")).unwrap();
    write_app(&stage, APP);
    fs::write(stage.join("assets/app.js"), "export default { mount() { return {}; } };\n".repeat(64)).unwrap();
    fs::write(stage.join("assets/app.css"), "body { color: var(--slop-accent); }").unwrap();
    fs::write(stage.join("assets/fonts/face.woff2"), (0..=255u8).cycle().take(4096).collect::<Vec<u8>>()).unwrap();
    stage
}
fn template(dir: &Path) -> PathBuf {
    let out = dir.join("Template.slop");
    file::pack(&stage(dir), &out).unwrap();
    out
}
fn document(dir: &Path) -> PathBuf {
    let doc = dir.join("Doc.slop");
    file::create_document(&template(dir), &doc).unwrap();
    doc
}
fn raw(path: &Path) -> Connection {
    Connection::open(path).unwrap()
}
fn code(error: Error) -> Code {
    match error {
        Error::Rejected(e) => e.code,
        other => panic!("expected a refusal, got {other:?}"),
    }
}
fn temporaries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".tmp")).collect()
}
/// Each marker raised one past what this build writes.
fn raised(doc: &Path) -> [String; 3] {
    let storage: i64 = raw(doc).query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    [
        format!("PRAGMA user_version={}", storage + 1),
        format!("UPDATE app SET package_format={}", hitslop_core::PACKAGE_FORMAT + 1),
        format!("UPDATE app SET runtime_abi={}", hitslop_core::RUNTIME_ABI + 1),
    ]
}
fn png(width: u32, height: u32, color_type: u8) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, color_type, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[test]
fn a_stage_packs_into_a_template_and_a_rebuild_replaces_only_templates() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    fs::create_dir(stage.join("artwork")).unwrap();
    fs::write(stage.join("artwork/preview.png"), png(640, 480, 6)).unwrap();
    let (a, b) = (dir.path().join("A.slop"), dir.path().join("B.slop"));
    file::pack(&stage, &a).unwrap();
    file::pack(&stage, &b).unwrap();
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap(), "packing is reproducible");
    let opened = file::open(&a, true).unwrap();
    assert_eq!(opened.kind, Kind::Template);
    assert_eq!((opened.app.package_format, opened.app.runtime_abi), (APP.format, APP.abi));
    assert_eq!(opened.app.descriptor, SCHEMA);
    assert_eq!(opened.theme_tokens, vec![("accent".to_string(), "#335577".to_string())]);
    let manifest: serde_json::Value = serde_json::from_str(&opened.app.manifest).unwrap();
    assert!(manifest.get("packageFormat").is_none() && manifest.get("runtimeABI").is_none(), "the markers are columns");
    assert_eq!(store::artwork(&a, &["preview"]).unwrap().map(|(_, png)| png), Some(png(640, 480, 6)));
    let summary = file::inspect(&a).unwrap();
    assert_eq!(summary["kind"], "template");
    let assets: Vec<&str> = summary["assets"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(assets, ["app.css", "app.js", "fonts/face.woff2"]);
    // A rebuild replaces a template; it never replaces a document, or anything else.
    let folder = dir.path().join("Folder.slop");
    fs::create_dir(&folder).unwrap();
    assert_eq!(code(file::pack(&stage, &folder).unwrap_err()), Code::Exists);
    file::pack(&stage, &a).unwrap();
    let doc = dir.path().join("Doc.slop");
    file::create_document(&a, &doc).unwrap();
    let before = fs::read(&doc).unwrap();
    assert_eq!(code(file::pack(&stage, &doc).unwrap_err()), Code::Exists);
    assert_eq!(fs::read(&doc).unwrap(), before);
    assert!(temporaries(dir.path()).is_empty());
}

/// The `app` row holds one-line JSON whatever the build's spacing, so hosts pass it on
/// as stored; text inside strings is kept exactly.
#[test]
fn pack_stores_the_app_as_compact_json() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let descriptor = "{ \"kind\" : \"object\",\n  \"properties\": { \"title\": { \"kind\": \"string\" } } }";
    let initial = r#"{ "title" : "Spaced \"and\" \\ quoted" }"#;
    write_app(&stage, App { descriptor, initial, ..APP });
    let out = dir.path().join("Compact.slop");
    file::pack(&stage, &out).unwrap();
    let opened = file::open(&out, true).unwrap();
    assert_eq!(opened.app.descriptor, SCHEMA);
    assert_eq!(opened.app.initial, r#"{"title":"Spaced \"and\" \\ quoted"}"#);
}

#[test]
fn pack_checks_the_whole_build_before_publishing_anything() {
    type Damage = fn(&Path);
    let cases: [(&str, Damage); 11] = [
        ("no app module", |s| fs::remove_file(s.join("assets/app.js")).unwrap()),
        ("app module not UTF-8", |s| fs::write(s.join("assets/app.js"), [0xff, 0xfe]).unwrap()),
        ("descriptor", |s| write_app(s, App { descriptor: r#"{"kind":"nope"}"#, ..APP })),
        ("initial values", |s| write_app(s, App { initial: r#"{"title":7}"#, ..APP })),
        ("theme", |s| write_app(s, App { theme: r#"{"accent":"blue"}"#, ..APP })),
        ("manifest", |s| write_app(s, App { manifest: r#"{"author":{"name":"Fixture"},"slug":"checklist","title":"","description":"A test document.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#, ..APP })),
        ("markers in the manifest", |s| write_app(s, App { manifest: r#"{"author":{"name":"Fixture"},"slug":"checklist","title":"Checklist","description":"A test document.","categories":["utilities"],"presentation":{"width":320,"height":240},"runtimeABI":1}"#, ..APP })),
        ("missing markers", |s| fs::write(s.join("app.json"), format!(r#"{{"manifest":{MANIFEST},"descriptor":{SCHEMA},"initial":{INITIAL},"theme":{THEME}}}"#)).unwrap()),
        ("unknown field", |s| fs::write(s.join("app.json"), format!(r#"{{"packageFormat":1,"runtimeABI":1,"manifest":{MANIFEST},"descriptor":{SCHEMA},"initial":{INITIAL},"theme":{THEME},"extra":1}}"#)).unwrap()),
        ("symbolic link", |s| std::os::unix::fs::symlink("/etc/hosts", s.join("assets/hosts")).unwrap()),
        ("artwork", |s| {
            fs::create_dir(s.join("artwork")).unwrap();
            fs::write(s.join("artwork/icon.png"), b"not a png").unwrap();
        }),
    ];
    for (name, damage) in cases {
        let dir = tempfile::tempdir().unwrap();
        let stage = stage(dir.path());
        damage(&stage);
        let out = dir.path().join("Out.slop");
        assert!(file::pack(&stage, &out).is_err(), "{name}");
        assert!(!out.exists(), "{name}: nothing published");
        assert!(temporaries(dir.path()).is_empty(), "{name}: no temporary file left");
    }
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    fs::write(stage.join("assets/big.bin"), vec![0u8; hitslop_core::ASSET_FILE_BYTES + 1]).unwrap();
    assert!(file::pack(&stage, &dir.path().join("Big.slop")).is_err(), "an asset over 25 MiB");
    write_app(&stage, App { abi: 99, ..APP });
    fs::remove_file(stage.join("assets/big.bin")).unwrap();
    assert_eq!(code(file::pack(&stage, &dir.path().join("Newer.slop")).unwrap_err()), Code::RequiresUpdate);
}

#[test]
fn a_skin_must_be_an_rgba_asset_the_size_of_the_window() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let skinned = MANIFEST.replace(r#""presentation":{"width":320,"height":240}"#, r#""presentation":{"width":320,"height":240,"skin":"assets/skin.png"}"#);
    write_app(&stage, App { manifest: &skinned, ..APP });
    assert!(file::pack(&stage, &dir.path().join("Missing.slop")).is_err(), "missing skin");
    fs::write(stage.join("assets/skin.png"), png(320, 240, 2)).unwrap();
    assert!(file::pack(&stage, &dir.path().join("Opaque.slop")).is_err(), "a skin needs alpha");
    fs::write(stage.join("assets/skin.png"), png(300, 240, 6)).unwrap();
    assert!(file::pack(&stage, &dir.path().join("Wrong.slop")).is_err(), "a skin is the window's size");
    fs::write(stage.join("assets/skin.png"), png(320, 240, 6)).unwrap();
    let out = dir.path().join("Skinned.slop");
    file::pack(&stage, &out).unwrap();
    assert_eq!(file::open(&out, true).unwrap().skin, Some(png(320, 240, 6)));
}

#[test]
fn hostile_layouts_and_rows_are_refused_before_any_value_is_read() {
    for ddl in [
        "CREATE TABLE extra(x)",
        "CREATE INDEX extra_index ON assets(bytes)",
        "CREATE VIEW extra_view AS SELECT 1",
        "CREATE TRIGGER extra_trigger AFTER INSERT ON updates BEGIN DELETE FROM attachments; END",
        "INSERT INTO assets VALUES('../escape.js', 'identity', 1, x'00')",
        "INSERT INTO assets VALUES('short.js', 'identity', 2, x'00')",
        "INSERT INTO assets VALUES('unnumbered.js', 'identity', 'one', x'00')",
        "INSERT INTO assets VALUES('expanded.js', 'br', 2, x'789c')",
        "INSERT INTO updates(bytes) VALUES(x'00')",
        "DELETE FROM app",
        "INSERT INTO attachments VALUES('not-an-id', x'00')",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let doc = document(dir.path());
        let damaged = if ddl.starts_with("INSERT INTO updates") { template(dir.path()) } else { doc.clone() };
        raw(&damaged).execute_batch(ddl).unwrap();
        assert!(file::open(&damaged, true).is_err(), "{ddl}");
        assert!(Store::open(&damaged, Mode::Snapshot).is_err(), "{ddl}");
    }
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    raw(&doc).execute("INSERT INTO assets VALUES('big.bin', 'identity', ?1, zeroblob(?1))", [hitslop_core::ASSET_FILE_BYTES as i64 + 1]).unwrap();
    let started = std::time::Instant::now();
    assert_eq!(code(file::open(&doc, true).err().unwrap()), Code::InvalidRequest);
    assert!(started.elapsed() < std::time::Duration::from_millis(500), "sizes come from length(), not the bytes");
    // A compressed asset is bounded by what it decodes to, before anything is decoded.
    let doc = document(tempfile::tempdir().unwrap().keep().as_path());
    raw(&doc).execute("INSERT INTO assets VALUES('bomb.js', 'br', ?, x'789c')", [hitslop_core::ASSET_FILE_BYTES as i64 + 1]).unwrap();
    assert_eq!(code(file::open(&doc, true).err().unwrap()), Code::InvalidRequest);
    // SQLite's own primary-key indexes are part of the expected layout, not refused.
    let fresh = document(tempfile::tempdir().unwrap().keep().as_path());
    let indexes: i64 = raw(&fresh)
        .query_row("SELECT count(*) FROM sqlite_master WHERE type='index' AND name LIKE 'sqlite_autoindex_%'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(indexes, 3);
    file::open(&fresh, true).unwrap();
}

/// What pack and a save never write is refused on open, sizes before values: artwork that
/// is not an image within the limits, a theme over its budget, and a second or misnumbered
/// row in a one-row table.
#[test]
fn stored_values_are_bounded_as_writes_bound_them() {
    let many_tokens = format!("{{{}}}", (0..100_000).map(|i| format!(r##""t{i}":"#000000""##)).collect::<Vec<_>>().join(","));
    let long_override = format!(r#"{{"accent":"{}"}}"#, "a".repeat(1 << 20));
    let cases: [(&str, &str, Option<Vec<u8>>); 7] = [
        ("oversized artwork", "INSERT INTO artwork VALUES('preview', ?)", Some(png(100_000, 100_000, 6))),
        ("artwork that is not a PNG", "INSERT INTO artwork VALUES('icon', ?)", Some(b"not a png".to_vec())),
        ("theme defaults over budget", "UPDATE app SET theme=CAST(? AS TEXT)", Some(many_tokens.into_bytes())),
        ("theme overrides over budget", "UPDATE document SET theme=CAST(? AS TEXT)", Some(long_override.into_bytes())),
        ("a misnumbered checkpoint", "PRAGMA ignore_check_constraints=ON; INSERT INTO checkpoint VALUES(2,'key',x'00')", None),
        ("a second document row", "PRAGMA ignore_check_constraints=ON; INSERT INTO document VALUES(2,'{}')", None),
        ("a misnumbered app row", "PRAGMA ignore_check_constraints=ON; UPDATE app SET id=2", None),
    ];
    let mut accepted = vec![];
    for (name, sql, value) in cases {
        let dir = tempfile::tempdir().unwrap();
        let doc = document(dir.path());
        let conn = raw(&doc);
        match value {
            Some(value) => conn.execute(sql, [value]).map(|_| ()).unwrap(),
            None => conn.execute_batch(sql).unwrap(),
        }
        drop(conn);
        let started = std::time::Instant::now();
        let refused = file::open(&doc, false).is_err();
        // Refused from sizes, never by reading the value.
        if !refused || started.elapsed() > std::time::Duration::from_millis(500) || Store::open(&doc, Mode::Snapshot).is_ok() {
            accepted.push(name);
        }
    }
    assert!(accepted.is_empty(), "accepted, or refused only after reading the value: {accepted:?}");
}

#[test]
fn newer_markers_ask_for_an_update_and_foreign_files_are_refused() {
    for index in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let doc = document(dir.path());
        let change = &raised(&doc)[index];
        raw(&doc).execute_batch(change).unwrap();
        let before = fs::read(&doc).unwrap();
        for refusal in [file::open(&doc, true).err(), Store::open(&doc, Mode::Document).err(), Store::open(&doc, Mode::Snapshot).err()] {
            assert_eq!(code(refusal.expect("refused")), Code::RequiresUpdate, "{change}");
        }
        assert_eq!(fs::read(&doc).unwrap(), before, "{change}: nothing written");
    }
    let dir = tempfile::tempdir().unwrap();
    let foreign = dir.path().join("Foreign.slop");
    raw(&foreign).execute_batch("PRAGMA application_id=1; CREATE TABLE t(x);").unwrap();
    assert!(file::open(&foreign, true).err().unwrap().to_string().contains("not a hitSlop document"));
}

#[test]
fn templates_create_documents_and_never_open_as_one() {
    let dir = tempfile::tempdir().unwrap();
    let template = template(dir.path());
    assert_eq!(code(Store::open(&template, Mode::Document).err().unwrap()), Code::IsTemplate);
    let snapshot = Store::open(&template, Mode::Snapshot).unwrap();
    assert!(snapshot.document().unwrap().value().unwrap().contains("Initial"), "a template renders its initial values");
    let (a, b) = (dir.path().join("A.slop"), dir.path().join("B.slop"));
    file::create_document(&template, &a).unwrap();
    file::create_document(&template, &b).unwrap();
    assert_eq!(file::open(&a, true).unwrap().kind, Kind::Document);
    assert_eq!(file::open(&b, true).unwrap().kind, Kind::Document);
}

/// A store keeps the app its open checked, in either mode, so a host shows the document
/// from it without opening the file again. A file's kind comes from its header alone.
#[test]
fn a_store_keeps_the_app_its_open_checked() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let read = file::open(&doc, false).unwrap();
    for mode in [Mode::Document, Mode::Snapshot] {
        let store = Store::open(&doc, mode).unwrap();
        let app = store.app();
        assert_eq!(app.kind, Kind::Document);
        assert_eq!(app.canonical_descriptor, read.canonical_descriptor);
        assert_eq!(app.silhouette, read.silhouette);
        assert_eq!((&app.theme_tokens, &app.skin), (&read.theme_tokens, &read.skin));
        assert_eq!((&app.app.manifest, &app.app.descriptor, &app.app.theme), (&read.app.manifest, &read.app.descriptor, &read.app.theme));
        store.close().unwrap();
    }
    assert_eq!(file::kind(&doc).unwrap(), Kind::Document);
    assert_eq!(file::kind(&dir.path().join("Template.slop")).unwrap(), Kind::Template);
}

/// A template whose app fails its checks makes no document: nothing is published, so a
/// refused creation leaves no file to clean up.
#[test]
fn a_template_with_a_broken_app_creates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let template = template(dir.path());
    raw(&template).execute("DELETE FROM assets WHERE path='app.js'", []).unwrap();
    let dest = dir.path().join("Doc.slop");
    assert_eq!(code(file::create_document(&template, &dest).unwrap_err()), Code::InvalidRequest);
    assert!(!dest.exists(), "a refused creation publishes nothing");
    assert!(temporaries(dir.path()).is_empty());
}

#[test]
fn create_and_copy_never_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let taken = dir.path().join("Taken.slop");
    fs::write(&taken, "keep me").unwrap();
    assert_eq!(code(file::create_document(&dir.path().join("Template.slop"), &taken).unwrap_err()), Code::Exists);
    let store = Store::open(&doc, Mode::Document).unwrap();
    assert_eq!(code(store.copy_to(&taken).unwrap_err()), Code::Exists);
    store.close().unwrap();
    assert_eq!(fs::read_to_string(&taken).unwrap(), "keep me");
    assert!(temporaries(dir.path()).is_empty());
}

#[test]
fn the_registry_admits_one_writer_and_names_the_live_owner() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    assert!(matches!(Lease::acquire(&doc), Err(Error::Locked)), "a second descriptor is refused");
    assert!(matches!(Store::open(&doc, Mode::Document), Err(Error::Locked)));
    assert_eq!(registry::discovery(&doc).unwrap(), None);
    store.publish_discovery(r#"{"socket":"/tmp/owner"}"#).unwrap();
    assert_eq!(registry::discovery(&doc).unwrap().as_deref(), Some(r#"{"socket":"/tmp/owner"}"#));
    store.close().unwrap();
    assert_eq!(registry::discovery(&doc).unwrap(), None, "closing withdraws discovery");
    // A crashed owner leaves its discovery behind (the lock dies with it); the next holder
    // removes it.
    crashed_owner(&doc, "stale");
    assert_eq!(registry::discovery(&doc).unwrap().as_deref(), Some("stale"));
    let next = Lease::acquire(&doc).unwrap();
    assert_eq!(registry::discovery(&doc).unwrap(), None);
    drop(next);
    // A lease that goes away withdraws its own discovery.
    let lease = Lease::acquire(&doc).unwrap();
    lease.publish("dropped").unwrap();
    drop(lease);
    assert_eq!(registry::discovery(&doc).unwrap(), None);
    // A sweep clears a crashed owner's discovery and keeps a live owner's.
    let other_dir = tempfile::tempdir().unwrap();
    let other = document(other_dir.path());
    let live = Lease::acquire(&other).unwrap();
    live.publish("live").unwrap();
    crashed_owner(&doc, "crashed");
    registry::sweep().unwrap();
    assert_eq!(registry::discovery(&doc).unwrap(), None);
    assert_eq!(registry::discovery(&other).unwrap().as_deref(), Some("live"));
    drop(live);
}

#[test]
fn a_rename_or_a_hard_link_stops_the_writer() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let mut state = store.document().unwrap();
    fs::rename(&doc, dir.path().join("Renamed.slop")).unwrap();
    state.apply_batch(r#"{"intents":[{"type":"set","path":["title"],"value":"After"}]}"#, Origin::Page).unwrap();
    let job = store.job(&mut state, false).unwrap().unwrap();
    assert!(matches!(store.write(&job), Err(Error::Moved)));
    // Moved back, the writer saves what it kept, although Apple's SQLite never writes
    // through a connection whose file was renamed.
    fs::rename(dir.path().join("Renamed.slop"), &doc).unwrap();
    store.write(&job).unwrap();
    fs::hard_link(&doc, dir.path().join("Link.slop")).unwrap();
    assert!(matches!(store.check(false), Err(Error::Moved)), "a hard link would split the journal's name");
    fs::remove_file(dir.path().join("Link.slop")).unwrap();
    state.apply_batch(r#"{"intents":[{"type":"set","path":["title"],"value":"Unlinked"}]}"#, Origin::Page).unwrap();
    let job = store.job(&mut state, false).unwrap().unwrap();
    store.write(&job).unwrap();
    store.put_attachment(b"after the link").unwrap();
    store.close().unwrap();
    let reopened = Store::open(&doc, Mode::Snapshot).unwrap();
    assert!(reopened.document().unwrap().value().unwrap().contains("Unlinked"));
    fs::rename(&doc, dir.path().join("Renamed.slop")).unwrap();
    let doc = dir.path().join("Renamed.slop");
    let store = Store::open(&doc, Mode::Document).unwrap();
    fs::hard_link(&doc, dir.path().join("Link.slop")).unwrap();
    store.close().unwrap();
    assert_eq!(code(Lease::acquire(&doc).err().unwrap()), Code::InvalidRequest);
}

#[test]
fn attachments_are_content_addressed_bounded_and_verified() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let first = store.put_attachment(b"hello").unwrap();
    assert_eq!(first.id, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", "SHA-256");
    assert_eq!(store.put_attachment(b"hello").unwrap(), first, "storing the same bytes again is a no-op");
    assert_eq!(store.attachment(&first.id).unwrap(), b"hello");
    assert_eq!(store.attachments().unwrap(), vec![first.clone()]);
    assert_eq!(code(store.put_attachment(&vec![0; hitslop_core::ATTACHMENT_FILE_BYTES + 1]).unwrap_err()), Code::TooLarge);
    assert_eq!(code(store.attachment(&"0".repeat(64)).unwrap_err()), Code::PathNotFound);
    assert_eq!(code(store.attachment("../x").unwrap_err()), Code::InvalidId);
    // A document holds a bounded number of attachments; bytes it already holds still store.
    for index in 1..hitslop_core::ATTACHMENT_COUNT {
        store.put_attachment(format!("blob-{index}").as_bytes()).unwrap();
    }
    assert_eq!(code(store.put_attachment(b"one too many").unwrap_err()), Code::TooLarge);
    assert_eq!(store.put_attachment(b"hello").unwrap(), first);
    store.close().unwrap();
    // A snapshot reads attachments through its own connection.
    let snapshot = Store::open(&doc, Mode::Snapshot).unwrap();
    assert_eq!(snapshot.attachment(&first.id).unwrap(), b"hello");
    assert!(snapshot.put_attachment(b"no").is_err(), "snapshots own nothing");
    raw(&doc).execute("UPDATE attachments SET bytes=x'00'", []).unwrap();
    assert!(snapshot.attachment(&first.id).unwrap_err().to_string().contains("checksum"));
}

/// Storing bytes the document already holds succeeds only when the stored copy is intact:
/// a successful import always returns a readable attachment. Damage is refused, never
/// repaired in passing.
#[test]
fn reimporting_over_damaged_bytes_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let stored = store.put_attachment(b"hello").unwrap();
    store.close().unwrap();
    raw(&doc).execute("UPDATE attachments SET bytes=x'00'", []).unwrap();
    let store = Store::open(&doc, Mode::Document).unwrap();
    assert!(store.put_attachment(b"hello").unwrap_err().to_string().contains("checksum"));
    assert!(store.attachment(&stored.id).unwrap_err().to_string().contains("checksum"), "the damage is kept for recovery");
    store.close().unwrap();
}

/// Hosts that display a file take the first artwork it holds, in one read, and learn that
/// a file they cannot read now is busy, never that it has no artwork.
#[test]
fn display_reads_fall_back_in_one_read_and_report_a_busy_file() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    fs::create_dir_all(stage.join("artwork")).unwrap();
    fs::write(stage.join("artwork/preview.png"), png(640, 480, 6)).unwrap();
    let (template, doc) = (dir.path().join("T.slop"), dir.path().join("D.slop"));
    file::pack(&stage, &template).unwrap();
    file::create_document(&template, &doc).unwrap();
    let (name, preview) = store::artwork(&doc, &["icon", "preview"]).unwrap().unwrap();
    assert_eq!((name.as_str(), preview), ("preview", png(640, 480, 6)), "a build without an icon falls back to its preview");
    assert_eq!(store::artwork(&doc, &["icon"]).unwrap(), None);
    let holder = raw(&doc);
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert!(matches!(store::artwork(&doc, &["preview"]), Err(Error::Busy)));
    holder.execute_batch("ROLLBACK").unwrap();
}

/// A window writes its artwork through its own writer as it closes; a snapshot owns
/// nothing to write with.
#[test]
fn artwork_is_written_by_the_writer() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let (preview, icon) = (png(640, 480, 6), png(512, 512, 6));
    let snapshot = Store::open(&doc, Mode::Snapshot).unwrap();
    assert!(matches!(snapshot.set_artwork(&[("preview", &preview)]), Err(Error::Closed)));
    let store = Store::open(&doc, Mode::Document).unwrap();
    store.set_artwork(&[("preview", &preview), ("icon", &icon)]).unwrap();
    assert_eq!(store.artwork("icon").unwrap(), Some(icon));
    store.close().unwrap();
    assert_eq!(store::artwork(&doc, &["preview"]).unwrap().map(|(_, png)| png), Some(preview));
}

/// Each close replaces the artwork; the pages the old images held leave the file.
#[test]
fn replaced_artwork_leaves_no_free_pages() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let sized = |bytes: usize| {
        let mut png = png(640, 480, 6);
        png.resize(bytes, 0x5a);
        png
    };
    let store = Store::open(&doc, Mode::Document).unwrap();
    store.set_artwork(&[("preview", &sized(96 * 1024)), ("icon", &sized(24 * 1024))]).unwrap();
    store.set_artwork(&[("preview", &sized(16 * 1024)), ("icon", &sized(4 * 1024))]).unwrap();
    store.close().unwrap();
    let free: i64 = raw(&doc).query_row("PRAGMA freelist_count", [], |r| r.get(0)).unwrap();
    assert_eq!(free, 0);
}

#[test]
fn an_open_document_copies_through_its_owner() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let mut state = store.document().unwrap();
    state.apply_batch(r#"{"intents":[{"type":"set","path":["title"],"value":"Shared"}]}"#, Origin::Page).unwrap();
    store.write(&store.job(&mut state, false).unwrap().unwrap()).unwrap();
    store.put_attachment(b"photo").unwrap();
    let copy = dir.path().join("Copy.slop");
    store.copy_to(&copy).unwrap();
    assert_eq!(code(store.copy_to(&copy).unwrap_err()), Code::Exists);
    // The owner keeps saving after the copy.
    state.apply_batch(r#"{"intents":[{"type":"set","path":["title"],"value":"Owner"}]}"#, Origin::Page).unwrap();
    store.write(&store.job(&mut state, false).unwrap().unwrap()).unwrap();
    let copied = Store::open(&copy, Mode::Document).unwrap();
    assert!(copied.document().unwrap().value().unwrap().contains("Shared"));
    assert_eq!(copied.attachments().unwrap().len(), 1);
}

#[test]
fn assets_are_served_whole_or_in_ranges() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let reader = file::AssetReader::open(&doc).unwrap();
    // Text is stored compressed and served as written; media is stored as it is.
    let stored = |path: &str| -> (i64, i64) {
        raw(&doc).query_row("SELECT length(bytes), size FROM assets WHERE path=?", [path], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
    };
    let (compressed, size) = stored("app.js");
    assert!(compressed < size, "app.js is stored compressed");
    assert_eq!(stored("fonts/face.woff2"), (4096, 4096));
    let script = fs::read(dir.path().join("stage/assets/app.js")).unwrap();
    assert_eq!(reader.size("app.js").unwrap(), Some(script.len() as u64));
    assert_eq!(reader.read_range("app.js", 0, u64::MAX).unwrap(), Some(script.clone()));
    assert_eq!(reader.read_range("app.js", 100, 50).unwrap(), Some(script[100..150].to_vec()));
    assert_eq!(reader.read_range("app.js", script.len() as u64 - 10, 500).unwrap(), Some(script[script.len() - 10..].to_vec()));
    assert_eq!(reader.read_range("app.js", u64::MAX, 10).unwrap(), Some(vec![]));
    let font: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    assert_eq!(reader.size("fonts/face.woff2").unwrap(), Some(4096));
    assert_eq!(reader.read_range("fonts/face.woff2", 0, u64::MAX).unwrap(), Some(font.clone()));
    assert_eq!(reader.read_range("fonts/face.woff2", 100, 50).unwrap(), Some(font[100..150].to_vec()));
    assert_eq!(reader.read_range("fonts/face.woff2", 4000, 500).unwrap(), Some(font[4000..].to_vec()), "clamped to the asset");
    assert_eq!(reader.read_range("missing.js", 0, u64::MAX).unwrap(), None);
    assert_eq!(file::content_type("fonts/face.woff2"), "font/woff2");
    assert_eq!(file::content_type("worklet.mjs"), "text/javascript");
    assert_eq!(file::content_type("module.wasm"), "application/wasm");
    assert_eq!(file::content_type("data.unknown"), "application/octet-stream");
    assert_eq!(file::descriptor(&doc).unwrap(), SCHEMA);
    // Damaged compressed text is an error, never served.
    raw(&doc).execute("UPDATE assets SET bytes=x'00' || substr(bytes, 2) WHERE path='app.js'", []).unwrap();
    assert!(reader.read_range("app.js", 0, u64::MAX).is_err());
    assert!(file::open(&doc, false).is_err(), "the app's checks read app.js");
}

#[test]
fn artwork_is_checked_as_every_open_checks_it() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let preview = png(640, 480, 6);
    store.set_artwork(&[("preview", &preview)]).unwrap();
    let mut oversized = png(640, 480, 6);
    oversized.resize(hitslop_core::ASSET_FILE_BYTES + 1, 0);
    let huge = png(100_000, 100_000, 6);
    for (name, bytes) in [
        ("preview", oversized.as_slice()),
        ("icon", b"not a png".as_slice()),
        ("icon", png(0, 0, 6).as_slice()),
        ("icon", huge.as_slice()),
        ("splash", preview.as_slice()),
    ] {
        assert!(store.set_artwork(&[(name, bytes)]).is_err(), "{name}: refused");
    }
    store.close().unwrap();
    // The document still opens, with the artwork it had.
    file::open(&doc, true).unwrap();
    assert_eq!(store::artwork(&doc, &["preview"]).unwrap().map(|(_, png)| png), Some(preview));
    assert_eq!(store::artwork(&doc, &["icon"]).unwrap().map(|(_, png)| png), None);
}

/// A file a newer build wrote, as that build might have left it: one of its markers raised
/// (`raised`), in rollback (`DELETE`) or `WAL` journaling. In WAL, the change stays in the
/// `-wal` file.
fn newer(dir: &Path, marker: usize, wal: bool) -> (PathBuf, String) {
    let doc = document(dir);
    let change = raised(&doc)[marker].clone();
    let conn = raw(&doc);
    conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true).unwrap();
    if wal {
        conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(())).unwrap();
    }
    conn.execute_batch(&change).unwrap();
    drop(conn);
    (doc, change)
}
fn sidecars(doc: &Path) -> Vec<(String, Vec<u8>)> {
    ["-wal", "-journal"]
        .iter()
        .filter_map(|suffix| {
            let path = PathBuf::from(format!("{}{suffix}", doc.display()));
            fs::read(&path).ok().map(|bytes| (suffix.to_string(), bytes))
        })
        .collect()
}

#[test]
fn refusing_a_newer_file_writes_nothing_in_any_journal_mode() {
    for index in 0..3 {
        for wal in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (doc, change) = newer(dir.path(), index, wal);
            let (before, sidecars_before) = (fs::read(&doc).unwrap(), sidecars(&doc));
            assert_eq!(sidecars_before.iter().any(|(s, _)| s == "-wal"), wal, "{change}: the newer build's WAL is in place");
            let refusals = [
                Store::open(&doc, Mode::Document).err().map(code),
                Store::open(&doc, Mode::Snapshot).err().map(code),
                file::open(&doc, true).err().map(code),
            ];
            for refusal in refusals {
                assert_eq!(refusal, Some(Code::RequiresUpdate), "{change}, wal {wal}");
            }
            assert_eq!(fs::read(&doc).unwrap(), before, "{change}, wal {wal}: the file is unchanged");
            assert_eq!(sidecars(&doc), sidecars_before, "{change}, wal {wal}: its journal is kept as it was");
        }
    }
}

/// SQLite rolls back a crashed write's hot journal whenever it reads the file: its own
/// recovery, which restores the last committed state. A newer file is refused after that,
/// and holds exactly what its build committed.
#[test]
fn a_newer_file_with_a_crashed_write_is_restored_then_refused() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    store.put_attachment(&vec![7u8; 4 << 20]).unwrap();
    store.close().unwrap();
    raw(&doc).execute_batch(&raised(&doc)[0]).unwrap();
    let committed = fs::read(&doc).unwrap();
    let status = support::child("crash_mid_commit", &[("HITSLOP_CRASH_DOCUMENT", doc.to_str().unwrap())]).status().unwrap();
    assert!(!status.success(), "the child died mid-commit");
    let journal = dir.path().join("Doc.slop-journal");
    assert!(journal.exists(), "a hot journal is left beside the file");
    assert_eq!(code(Store::open(&doc, Mode::Document).err().unwrap()), Code::RequiresUpdate);
    assert!(!journal.exists(), "SQLite rolled the crashed write back");
    assert_eq!(fs::read(&doc).unwrap(), committed, "the file holds what its build committed");
}

/// Runs only as the child of `taking_the_writer_lock_never_drops_this_process_sqlite_locks`:
/// a writer that bypasses the registry, as another SQLite program would. Exits 0 when the
/// file is busy, 3 when it got the exclusive lock.
#[test]
#[ignore]
fn exclusive_sqlite_writer() {
    let Ok(path) = std::env::var("HITSLOP_EXCLUSIVE_DOCUMENT") else { return };
    let conn = Connection::open(&path).unwrap();
    conn.busy_timeout(std::time::Duration::ZERO).unwrap();
    match conn.execute_batch("BEGIN EXCLUSIVE") {
        Err(e) if e.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) => std::process::exit(0),
        Err(e) => panic!("{e}"),
        Ok(()) => std::process::exit(3),
    }
}

/// POSIX record locks belong to the process: closing any descriptor on the database file
/// releases every lock SQLite holds on it through this process's other connections
/// (sqlite.org/howtocorrupt.html, "POSIX advisory locks canceled by a separate thread doing
/// close()"). Taking the writer lock must never open the database file.
#[test]
fn taking_the_writer_lock_never_drops_this_process_sqlite_locks() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let reader = raw(&doc);
    reader.execute_batch("BEGIN").unwrap();
    reader.query_row("SELECT count(*) FROM app", [], |r| r.get::<_, i64>(0)).unwrap();
    let lease = Lease::acquire(&doc).unwrap();
    let status = support::child("exclusive_sqlite_writer", &[("HITSLOP_EXCLUSIVE_DOCUMENT", doc.to_str().unwrap())]).status().unwrap();
    assert_eq!(status.code(), Some(0), "another writer must find the file busy while this process reads it");
    reader.execute_batch("COMMIT").unwrap();
    drop(lease);
}

/// Runs only as the child of `a_crash_mid_commit_is_recovered_by_the_next_writer`: starts a
/// large in-place rewrite with a tiny page cache, so changed pages spill into the file, and
/// dies before committing.
#[test]
#[ignore]
fn crash_mid_commit() {
    let Ok(path) = std::env::var("HITSLOP_CRASH_DOCUMENT") else { return };
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA cache_size=10; PRAGMA cache_spill=ON; BEGIN IMMEDIATE; UPDATE attachments SET bytes=randomblob(length(bytes));").unwrap();
    std::process::abort();
}

#[test]
fn a_crash_mid_commit_is_recovered_by_the_next_writer() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let attachment = store.put_attachment(&vec![7u8; 4 << 20]).unwrap();
    store.close().unwrap();
    let status = support::child("crash_mid_commit", &[("HITSLOP_CRASH_DOCUMENT", doc.to_str().unwrap())]).status().unwrap();
    assert!(!status.success(), "the child died mid-commit");
    let journal = dir.path().join("Doc.slop-journal");
    assert!(journal.exists(), "a hot journal is left beside the document");
    let store = Store::open(&doc, Mode::Document).unwrap();
    assert!(!journal.exists(), "the writer rolled it back");
    assert_eq!(store.attachment(&attachment.id).unwrap(), vec![7u8; 4 << 20], "the committed attachment is intact");
}
