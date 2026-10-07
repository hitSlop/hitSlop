//! The document file: packing a build, the checks every open runs, creating and copying
//! documents, the writer lock and discovery, attachments, artwork, assets and recovery.
use hitslop_core::file::{self, Artwork, Kind, TemplateSource};
use hitslop_core::registry::{self, Lease};
use hitslop_core::store::{Error, Mode, Store};
use hitslop_core::{Code, Origin};
use rusqlite::{Connection, config::DbConfig};
use std::fs;
use std::path::{Path, PathBuf};
mod support;
use support::{App, isolate_registry, registry_folder, write_app};

const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#;
const INITIAL: &str = r#"{"title":"Initial"}"#;
const APP: App = App::new(SCHEMA, INITIAL);
/// What a crashed owner leaves behind: its discovery file, with its lock released.
fn crashed_owner(doc: &Path, json: &str) {
    let lease = Lease::acquire(doc).unwrap();
    lease.publish(json).unwrap();
    let file = fs::read_dir(registry_folder())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().is_some_and(|e| e == "json") && fs::read_to_string(path).is_ok_and(|text| text == json)
        })
        .unwrap();
    drop(lease);
    fs::write(file, json).unwrap();
}
/// A build's stage: its explicit BuildInput and assets.
fn stage(dir: &Path) -> PathBuf {
    isolate_registry();
    let stage = dir.join("stage");
    fs::create_dir_all(stage.join("assets")).unwrap();
    write_app(&stage, APP);
    fs::write(stage.join("assets/ui.js"), "export default { mount() { return {}; } };\n".repeat(64)).unwrap();
    support::add_asset(&stage, "ui.css", "text/css", b"body { color: var(--slop-accent); }");
    support::add_asset(&stage, &font_key(), "font/woff2", &font());
    stage
}
fn font() -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    bytes[..4].copy_from_slice(b"wOF2");
    bytes
}
fn font_key() -> String {
    support::media_key(&font(), "woff2")
}
fn template(dir: &Path) -> PathBuf {
    let out = dir.join("Template.slop");
    support::pack(&stage(dir), &out).unwrap();
    out
}
fn document(dir: &Path) -> PathBuf {
    let doc = dir.join("Doc.slop");
    file::create_document(&template(dir), &doc).unwrap();
    doc
}
fn raw(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false).unwrap();
    conn
}
fn code(error: Error) -> Code {
    match error {
        Error::Rejected(e) => e.code,
        other => panic!("expected a refusal, got {other:?}"),
    }
}
fn temporaries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
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
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(if color_type == 6 { png::ColorType::Rgba } else { png::ColorType::Rgb });
    encoder.set_depth(png::BitDepth::Eight);
    let channels = if color_type == 6 { 4 } else { 3 };
    encoder.write_header().unwrap().write_image_data(&vec![255; width as usize * height as usize * channels]).unwrap();
    bytes
}
/// Intentionally no pixels: dimensions are refused before decoding a frame.
fn oversized_png_header() -> Vec<u8> {
    let mut bytes = vec![];
    png::Encoder::new(&mut bytes, 100_000, 100_000).write_header().unwrap();
    bytes
}

#[test]
fn a_stage_packs_into_a_template_and_a_rebuild_replaces_only_templates() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    fs::create_dir(stage.join("artwork")).unwrap();
    support::artwork(&stage, "preview", &png(640, 480, 6));
    let (a, b) = (dir.path().join("A.slop"), dir.path().join("B.slop"));
    support::pack(&stage, &a).unwrap();
    support::pack(&stage, &b).unwrap();
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap(), "packing is reproducible");
    let opened = file::open(&a, true).unwrap();
    assert_eq!(opened.kind, Kind::Template);
    assert_eq!((opened.package_format, opened.runtime_abi), (APP.format, APP.abi));
    assert_eq!(opened.app.document_json(), SCHEMA);
    assert_eq!(opened.app.spec().theme_tokens(), [("accent".to_string(), "#335577".to_string())]);
    let metadata = serde_json::to_value(opened.app.metadata()).unwrap();
    assert!(metadata.get("packageFormat").is_none() && metadata.get("runtimeABI").is_none(), "the markers are columns");
    assert_eq!(decoded(&file::artwork(&a, &[Artwork::Preview]).unwrap().unwrap().1), decoded(&png(640, 480, 6)));
    let summary = file::inspect(&a).unwrap();
    assert_eq!(summary["kind"], "template");
    let assets: Vec<&str> = summary["assets"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(assets, [font_key().as_str(), "ui.css", "ui.js"]);
    // A rebuild replaces a template; it never replaces a document, or anything else.
    let folder = dir.path().join("Folder.slop");
    fs::create_dir(&folder).unwrap();
    assert_eq!(code(support::pack(&stage, &folder).unwrap_err()), Code::Exists);
    support::pack(&stage, &a).unwrap();
    let doc = dir.path().join("Doc.slop");
    file::create_document(&a, &doc).unwrap();
    let before = fs::read(&doc).unwrap();
    assert_eq!(code(support::pack(&stage, &doc).unwrap_err()), Code::Exists);
    assert_eq!(fs::read(&doc).unwrap(), before);
    assert!(temporaries(dir.path()).is_empty());
}

/// A real PNG of 8-bit RGBA `pixels`, stored uncompressed.
fn encoded(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::NoCompression);
    encoder.write_header().unwrap().write_image_data(pixels).unwrap();
    bytes
}

#[test]
fn pack_refuses_a_png_header_without_pixels_and_keeps_the_previous_template() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let output = dir.path().join("Before.slop");
    support::pack(&stage, &output).unwrap();
    let before = fs::read(&output).unwrap();
    let image = encoded(2, 2, &[255; 16]);
    fs::create_dir(stage.join("artwork")).unwrap();
    support::artwork(&stage, "preview", &image[..33]);
    assert!(support::pack(&stage, &output).is_err(), "A valid IHDR without pixel data is not an image");
    assert_eq!(fs::read(output).unwrap(), before);
}
/// A PNG's pixels as 8-bit RGBA, whatever colour type it stores.
fn decoded(png: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buffer).unwrap();
    let pixels = &buffer[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => unreachable!("normalizing expands a palette"),
    }
}

/// Packing stores artwork losslessly smaller: every pixel decodes the same, fully
/// transparent ones keep their colour, and packing stays reproducible.
#[test]
fn packed_artwork_is_losslessly_smaller() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let bands = [[0x33, 0x55, 0x77, 255], [0xee, 0xee, 0xe0, 255], [0x10, 0x20, 0x30, 255], [0xff, 0x80, 0, 255]];
    let preview: Vec<u8> = (0..320 * 200usize).flat_map(|i| bands[i % 320 / 80]).collect();
    let icon: Vec<u8> = (0..512 * 512usize)
        .flat_map(|i| if i % 2 == 0 { [(i % 251) as u8, (i * 7 % 256) as u8, 9, 0] } else { bands[0] })
        .collect();
    let inputs =
        [(Artwork::Preview, encoded(320, 200, &preview), preview), (Artwork::Icon, encoded(512, 512, &icon), icon)];
    fs::create_dir(stage.join("artwork")).unwrap();
    for (name, input, _) in &inputs {
        support::artwork(&stage, &name.to_string(), input);
    }
    let (a, b) = (dir.path().join("A.slop"), dir.path().join("B.slop"));
    support::pack(&stage, &a).unwrap();
    support::pack(&stage, &b).unwrap();
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap(), "packing is reproducible");
    for (name, input, pixels) in &inputs {
        let (_, stored) = file::artwork(&a, &[*name]).unwrap().unwrap();
        assert!(stored.len() < input.len(), "the {name} is stored smaller");
        assert_eq!(&decoded(&stored), pixels, "the {name} decodes to the same pixels");
    }
}

/// The `app` row holds one-line JSON whatever the build's spacing, so hosts pass it on
/// as stored; text inside strings is kept exactly, in the descriptor and the initial state.
#[test]
fn pack_stores_the_app_as_compact_json() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let descriptor = "{ \"kind\" : \"object\",\n  \"properties\": { \"title\": { \"kind\": \"string\" } } }";
    let initial = r#"{ "title" : "Spaced \"and\" \\ quoted" }"#;
    write_app(&stage, App { descriptor, initial, ..APP });
    let out = dir.path().join("Compact.slop");
    support::pack(&stage, &out).unwrap();
    let opened = file::open(&out, true).unwrap();
    assert_eq!(opened.app.document_json(), SCHEMA);
    let initial = Store::open(&out, Mode::Snapshot).unwrap().document().unwrap().value();
    assert_eq!(
        initial, r#"{"title":"Spaced \"and\" \\ quoted"}"#,
        "the template's checkpoint holds its initial values"
    );
}

#[test]
fn pack_checks_the_whole_build_before_publishing_anything() {
    type Damage = fn(&Path);
    let cases: [(&str, Damage); 11] = [
        ("no app module", |s| fs::remove_file(s.join("assets/ui.js")).unwrap()),
        ("app module not UTF-8", |s| fs::write(s.join("assets/ui.js"), [0xff, 0xfe]).unwrap()),
        ("descriptor", |s| write_app(s, App { descriptor: r#"{"kind":"nope"}"#, ..APP })),
        ("initial", |s| write_app(s, App { initial: r#"{"title":7}"#, ..APP })),
        ("theme", |s| write_app(s, App { theme: r#"{"accent":"blue"}"#, ..APP })),
        ("metadata", |s| support::edit_input(s, |v| v["declaration"]["metadata"]["title"] = "".into())),
        ("metadata marker", |s| support::edit_input(s, |v| v["declaration"]["metadata"]["runtimeABI"] = 1.into())),
        ("missing markers", |s| {
            support::edit_input(s, |v| {
                v.as_object_mut().unwrap().remove("runtimeABI");
            })
        }),
        ("unknown field", |s| support::edit_input(s, |v| v["extra"] = 1.into())),
        ("symbolic link", |s| {
            fs::remove_file(s.join("assets/ui.js")).unwrap();
            std::os::unix::fs::symlink("/etc/hosts", s.join("assets/ui.js")).unwrap();
        }),
        ("artwork", |s| support::artwork(s, "icon", b"not a png")),
    ];
    for (name, damage) in cases {
        let dir = tempfile::tempdir().unwrap();
        let stage = stage(dir.path());
        damage(&stage);
        let out = dir.path().join("Out.slop");
        assert!(support::pack(&stage, &out).is_err(), "{name}");
        assert!(!out.exists(), "{name}: nothing published");
        assert!(temporaries(dir.path()).is_empty(), "{name}: no temporary file left");
    }
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    fs::write(stage.join("assets/ui.js"), vec![b' '; hitslop_core::ASSET_FILE_BYTES + 1]).unwrap();
    assert!(support::pack(&stage, &dir.path().join("Big.slop")).is_err(), "an asset over 25 MiB");
    write_app(&stage, App { abi: 99, ..APP });

    assert_eq!(code(support::pack(&stage, &dir.path().join("Newer.slop")).unwrap_err()), Code::RequiresUpdate);
}

#[test]
fn a_skin_must_be_an_rgba_asset_at_one_or_two_times_the_window_size() {
    for (width, height, color, accept) in
        [(320, 240, 2, false), (300, 240, 6, false), (320, 240, 6, true), (640, 480, 6, true)]
    {
        let dir = tempfile::tempdir().unwrap();
        let stage = stage(dir.path());
        let bytes = png(width, height, color);
        let key = support::media_key(&bytes, "png");
        support::add_asset(&stage, &key, "image/png", &bytes);
        support::edit_input(&stage, |v| {
            v["roles"]["skin"] = key.clone().into();
            v["declaration"]["window"] =
                serde_json::json!({"kind":"skin","width":320,"height":240,"image":format!("/assets/{key}")});
        });
        let out = dir.path().join("Skin.slop");
        assert_eq!(support::pack(&stage, &out).is_ok(), accept, "{width}×{height} color {color}");
        if accept {
            assert_eq!(file::open(&out, true).unwrap().skin, Some(bytes));
        }
    }
}

#[test]
fn hostile_layouts_and_rows_are_refused_before_any_value_is_read() {
    for ddl in [
        "CREATE TABLE extra(x)",
        "CREATE INDEX extra_index ON assets(bytes)",
        "CREATE VIEW extra_view AS SELECT 1",
        "CREATE TRIGGER extra_trigger AFTER INSERT ON updates BEGIN DELETE FROM attachments; END",
        "INSERT INTO assets VALUES('../escape.js', 'text/javascript', 'identity', 1, x'00')",
        "INSERT INTO assets VALUES('invalid.css', 'text/css', 'identity', 2, x'00')",
        "INSERT INTO assets VALUES('commands.js', 'text/javascript', 'br', 2, x'789c')",
        "INSERT INTO updates(bytes) VALUES(x'00')",
        "DELETE FROM app",
        "PRAGMA ignore_check_constraints=ON; INSERT INTO attachments VALUES('not-an-id', 'application/octet-stream', x'00')",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let doc = document(dir.path());
        let damaged = if ddl.starts_with("INSERT INTO updates") { template(dir.path()) } else { doc.clone() };
        raw(&damaged).execute_batch(&format!("PRAGMA ignore_check_constraints=ON; {ddl}")).unwrap();
        assert!(file::open(&damaged, true).is_err(), "{ddl}");
        assert!(Store::open(&damaged, Mode::Snapshot).is_err(), "{ddl}");
    }
    // STRICT refuses a mistyped size, but a file written without STRICT and then given it
    // back holds one all the same; opens that skip the quick check still refuse it.
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let strict: String =
        raw(&doc).query_row("SELECT sql FROM sqlite_schema WHERE name='assets'", [], |r| r.get(0)).unwrap();
    let rewrite = |sql: &str| {
        let conn = raw(&doc);
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, false).unwrap();
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_WRITABLE_SCHEMA, true).unwrap();
        conn.execute("UPDATE sqlite_schema SET sql=? WHERE name='assets'", [sql]).unwrap();
    };
    rewrite(&strict.replace(" STRICT", ""));
    raw(&doc).execute_batch("PRAGMA ignore_check_constraints=ON; INSERT INTO assets VALUES('unnumbered.js', 'text/javascript', 'identity', 'one', x'00')").unwrap();
    rewrite(&strict);
    assert!(file::open(&doc, false).is_err() && file::open(&doc, true).is_err());
    assert!(Store::open(&doc, Mode::Snapshot).is_err());
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    raw(&doc)
        .execute(
            "INSERT INTO assets VALUES('big.bin', 'application/octet-stream', 'identity', ?1, zeroblob(?1))",
            [hitslop_core::ASSET_FILE_BYTES as i64 + 1],
        )
        .unwrap();
    let started = std::time::Instant::now();
    assert_eq!(code(file::open(&doc, true).err().unwrap()), Code::InvalidRequest);
    assert!(started.elapsed() < std::time::Duration::from_millis(500), "sizes come from length(), not the bytes");
    // A compressed asset is bounded by what it decodes to, before anything is decoded.
    let doc = document(tempfile::tempdir().unwrap().keep().as_path());
    raw(&doc)
        .execute(
            "INSERT INTO assets VALUES('bomb.js', 'text/javascript', 'br', ?, x'789c')",
            [hitslop_core::ASSET_FILE_BYTES as i64 + 1],
        )
        .unwrap();
    assert_eq!(code(file::open(&doc, true).err().unwrap()), Code::InvalidRequest);
    // SQLite's own primary-key indexes are part of the expected layout, not refused.
    let fresh = document(tempfile::tempdir().unwrap().keep().as_path());
    let indexes: i64 = raw(&fresh)
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name LIKE 'sqlite_autoindex_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(indexes > 0);
    file::open(&fresh, true).unwrap();
}

/// A newer app format may change the tables its requirements describe (a column, another
/// asset encoding): this build refuses the file as one it is too old for, never as a
/// damaged one, and leaves it unchanged.
#[test]
fn a_newer_app_format_is_refused_before_its_tables_are_compared() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    raw(&doc)
        .execute_batch(&format!(
            "ALTER TABLE app ADD COLUMN notes TEXT;
             CREATE TABLE wider(key TEXT PRIMARY KEY, media_type TEXT NOT NULL, encoding TEXT NOT NULL CHECK(encoding IN ('identity','br','zstd')), size INTEGER NOT NULL, bytes BLOB NOT NULL);
             INSERT INTO wider SELECT * FROM assets; DROP TABLE assets; ALTER TABLE wider RENAME TO assets;
             UPDATE app SET package_format={};",
            hitslop_core::PACKAGE_FORMAT + 1
        ))
        .unwrap();
    let before = fs::read(&doc).unwrap();
    assert_eq!(code(file::open(&doc, false).err().unwrap()), Code::RequiresUpdate);
    for mode in [Mode::Document, Mode::Snapshot] {
        assert_eq!(code(Store::open(&doc, mode).err().unwrap()), Code::RequiresUpdate);
    }
    assert_eq!(fs::read(&doc).unwrap(), before);
}

/// What pack and a save never write is refused on open, sizes before values: a theme over
/// its budget, and a second or misnumbered row in a one-row table. (Damaged artwork reads
/// as absent instead; see `saved_apps_open_under_their_format...`.)
#[test]
fn stored_values_are_bounded_as_writes_bound_them() {
    let many_tokens =
        format!("{{{}}}", (0..100_000).map(|i| format!(r##""t{i}":"#000000""##)).collect::<Vec<_>>().join(","));
    let cases: [(&str, &str, Option<Vec<u8>>); 4] = [
        (
            "theme defaults over budget",
            "UPDATE app SET definition_json=CAST(? AS TEXT)",
            Some(many_tokens.into_bytes()),
        ),
        (
            "a misnumbered checkpoint",
            "PRAGMA ignore_check_constraints=ON; INSERT INTO checkpoint VALUES(2,x'00')",
            None,
        ),
        ("a second document row", "PRAGMA ignore_check_constraints=ON; INSERT INTO document VALUES(2)", None),
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
        if !refused
            || started.elapsed() > std::time::Duration::from_millis(500)
            || Store::open(&doc, Mode::Snapshot).is_ok()
        {
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
        for refusal in [
            file::open(&doc, true).err(),
            Store::open(&doc, Mode::Document).err(),
            Store::open(&doc, Mode::Snapshot).err(),
        ] {
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
    assert!(snapshot.document().unwrap().value().contains("Initial"), "a template renders its initial values");
    let (a, b) = (dir.path().join("A.slop"), dir.path().join("B.slop"));
    file::create_document(&template, &a).unwrap();
    file::create_document(&template, &b).unwrap();
    assert_eq!(file::open(&a, true).unwrap().kind, Kind::Document);
    assert_eq!(file::open(&b, true).unwrap().kind, Kind::Document);
}

/// The catalog lists each template folder's templates, named for their slugs, and says why
/// it left out every other `.slop` file. A missing folder lists nothing and is not made.
#[test]
fn template_folders_list_their_templates_and_why_others_were_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let (bundled, installed, missing) =
        (dir.path().join("bundled"), dir.path().join("installed"), dir.path().join("missing"));
    let template = template(dir.path());
    for folder in [&bundled, &installed] {
        fs::create_dir_all(folder).unwrap();
        fs::copy(&template, folder.join("checklist.slop")).unwrap();
    }
    fs::copy(&template, installed.join(".hidden.slop")).unwrap();
    fs::copy(&template, installed.join("renamed.slop")).unwrap();
    fs::write(installed.join("notes.txt"), "not a template").unwrap();
    fs::create_dir(installed.join("folder.slop")).unwrap();
    fs::rename(document(dir.path()), installed.join("doc.slop")).unwrap();
    let roots = [
        (TemplateSource::Bundled, bundled.clone()),
        (TemplateSource::Installed, installed.clone()),
        (TemplateSource::Installed, missing.clone()),
    ];
    let catalog = file::list_templates(&roots);
    let folders: Vec<_> =
        roots.iter().map(|(source, path)| file::Folder { source: *source, path: path.clone() }).collect();
    assert_eq!(catalog.folders, folders, "every listed folder, made or not");
    let listed: Vec<_> = catalog.templates.iter().map(|t| (t.slug.as_str(), t.source, t.path.clone())).collect();
    assert_eq!(
        listed,
        [
            ("checklist", TemplateSource::Bundled, fs::canonicalize(&bundled).unwrap().join("checklist.slop")),
            ("checklist", TemplateSource::Installed, fs::canonicalize(&installed).unwrap().join("checklist.slop")),
        ]
    );
    assert_eq!(
        (catalog.templates[0].title.as_str(), &catalog.templates[0].categories[..]),
        ("Checklist", &["utilities".to_owned()][..])
    );
    let left_out: Vec<_> = catalog.issues.iter().map(|issue| issue.split(':').next().unwrap()).collect();
    assert_eq!(left_out, ["doc.slop", "folder.slop", "renamed.slop"]);
    assert!(!missing.exists(), "listing makes no folder");
    file::open_template(&installed.join("checklist.slop")).unwrap();
    for refused in ["doc.slop", "renamed.slop"] {
        assert_eq!(
            code(file::open_template(&installed.join(refused)).err().unwrap()),
            Code::InvalidRequest,
            "{refused}"
        );
    }
}

/// A slug names the listed template with that slug, an installed one before a bundled
/// starter; a name no listed template has, or that is not a bare name, is refused.
#[test]
fn a_slug_names_a_listed_template_and_installed_ones_come_first() {
    let dir = tempfile::tempdir().unwrap();
    let (bundled, installed) = (dir.path().join("bundled"), dir.path().join("installed"));
    let template = template(dir.path());
    for folder in [&bundled, &installed] {
        fs::create_dir_all(folder).unwrap();
        fs::copy(&template, folder.join("checklist.slop")).unwrap();
    }
    fs::copy(&template, installed.join("renamed.slop")).unwrap();
    let both = [(TemplateSource::Bundled, bundled.clone()), (TemplateSource::Installed, installed.clone())];
    assert_eq!(file::find_template("checklist", &both).unwrap(), installed.join("checklist.slop"));
    assert_eq!(file::find_template("checklist", &both[..1]).unwrap(), bundled.join("checklist.slop"));
    for name in ["renamed", "missing", "../installed/checklist", ""] {
        assert_eq!(code(file::find_template(name, &both).unwrap_err()), Code::InvalidRequest, "{name:?}");
    }
    for path in ["Template.slop", "./checklist", "templates/checklist"] {
        assert_eq!(file::template_source(path).unwrap(), Path::new(path), "a path stays a path");
    }
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
        assert_eq!(app.app.definition_json(), read.app.definition_json());
        assert_eq!((app.app.spec().theme_tokens(), &app.skin), (read.app.spec().theme_tokens(), &read.skin));
        assert_eq!((app.app.document_json(), app.app.theme_json()), (read.app.document_json(), read.app.theme_json()));
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
    raw(&template).execute("DELETE FROM assets WHERE key='ui.js'", []).unwrap();
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
    assert_eq!(code(store.copy_clean(&taken, &[]).unwrap_err()), Code::Exists);
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
fn attachments_are_content_addressed_bounded_and_verified() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let first = store.put_attachment(b"hello").unwrap();
    assert_eq!(first.id, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", "SHA-256");
    assert_eq!(store.put_attachment(b"hello").unwrap(), first, "storing the same bytes again is a no-op");
    assert_eq!(store.attachment(&first.id).unwrap(), b"hello");
    assert_eq!(store.attachments().unwrap(), vec![first.clone()]);
    assert_eq!(
        code(store.put_attachment(&vec![0; hitslop_core::ATTACHMENT_FILE_BYTES + 1]).unwrap_err()),
        Code::TooLarge
    );
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
    assert!(
        store.attachment(&stored.id).unwrap_err().to_string().contains("checksum"),
        "the damage is kept for recovery"
    );
    store.close().unwrap();
}

/// Hosts that display a file take the first artwork it holds, in one read, and learn that
/// a file they cannot read now is busy, never that it has no artwork.
#[test]
fn display_reads_fall_back_in_one_read_and_report_a_busy_file() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    fs::create_dir_all(stage.join("artwork")).unwrap();
    support::artwork(&stage, "preview", &png(640, 480, 6));
    let (template, doc) = (dir.path().join("T.slop"), dir.path().join("D.slop"));
    support::pack(&stage, &template).unwrap();
    file::create_document(&template, &doc).unwrap();
    let (name, preview) = file::artwork(&doc, &[Artwork::Icon, Artwork::Preview]).unwrap().unwrap();
    assert_eq!(
        (name, decoded(&preview)),
        (Artwork::Preview, decoded(&png(640, 480, 6))),
        "a build without an icon falls back to its preview"
    );
    assert_eq!(file::artwork(&doc, &[Artwork::Icon]).unwrap(), None);
    let holder = raw(&doc);
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert!(matches!(file::artwork(&doc, &[Artwork::Preview]), Err(Error::Busy)));
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
    assert!(matches!(snapshot.set_artwork(&[(Artwork::Preview, &preview)]), Err(Error::Closed)));
    let store = Store::open(&doc, Mode::Document).unwrap();
    store.set_artwork(&[(Artwork::Preview, &preview), (Artwork::Icon, &icon)]).unwrap();
    assert_eq!(decoded(&store.artwork(Artwork::Icon).unwrap().unwrap()), decoded(&icon));
    store.close().unwrap();
    assert_eq!(decoded(&file::artwork(&doc, &[Artwork::Preview]).unwrap().unwrap().1), decoded(&preview));
}

/// A closing window's capture is stored losslessly smaller.
#[test]
fn written_artwork_is_losslessly_smaller() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let pixels: Vec<u8> = (0..240 * 160usize)
        .flat_map(|i| if i % 240 < 120 { [0x33, 0x55, 0x77, 255] } else { [0xee, 0xee, 0xe0, 255] })
        .collect();
    let (preview, icon) = (encoded(240, 160, &pixels), png(512, 512, 6));
    let store = Store::open(&doc, Mode::Document).unwrap();
    store.set_artwork(&[(Artwork::Preview, &preview), (Artwork::Icon, &icon)]).unwrap();
    let stored = store.artwork(Artwork::Preview).unwrap().unwrap();
    assert!(stored.len() < preview.len());
    assert_eq!(decoded(&stored), pixels);
    assert_eq!(decoded(&store.artwork(Artwork::Icon).unwrap().unwrap()), decoded(&icon));
    store.close().unwrap();
}

/// Each close replaces the artwork; the pages the old images held leave the file.
#[test]
fn replaced_artwork_leaves_no_free_pages() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    // High-entropy pixels retain many pages after optimization, unlike padding after IEND.
    let mut seed = 1u32;
    let pixels: Vec<u8> = (0..512 * 512 * 4)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as u8
        })
        .collect();
    let large = encoded(512, 512, &pixels);
    let small = png(512, 512, 6);
    let store = Store::open(&doc, Mode::Document).unwrap();
    store.set_artwork(&[(Artwork::Preview, &large), (Artwork::Icon, &large)]).unwrap();
    let before = fs::metadata(&doc).unwrap().len();
    store.set_artwork(&[(Artwork::Preview, &small), (Artwork::Icon, &small)]).unwrap();
    assert!(fs::metadata(&doc).unwrap().len() < before);
    store.close().unwrap();
    let free: i64 = raw(&doc).query_row("PRAGMA freelist_count", [], |r| r.get(0)).unwrap();
    assert_eq!(free, 0);
}

/// Replaced values leave no trace in the file on any SQLite, whatever its default: here,
/// artwork small enough to stay in its page.
#[test]
fn replaced_values_leave_no_bytes_behind() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let marker = b"replaced-artwork-7f3a9c1e5b2d";
    let mut first = vec![];
    let mut encoder = png::Encoder::new(&mut first, 64, 64);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.add_text_chunk("Comment".into(), String::from_utf8(marker.to_vec()).unwrap()).unwrap();
    encoder.write_header().unwrap().write_image_data(&[255; 64 * 64 * 4]).unwrap();
    // A legal stored PNG with metadata. The current optimizer strips text at write;
    // seed it directly so the SQLite replacement test actually starts with the marker.
    raw(&doc).execute("INSERT INTO artwork VALUES('preview', ?)", [&first]).unwrap();
    assert!(fs::read(&doc).unwrap().windows(marker.len()).any(|w| w == marker));
    let store = Store::open(&doc, Mode::Document).unwrap();
    store.set_artwork(&[(Artwork::Preview, &png(64, 64, 6))]).unwrap();
    store.close().unwrap();
    assert!(!fs::read(&doc).unwrap().windows(marker.len()).any(|w| w == marker));
}

#[test]
fn an_open_document_copies_through_its_owner() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let mut state = store.document().unwrap();
    state.apply_batch(r#"{"intents":[{"type":"set","path":["title"],"value":"Shared"}]}"#, Origin::Page).unwrap();
    store.write(&store.job(&mut state, false).unwrap().unwrap()).unwrap();
    let photo = store.put_attachment(b"photo").unwrap().id;
    let reference = format!(r#"{{"intents":[{{"type":"set","path":["title"],"value":"Shared {photo}"}}]}}"#);
    state.apply_batch(&reference, Origin::Page).unwrap();
    store.write(&store.job(&mut state, false).unwrap().unwrap()).unwrap();
    let copy = dir.path().join("Copy.slop");
    store.copy_clean(&copy, &[(Artwork::Preview, &png(640, 480, 6))]).unwrap();
    assert_eq!(code(store.copy_clean(&copy, &[]).unwrap_err()), Code::Exists);
    // The owner keeps saving after the copy.
    state.apply_batch(r#"{"intents":[{"type":"set","path":["title"],"value":"Owner"}]}"#, Origin::Page).unwrap();
    store.write(&store.job(&mut state, false).unwrap().unwrap()).unwrap();
    let copied = Store::open(&copy, Mode::Document).unwrap();
    assert!(copied.document().unwrap().value().contains("Shared"));
    assert_eq!(copied.attachments().unwrap().len(), 1);
    assert_eq!(decoded(&copied.artwork(Artwork::Preview).unwrap().unwrap()), decoded(&png(640, 480, 6)));
    // A capture's disposable source skips the syncs and reads the same.
    let source = dir.path().join("Capture.slop");
    store.capture_source(&source).unwrap();
    let captured = Store::open(&source, Mode::Snapshot).unwrap();
    assert!(captured.document().unwrap().value().contains("Owner"));
    assert_eq!(captured.attachments().unwrap().len(), 1);
}

/// A page's asset reader opens the file its store checked: a file put in its place is
/// never served.
#[test]
fn an_asset_reader_reads_only_the_checked_file() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let replacement = document(&dir.path().join("other"));
    for mode in [Mode::Document, Mode::Snapshot] {
        let store = Store::open(&doc, mode).unwrap();
        store.resource_reader().unwrap();
        fs::rename(&doc, dir.path().join("Opened.slop")).unwrap();
        fs::copy(&replacement, &doc).unwrap();
        assert!(matches!(store.resource_reader(), Err(Error::Moved)));
        fs::remove_file(&doc).unwrap();
        fs::rename(dir.path().join("Opened.slop"), &doc).unwrap();
        store.resource_reader().unwrap();
        store.close().unwrap();
    }
}

#[test]
fn commands_are_checked_definitions_and_private_programs() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    support::edit_input(&stage, |v| {
        v["declaration"]["commands"] =
            serde_json::json!([{"name":"addTask","description":"Add a task","args":{"kind":"object","properties":{}}}])
    });
    let template = dir.path().join("Template.slop");
    assert!(support::pack(&stage, &template).is_err());
    support::add_asset(&stage, "commands.js", "text/javascript", b"globalThis.__slopCommands = {};");
    support::pack(&stage, &template).unwrap();
    let app = file::open(&template, true).unwrap();
    assert_eq!(app.app.commands()[0].name, "addTask");
    let reader = Store::open(&template, Mode::Snapshot).unwrap().resource_reader().unwrap();
    for key in ["commands.js", "../commands.js", "media/commands.js"] {
        assert!(reader.info(file::ResourceRoute::App, key).unwrap().is_none());
        assert!(reader.read_range(file::ResourceRoute::App, key, 0, u64::MAX).unwrap().is_none());
    }
}

#[test]
fn resources_are_served_whole_or_in_ranges_with_checked_types() {
    use file::ResourceRoute::App;
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let reader = Store::open(&doc, Mode::Snapshot).unwrap().resource_reader().unwrap();
    let compressed: (i64, i64) = raw(&doc)
        .query_row("SELECT length(bytes),size FROM assets WHERE key='ui.js'", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert!(compressed.0 < compressed.1);
    let script = fs::read(dir.path().join("stage/assets/ui.js")).unwrap();
    for (key, bytes, mime) in [("ui.js".to_owned(), script, "text/javascript"), (font_key(), font(), "font/woff2")] {
        let info = reader.info(App, &key).unwrap().unwrap();
        assert_eq!(info.size, bytes.len() as u64);
        assert_eq!(info.media_type, mime);
        assert_eq!(reader.read_range(App, &key, 0, u64::MAX).unwrap(), Some(bytes.clone()));
        assert_eq!(reader.read_range(App, &key, 100, 50).unwrap(), Some(bytes[100..150].to_vec()));
        assert_eq!(
            reader.read_range(App, &key, bytes.len() as u64 - 10, 500).unwrap(),
            Some(bytes[bytes.len() - 10..].to_vec())
        );
        assert_eq!(reader.read_range(App, &key, u64::MAX, 10).unwrap(), Some(vec![]));
    }
    assert!(reader.info(App, "missing.js").unwrap().is_none());
    assert_eq!(file::descriptor(&doc).unwrap(), SCHEMA);
    raw(&doc).execute("UPDATE assets SET bytes=CAST(x'00' || substr(bytes,2) AS BLOB) WHERE key='ui.js'", []).unwrap();
    // A fresh reader sees corruption; already decoded immutable resources may be cached.
    assert!(file::open(&doc, false).is_err());
}

#[test]
fn artwork_is_checked_as_every_open_checks_it() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let preview = png(640, 480, 6);
    store.set_artwork(&[(Artwork::Preview, &preview)]).unwrap();
    let mut oversized = png(640, 480, 6);
    oversized.resize(hitslop_core::ASSET_FILE_BYTES + 1, 0);
    let huge = oversized_png_header();
    for (name, bytes) in [
        (Artwork::Preview, oversized.as_slice()),
        (Artwork::Icon, b"not a png".as_slice()),
        (Artwork::Icon, b"".as_slice()),
        (Artwork::Icon, huge.as_slice()),
    ] {
        assert!(store.set_artwork(&[(name, bytes)]).is_err(), "{name}: refused");
    }
    store.close().unwrap();
    // The document still opens, with the artwork it had.
    file::open(&doc, true).unwrap();
    assert_eq!(decoded(&file::artwork(&doc, &[Artwork::Preview]).unwrap().unwrap().1), decoded(&preview));
    assert_eq!(file::artwork(&doc, &[Artwork::Icon]).unwrap().map(|(_, png)| png), None);
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
            assert_eq!(
                sidecars_before.iter().any(|(s, _)| s == "-wal"),
                wal,
                "{change}: the newer build's WAL is in place"
            );
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
    let status =
        support::child("crash_mid_commit", &[("HITSLOP_CRASH_DOCUMENT", doc.to_str().unwrap())]).status().unwrap();
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
    let status = support::child("exclusive_sqlite_writer", &[("HITSLOP_EXCLUSIVE_DOCUMENT", doc.to_str().unwrap())])
        .status()
        .unwrap();
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
    let status =
        support::child("crash_mid_commit", &[("HITSLOP_CRASH_DOCUMENT", doc.to_str().unwrap())]).status().unwrap();
    assert!(!status.success(), "the child died mid-commit");
    let journal = dir.path().join("Doc.slop-journal");
    assert!(journal.exists(), "a hot journal is left beside the document");
    let store = Store::open(&doc, Mode::Document).unwrap();
    assert!(!journal.exists(), "the writer rolled it back");
    assert_eq!(store.attachment(&attachment.id).unwrap(), vec![7u8; 4 << 20], "the committed attachment is intact");
}

// Failure: the engine created and edited documents the app refuses to open (a name without
// `.slop`), so where a document may live was the app's rule alone. Oracle: each refusal's
// code, and nothing written in the refused place.
#[test]
fn documents_open_and_go_only_where_the_app_opens_them() {
    let dir = tempfile::tempdir().unwrap();
    let notes = dir.path().join("notes");
    assert_eq!(code(file::create_document(&template(dir.path()), &notes).unwrap_err()), Code::InvalidRequest);
    assert!(!notes.exists());
    let doc = document(dir.path());
    let renamed = dir.path().join("Doc.txt");
    fs::rename(&doc, &renamed).unwrap();
    assert_eq!(code(Store::open(&renamed, Mode::Document).err().unwrap()), Code::InvalidRequest);
    // Reading takes no lock and writes nothing, so a snapshot reads any name.
    Store::open(&renamed, Mode::Snapshot).unwrap().close().unwrap();
    fs::rename(&renamed, &doc).unwrap();
    let store = Store::open(&doc, Mode::Document).unwrap();
    let copy = dir.path().join("Copy");
    assert_eq!(code(store.copy_clean(&copy, &[]).unwrap_err()), Code::InvalidRequest);
    assert!(!copy.exists());
    store.close().unwrap();
}

#[test]
fn the_app_row_seals_assets_against_conflict_and_rowid_bypasses() {
    for as_document in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = if as_document { document(dir.path()) } else { template(dir.path()) };
        // Real connection with triggers enabled, unlike intentional corruption fixtures.
        let conn = Connection::open(&path).unwrap();
        let before = fs::read(&path).unwrap();
        for sql in [
            "INSERT OR REPLACE INTO app SELECT * FROM app",
            "INSERT OR IGNORE INTO app SELECT * FROM app",
            "INSERT INTO app SELECT * FROM app WHERE true ON CONFLICT(id) DO UPDATE SET title='Changed'",
            "UPDATE app SET title='Changed'",
            "DELETE FROM app",
            "INSERT OR REPLACE INTO assets SELECT * FROM assets",
            "INSERT OR REPLACE INTO assets(rowid,key,media_type,encoding,size,bytes) SELECT rowid,'new.js',media_type,encoding,size,bytes FROM assets LIMIT 1",
            "INSERT OR REPLACE INTO assets(rowid,key,media_type,encoding,size,bytes) VALUES(-1,'new.js','text/javascript','identity',1,x'00')",
            "INSERT OR IGNORE INTO assets SELECT * FROM assets",
            "INSERT INTO assets SELECT * FROM assets WHERE true ON CONFLICT(key) DO UPDATE SET bytes=excluded.bytes",
            "INSERT INTO assets VALUES('commands.js','text/javascript','identity',1,x'00')",
            "UPDATE assets SET key='new.js' WHERE key='ui.js'",
            "DELETE FROM assets",
        ] {
            assert!(conn.execute_batch(sql).is_err(), "{sql}");
            assert_eq!(fs::read(&path).unwrap(), before, "{sql} changed the file");
        }
        // Mutable lifecycles remain independent of the seal.
        conn.execute("INSERT INTO attachments VALUES(?, 'application/octet-stream', x'00')", ["a".repeat(64)]).unwrap();
        conn.execute("DELETE FROM attachments", []).unwrap();
        for pixels in [png(2, 2, 6), png(3, 3, 6)] {
            conn.execute("DELETE FROM artwork WHERE name='preview'", []).unwrap();
            conn.execute("INSERT INTO artwork VALUES('preview',?)", [&pixels]).unwrap();
        }
        file::open(&path, true).unwrap();
    }
}

#[test]
fn attachment_urls_check_the_whole_blob_before_serving_even_one_byte() {
    use file::ResourceRoute::{App, Attachment};
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let store = Store::open(&doc, Mode::Document).unwrap();
    let bytes = b"<script>active HTML is not an image</script>";
    let attachment = store.put_attachment(bytes).unwrap();
    let reader = store.resource_reader().unwrap();
    assert_eq!(reader.info(Attachment, &attachment.id).unwrap().unwrap().media_type, "application/octet-stream");
    assert_eq!(reader.read_range(Attachment, &attachment.id, 0, 1).unwrap(), Some(vec![b'<']));
    assert!(reader.info(App, &attachment.id).unwrap().is_none());
    for key in ["ui.js", "../ui.js", "commands.js"] {
        assert!(reader.info(Attachment, key).unwrap().is_none());
    }
    // Corruption beyond the requested range must still be caught on first touch.
    raw(&doc)
        .execute(
            "UPDATE attachments SET bytes=CAST(substr(bytes,1,length(bytes)-1) || x'00' AS BLOB) WHERE id=?",
            [&attachment.id],
        )
        .unwrap();
    let fresh = store.resource_reader().unwrap();
    assert!(fresh.read_range(Attachment, &attachment.id, 0, 1).is_err());
    raw(&doc)
        .execute(
            "UPDATE attachments SET bytes=?,media_type='image/png' WHERE id=?",
            rusqlite::params![bytes, attachment.id],
        )
        .unwrap();
    assert!(store.resource_reader().unwrap().info(Attachment, &attachment.id).is_err());
    store.close().unwrap();
}

/// A saved app is judged by its package format's acceptance, never by today's authoring
/// rules: metadata `init` would now refuse and a damaged preview still open, edit, save
/// and reopen. Artwork is cosmetic and reads as absent.
#[test]
fn saved_apps_open_under_their_format_and_damaged_artwork_reads_as_absent() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let title = "A title longer than any author may now write. ".repeat(4);
    raw(&doc)
        .execute("UPDATE app SET title=?, author_name=' ', author_url='not a URL', slug='Saved_Before'", [&title])
        .unwrap();
    raw(&doc).execute("INSERT OR REPLACE INTO artwork VALUES('preview', x'89504e470d0a1a0a00')", []).unwrap();
    raw(&doc).execute("INSERT OR REPLACE INTO artwork VALUES('icon', zeroblob(16))", []).unwrap();
    let summary = file::summary(&doc).unwrap();
    assert_eq!((summary.metadata.title.as_str(), summary.metadata.slug.as_str()), (title.as_str(), "Saved_Before"));
    assert!(hitslop_core::app::validate_metadata(&summary.metadata).is_err(), "authoring refuses it");
    assert_eq!(file::artwork(&doc, &[Artwork::Preview, Artwork::Icon]).unwrap(), None);
    let store = Store::open(&doc, Mode::Document).unwrap();
    assert_eq!(store.artwork(Artwork::Preview).unwrap(), None);
    let mut state = store.document().unwrap();
    state.apply_batch(r#"{"intents":[{"type":"set","path":["title"],"value":"Edited"}]}"#, Origin::Page).unwrap();
    store.write(&store.job(&mut state, false).unwrap().unwrap()).unwrap();
    store.close().unwrap();
    let reopened = Store::open(&doc, Mode::Snapshot).unwrap();
    assert!(reopened.document().unwrap().value().contains("Edited"));
    assert_eq!(reopened.app().app.metadata().title, title);
}
