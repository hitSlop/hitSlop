//! The document file: packing a build, the checks every open runs, creating and copying
//! documents, the writer lock and discovery, attachments, artwork, assets and recovery.
use crate::support;
use pop_core::{Code, Origin};
use pop_storage::file::{self, Artwork, Kind};
use pop_storage::registry::Lease;
use pop_storage::store::{Error, Mode, Store};
use rusqlite::{Connection, config::DbConfig};
use std::fs;
use std::path::{Path, PathBuf};
use support::ApplyJson;
use support::{App, isolate_registry, write_app};

const SCHEMA: &str = r#"{"kind":"object","properties":{"title":{"kind":"string"}}}"#;
const INITIAL: &str = r#"{"title":"Initial"}"#;
const APP: App = App::new(SCHEMA, INITIAL);

#[test]
fn a_capture_batch_keeps_successful_outputs_when_other_roles_fail() {
    use pop_capture::capture::{CaptureFormat, CaptureOutput, CaptureValue, render_batch};
    for has_widget in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let source = stage(directory.path());
        if has_widget {
            support::edit_input(&source, |input| {
                input["declaration"]["views"]["widget"] = serde_json::json!({});
                input["roles"]["widget"] = "widget.js".into();
            });
            support::add_asset(&source, "widget.js", "text/javascript", b"globalThis.__popWidget = () => '[]';");
        }
        let template = directory.path().join("template.pop");
        support::pack(&source, &template).unwrap();
        let document = directory.path().join("document.pop");
        file::create_document(&template, &document).unwrap();
        let saved = fs::read(&document).unwrap();
        let image = png(512, 512, 6);
        let reply = serde_json::json!({"ok": true, "images": [data_encoding::BASE64.encode(&image)]});
        let widget_images: Vec<_> =
            pop_capture::widgets::widget_viewports().iter().map(|v| png(v.width * 2, v.height * 2, 6)).collect();
        let widgets = serde_json::json!({"ok": true, "images": widget_images.iter()
            .map(|bytes| data_encoding::BASE64.encode(bytes)).collect::<Vec<_>>()});
        let markup =
            serde_json::json!({"frames":[{"kind":"preview","width":512,"height":512,"html":"","css":""}],"fonts":[]});
        let widget_markup = serde_json::json!({"frames": pop_capture::widgets::widget_viewports().iter().map(|v| serde_json::json!({"width":v.width,"height":v.height,"html":"","css":""})).collect::<Vec<_>>(),"fonts":[]});
        let evaluator = pop_runner::Evaluator::process(
            "/bin/sh".into(),
            vec!["-c".into(), format!(r#"input=$(/bin/cat); case "$input" in *'"mode":"markup"'*) printf '%s' '{markup}';; *'"mode":"widgetMarkup"'*) printf '%s' '{widget_markup}';; *'"mode":"widget"'*) printf '%s' '{widgets}';; *) printf '%s' '{reply}';; esac"#)],
        ).unwrap();
        let outputs = [
            CaptureOutput::Preview,
            CaptureOutput::Widgets,
            CaptureOutput::Export { format: CaptureFormat::Pdf }, // Child returned the wrong media.
            CaptureOutput::Icon,
        ];
        let results = render_batch(&document, &evaluator, &outputs).unwrap();
        assert_eq!(results.iter().map(|result| result.output).collect::<Vec<_>>(), outputs);
        for result in &results {
            match result.output {
                CaptureOutput::Preview | CaptureOutput::Icon => {
                    let Ok(CaptureValue::Image { bytes: Some(bytes) }) = &result.result else {
                        panic!("valid image must survive another role's failure")
                    };
                    assert_eq!(bytes, &image);
                }
                CaptureOutput::Widgets if has_widget => {
                    let Ok(CaptureValue::Widgets { capture }) = &result.result else {
                        panic!("valid widgets must survive another role's failure")
                    };
                    assert_eq!(
                        capture.images.iter().map(|i| &i.png).collect::<Vec<_>>(),
                        widget_images.iter().collect::<Vec<_>>()
                    );
                    assert_eq!(
                        capture.source_version,
                        pop_capture::widgets::saved_version(&document, &evaluator).unwrap()
                    );
                }
                _ => assert!(result.result.is_err()),
            }
        }
        assert_eq!(fs::read(&document).unwrap(), saved);
    }
}

/// A metadata-only markup stand-in and a bounded raster reply. The shell never
/// evaluates authored code, so these tests isolate core image admission and reads.
fn image_markup(ids: &[String], icon_ids: &[String]) -> pop_runner::Evaluator {
    image_markup_before(ids, icon_ids, ":")
}
fn image_markup_before(ids: &[String], icon_ids: &[String], before: &str) -> pop_runner::Evaluator {
    let frame = |kind: &str, ids: &[String]| {
        serde_json::json!({"frames":[{
        "html":ids.iter().step_by(2).map(|id| format!("<img src=\"pop-attachment:{id}\">" )).collect::<String>(),
        "css":ids.iter().skip(1).step_by(2).map(|id| format!(".photo{{background:url(\"pop-attachment:{id}\")}}" )).collect::<String>(),
        "width":512,"height":512,"kind":kind}],"fonts":[]})
    };
    let preview = frame("preview", ids);
    let icon = frame("icon", icon_ids);
    let widgets = serde_json::json!({"frames":pop_capture::widgets::widget_viewports().iter().map(|v|serde_json::json!({"width":v.width,"height":v.height,"html":"","css":""})).collect::<Vec<_>>(),"fonts":[]});
    let reply = serde_json::json!({"ok":true,"images":[data_encoding::BASE64.encode(&png(512,512,6))]});
    let widget_reply = serde_json::json!({"ok":true,"images":pop_capture::widgets::widget_viewports().iter().map(|v|data_encoding::BASE64.encode(&png(v.width*2,v.height*2,6))).collect::<Vec<_>>()});
    // Only the small request prefix selects a reply; drain image bytes without
    // copying or pattern-matching megabytes in the shell.
    let script = format!(
        r#"input=$(/usr/bin/head -c 4096); /bin/cat >/dev/null; {before}; case "$input" in *'"mode":"markup"'*) case "$input" in *'\"kind\":\"icon\"'*) printf '%s' '{icon}';; *) printf '%s' '{preview}';; esac;; *'"mode":"widgetMarkup"'*) printf '%s' '{widgets}';; *'"mode":"widget"'*) printf '%s' '{widget_reply}';; *) printf '%s' '{reply}';; esac"#
    );
    pop_runner::Evaluator::process("/bin/sh".into(), vec!["-c".into(), script]).unwrap()
}
fn capture_document(dir: &Path) -> PathBuf {
    let source = stage(dir);
    support::edit_input(&source, |input| {
        input["declaration"]["views"]["widget"] = serde_json::json!({});
        input["roles"]["widget"] = "widget.js".into();
    });
    support::add_asset(&source, "widget.js", "text/javascript", b"globalThis.__popWidget = () => '{}';");
    let template = dir.join("template.pop");
    support::pack(&source, &template).unwrap();
    let doc = dir.join("document.pop");
    file::create_document(&template, &doc).unwrap();
    doc
}
fn saved_images(path: &Path, count: usize, padded: bool) -> Vec<String> {
    let store = Store::open(path, Mode::Document).unwrap();
    let ids: Vec<_> = (0..count)
        .map(|i| {
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            if padded {
                // Large encoded bytes, one pixel to decode. An unknown ancillary
                // chunk keeps the PNG valid without adding raster work.
                writer.write_chunk(png::chunk::ChunkType(*b"teSt"), &vec![0; 6 << 20]).unwrap();
            }
            writer.write_image_data(&[i as u8, 0, 0, 255]).unwrap();
            writer.finish().unwrap();
            store.put_attachment(&bytes).unwrap().id
        })
        .collect();
    let mut doc = store.document().unwrap();
    doc.apply_json(
        &serde_json::json!({"intents":[{"type":"set","path":["title"],"value":ids.join(" ")}]}).to_string(),
        Origin::Page,
    )
    .unwrap();
    store.write(&store.job(&mut doc).unwrap().unwrap()).unwrap();
    store.close().unwrap();
    ids
}
#[test]
fn capture_damaged_unused_image_never_blocks_outputs() {
    use pop_capture::capture::{CaptureFormat, CaptureOutput, render_batch};
    let dir = tempfile::tempdir().unwrap();
    let doc = capture_document(dir.path());
    let ids = saved_images(&doc, 1, false);
    raw(&doc).execute("UPDATE attachments SET bytes=zeroblob(length(bytes))", []).unwrap();
    let evaluator = image_markup(&[], &[]);
    let results = render_batch(
        &doc,
        &evaluator,
        &[
            CaptureOutput::Preview,
            CaptureOutput::Icon,
            CaptureOutput::Export { format: CaptureFormat::Png },
            CaptureOutput::Widgets,
        ],
    )
    .unwrap();
    for result in results {
        assert!(result.result.is_ok(), "unused {}: {:?}", ids[0], result.result.err());
    }
}
#[test]
fn capture_used_damage_names_attachment_and_preserves_sibling() {
    use pop_capture::capture::{CaptureOutput, render_batch};
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let ids = saved_images(&doc, 1, false);
    raw(&doc).execute("UPDATE attachments SET bytes=zeroblob(length(bytes))", []).unwrap();
    // Preview references the damaged image; Icon has independent empty markup.
    let evaluator = image_markup(&ids, &[]);
    let results = render_batch(&doc, &evaluator, &[CaptureOutput::Preview, CaptureOutput::Icon]).unwrap();
    let mut results = results.into_iter();
    let error = results.next().unwrap().result.err().unwrap().to_string();
    assert!(error.contains(&ids[0]), "{error}");
    assert!(error.contains("Preview"), "{error}");
    assert!(results.next().unwrap().result.is_ok());
}
#[test]
fn capture_image_budget_is_per_output() {
    use pop_capture::capture::{CaptureOutput, render_batch};
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let ids = saved_images(&doc, 4, true); // Just over 24 MiB in state, just over 12 MiB per view.
    let store = Store::open(&doc, Mode::Snapshot).unwrap();
    let sizes: Vec<_> = store.attachments().unwrap().iter().map(|image| image.bytes).collect();
    assert_eq!(sizes.len(), 4);
    assert!(sizes.iter().sum::<u64>() > pop_runner::IMAGE_BYTES as u64);
    assert!(sizes.iter().sum::<u64>() / 2 < pop_runner::IMAGE_BYTES as u64);
    store.close().unwrap();
    let results =
        render_batch(&doc, &image_markup(&ids[..2], &ids[2..]), &[CaptureOutput::Preview, CaptureOutput::Icon])
            .unwrap();
    for result in results {
        assert!(result.result.is_ok(), "{:?}", result.result.as_ref().err());
    }
    let results = render_batch(&doc, &image_markup(&ids, &[]), &[CaptureOutput::Preview]).unwrap();
    let error = results.into_iter().next().unwrap().result.err().unwrap();
    assert_eq!(code(error.clone()), Code::TooLarge);
    assert!(error.to_string().contains("Preview"));
}
#[test]
fn capture_markup_outside_image_inventory_names_the_id() {
    use pop_capture::capture::{CaptureOutput, render_batch};
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let unknown = "a".repeat(64);
    let results =
        render_batch(&doc, &image_markup(std::slice::from_ref(&unknown), &[]), &[CaptureOutput::Preview]).unwrap();
    let error = results.into_iter().next().unwrap().result.err().unwrap().to_string();
    assert!(error.contains(&unknown) && error.contains("inventory"), "{error}");
}

#[test]
fn capture_attachment_removed_during_markup_fails_only_its_output() {
    use pop_capture::capture::{CaptureOutput, render_batch};
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let ids = saved_images(&doc, 1, false);
    let before = format!("/usr/bin/sqlite3 '{}' 'DELETE FROM attachments'", doc.display());
    let results =
        render_batch(&doc, &image_markup_before(&ids, &[], &before), &[CaptureOutput::Preview, CaptureOutput::Icon])
            .unwrap();
    let mut results = results.into_iter();
    let error = results.next().unwrap().result.err().unwrap().to_string();
    assert!(error.contains(&ids[0]) && error.contains("Preview"), "{error}");
    assert!(results.next().unwrap().result.is_ok());
}

/// A closed export to `output`, as a PNG, with a deadline it never reaches.
fn export(document: &Path, output: &Path, evaluator: &pop_runner::Evaluator) -> Result<(), Error> {
    use pop_capture::capture::{CaptureFormat, export_saved};
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    export_saved(document, CaptureFormat::Png, output, evaluator, deadline)
}
/// A renderer that leaves `marker` behind, so a test can tell whether it ran.
fn marking(marker: &Path) -> pop_runner::Evaluator {
    support::renderer(&format!(": > '{}'", marker.display()), &png(64, 48, 6))
}

/// An export never replaces anything at its output under any spelling, and refuses before
/// any authored code renders.
#[test]
fn closed_export_refuses_an_existing_output_before_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let rendered = dir.path().join("rendered");
    let file = dir.path().join("file.png");
    fs::write(&file, "an earlier export").unwrap();
    let folder = dir.path().join("folder.png");
    fs::create_dir(&folder).unwrap();
    let (link, dangling, missing) =
        (dir.path().join("link.png"), dir.path().join("dangling.png"), dir.path().join("missing.png"));
    std::os::unix::fs::symlink(&file, &link).unwrap();
    std::os::unix::fs::symlink(&missing, &dangling).unwrap();
    let mut outputs = vec![file.clone(), folder.clone(), link.clone(), dangling.clone()];
    // The Mac's file systems usually ignore case: another spelling of the same file.
    let alias = dir.path().join("FILE.png");
    if alias.exists() {
        outputs.push(alias);
    }
    for output in outputs {
        assert_eq!(code(export(&doc, &output, &marking(&rendered)).unwrap_err()), Code::Exists, "{}", output.display());
    }
    assert_eq!(fs::read_to_string(&file).unwrap(), "an earlier export");
    assert!(folder.is_dir() && fs::read_dir(&folder).unwrap().next().is_none());
    assert_eq!((fs::read_link(&link).unwrap(), fs::read_link(&dangling).unwrap()), (file, missing.clone()));
    assert!(!missing.exists());
    assert!(!rendered.exists(), "refused before rendering");
    assert!(temporaries(dir.path()).is_empty());
}

/// An export publishes exactly the image its renderer made, once it passed the checks every
/// capture passes, and leaves the document's bytes and its writer lock alone.
#[test]
fn closed_export_publishes_only_a_checked_image_and_never_writes_the_document() {
    use pop_capture::capture::{CaptureFormat, export_saved};
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let saved = fs::read(&doc).unwrap();
    let image = png(64, 48, 6);
    let output = dir.path().join("Export.png");
    export(&doc, &output, &support::renderer(":", &image)).unwrap();
    assert_eq!(fs::read(&output).unwrap(), image);
    // Bytes that are not the format asked for: no PNG, and a PNG where a PDF was asked.
    let refused = dir.path().join("Refused");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    for (format, bytes) in [(CaptureFormat::Png, b"not a PNG".to_vec()), (CaptureFormat::Pdf, image)] {
        assert!(export_saved(&doc, format, &refused, &support::renderer(":", &bytes), deadline).is_err());
        assert!(!refused.exists());
    }
    assert_eq!(fs::read(&doc).unwrap(), saved);
    assert!(Lease::acquire(&doc).is_ok(), "an export takes no writer lock");
    assert!(temporaries(dir.path()).is_empty());
}

/// A result that arrives after its command's deadline is discarded, not published late.
#[test]
fn a_closed_export_past_its_deadline_publishes_nothing() {
    use pop_capture::capture::{CaptureFormat, export_saved};
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let (rendered, output) = (dir.path().join("rendered"), dir.path().join("Late.png"));
    let late = export_saved(&doc, CaptureFormat::Png, &output, &marking(&rendered), std::time::Instant::now());
    assert!(late.unwrap_err().to_string().contains("not published"));
    assert!(rendered.exists(), "the renderer answered, too late");
    assert!(!output.exists());
    assert!(temporaries(dir.path()).is_empty());
}

#[test]
fn capture_export_bytes_are_complete_and_existing_outputs_are_never_replaced() {
    use pop_capture::capture::publish_export;
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("Export.png");
    let bytes = png(64, 48, 6);
    publish_export(&bytes, &output, || true).unwrap();
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert_eq!(code(publish_export(b"replacement", &output, || true).unwrap_err()), Code::Exists);
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert!(temporaries(dir.path()).is_empty());
}

#[test]
fn capture_export_accepts_long_destination_names_without_leaving_staging_files() {
    for length in [240, 255] {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join(format!("{}.png", "e".repeat(length - 4)));
        let bytes = png(64, 48, 6);
        pop_capture::capture::publish_export(&bytes, &output, || true).unwrap();
        assert_eq!(fs::read(&output).unwrap(), bytes);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(temporaries(dir.path()).is_empty());
    }
}

#[test]
fn capture_export_cancelled_after_staging_publishes_nothing() {
    use pop_capture::capture::publish_export;
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("Late.png");
    let result = publish_export(b"complete staged bytes", &output, || {
        assert!(!output.exists());
        let paths: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
        assert_eq!(paths.len(), 1);
        assert_eq!(fs::read(&paths[0]).unwrap(), b"complete staged bytes");
        false
    });
    assert!(result.unwrap_err().to_string().contains("not published"));
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[test]
fn capture_export_failed_write_removes_the_partial_staging_file() {
    let dir = tempfile::tempdir().unwrap();
    let status = support::child(
        "file::capture_export_write_limit_child",
        &[("POP_EXPORT_WRITE_LIMIT", dir.path().to_str().unwrap())],
    )
    .status()
    .unwrap();
    assert!(status.success(), "limited writer failed: {status}");
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none(), "no destination or partial staging file");
}

#[test]
#[ignore = "child process with a file-size limit"]
fn capture_export_write_limit_child() {
    let Ok(folder) = std::env::var("POP_EXPORT_WRITE_LIMIT") else { return };
    // A real write fault after four bytes; isolate this process-wide limit in a child.
    // SAFETY: SIG_IGN is valid; the limit points to an initialized rlimit.
    unsafe {
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &libc::rlimit { rlim_cur: 4, rlim_max: 4 }), 0);
    }
    let result =
        pop_capture::capture::publish_export(b"more than four bytes", &Path::new(&folder).join("Export.png"), || {
            panic!("a failed write must never reach publication")
        });
    assert!(result.unwrap_err().to_string().contains("Cannot write the export"));
}

/// A file made at the output while the export rendered wins: the export refuses rather
/// than replace it.
#[test]
fn a_file_made_at_the_output_while_rendering_is_never_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let doc = document(dir.path());
    let output = dir.path().join("Raced.png");
    let racing = support::renderer(&format!("printf theirs > '{}'", output.display()), &png(64, 48, 6));
    assert_eq!(code(export(&doc, &output, &racing).unwrap_err()), Code::Exists);
    assert_eq!(fs::read_to_string(&output).unwrap(), "theirs");
    assert!(temporaries(dir.path()).is_empty());
}

/// A template exports through a document made from it, and an app without an Export view
/// has nothing to export; neither runs its renderer.
#[test]
fn closed_export_refuses_templates_and_apps_without_an_export_view() {
    let dir = tempfile::tempdir().unwrap();
    let (rendered, output) = (dir.path().join("rendered"), dir.path().join("Export.png"));
    let template = template(dir.path());
    assert_eq!(code(export(&template, &output, &marking(&rendered)).unwrap_err()), Code::IsTemplate);
    support::edit_input(&dir.path().join("stage"), |input| {
        input["declaration"]["views"]["export"] = false.into();
        input["roles"].as_object_mut().unwrap().remove("export");
        input["resources"].as_array_mut().unwrap().retain(|resource| resource["key"] != "export.js");
    });
    let plain = dir.path().join("Plain.pop");
    support::pack(&dir.path().join("stage"), &plain).unwrap();
    let doc = dir.path().join("Plain document.pop");
    file::create_document(&plain, &doc).unwrap();
    let error = export(&doc, &output, &marking(&rendered)).unwrap_err();
    assert!(error.to_string().contains("Export.svelte"), "{error}");
    assert_eq!(code(error), Code::InvalidRequest);
    assert!(!output.exists());
    assert!(!rendered.exists());
}

#[test]
fn widget_capture_version_changes_when_the_renderer_changes_without_editing_the_document() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let source = stage(directory.path());
    support::edit_input(&source, |input| {
        input["declaration"]["views"]["widget"] = serde_json::json!({});
        input["roles"]["widget"] = "widget.js".into();
    });
    support::add_asset(&source, "widget.js", "text/javascript", b"globalThis.__popWidget = () => '[]';");
    let template = directory.path().join("widget.pop");
    support::pack(&source, &template).unwrap();
    let document = directory.path().join("document.pop");
    file::create_document(&template, &document).unwrap();
    let saved = fs::read(&document).unwrap();
    let images: Vec<_> = pop_capture::widgets::widget_viewports()
        .iter()
        .map(|view| data_encoding::BASE64.encode(&png(view.width * 2, view.height * 2, 6)))
        .collect();
    let reply = serde_json::json!({"ok": true, "images": images});
    let batch = serde_json::json!({"frames":pop_capture::widgets::widget_viewports().iter().map(|v| serde_json::json!({"width":v.width,"height":v.height,"html":"","css":""})).collect::<Vec<_>>(),"fonts":[]});
    let program = format!(
        r#"input=$(/bin/cat); case "$input" in *'"mode":"widgetMarkup"'*) printf '%s' '{batch}';; *) printf '%s' '{reply}';; esac"#
    );
    let executable = directory.path().join("evaluator");
    symlink("/bin/sh", &executable).unwrap();
    let evaluator = pop_runner::Evaluator::process(executable.clone(), vec!["-c".into(), program.clone()]).unwrap();
    let first = pop_capture::widgets::render_saved(&document, &evaluator).unwrap();
    let repeated = pop_capture::widgets::render_saved(&document, &evaluator).unwrap();
    assert_eq!(first.source_version, repeated.source_version);
    let replacement = directory.path().join("replacement");
    symlink("/bin/bash", &replacement).unwrap();
    fs::rename(&replacement, &executable).unwrap();
    let upgraded = pop_capture::widgets::render_saved(&document, &evaluator).unwrap();
    assert_ne!(first.source_version, upgraded.source_version, "new renderer must invalidate unchanged widget state");
    assert_eq!(pop_capture::widgets::saved_version(&document, &evaluator).unwrap(), upgraded.source_version);
    // Replacing a helper at the same path with identical bytes does not need a
    // new generation, but a capture that actually crosses a replacement is refused.
    symlink("/bin/bash", &replacement).unwrap();
    fs::rename(&replacement, &executable).unwrap();
    assert_eq!(pop_capture::widgets::saved_version(&document, &evaluator).unwrap(), upgraded.source_version);
    let evaluator = pop_runner::Evaluator::process(
        executable.clone(),
        vec!["-c".into(), format!("/bin/ln -sf /bin/sh '{}'; {program}", executable.display())],
    )
    .unwrap();
    let error =
        pop_capture::widgets::render_saved(&document, &evaluator).err().expect("replacement must refuse capture");
    assert!(error.to_string().contains("renderer changed during capture"), "{error}");
    assert_eq!(fs::read(&document).unwrap(), saved, "capture must never modify the document");
}
/// What a crashed owner leaves behind: its discovery file, with its lock released.
fn stage(dir: &Path) -> PathBuf {
    isolate_registry();
    let stage = dir.join("stage");
    fs::create_dir_all(stage.join("assets")).unwrap();
    write_app(&stage, APP);
    fs::write(stage.join("assets/ui.js"), "export default { mount() { return {}; } };\n".repeat(64)).unwrap();
    support::add_asset(&stage, "ui.css", "text/css", b"body { color: var(--pop-accent); }");
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
    let out = dir.join("Template.pop");
    support::pack(&stage(dir), &out).unwrap();
    out
}
fn document(dir: &Path) -> PathBuf {
    let doc = dir.join("Doc.pop");
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
fn encoded(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::NoCompression);
    encoder.write_header().unwrap().write_image_data(pixels).unwrap();
    bytes
}

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
fn render_of(shade: u8) -> (Vec<u8>, Vec<u8>) {
    let pixels: Vec<u8> = (0..512 * 512usize).flat_map(|i| [shade, (i % 251) as u8, 0x77, 255]).collect();
    (encoded(512, 512, &pixels), pixels)
}

/// Rendered artwork is stored exactly as supplied artwork is: checked, then optimized, so a
/// template packed with a render stores the bytes one packed with that render as its
/// artwork would.
#[test]
fn rendered_artwork_is_stored_as_supplied_artwork_is() {
    use pop_core::engine::{ArtworkSource::*, PackedArtwork};
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let ((raw, pixels), asked) = (render_of(1), std::cell::RefCell::new(vec![]));
    let rendered = dir.path().join("Rendered.pop");
    let render = renders(&asked, |_| Some(raw.clone()));
    let packed = support::pack_rendering(&stage, &rendered, Some(&render)).unwrap();
    assert_eq!(packed, PackedArtwork { preview: Rendered, icon: Rendered });
    // Through the capture renderer too, which renders the staged template's initial values.
    let evaluator = support::renderer_outputs(":", &png(512, 512, 6), b"%PDF-1.7\n%%EOF");
    let capture = |template: &Path, roles: &[Artwork]| pop_capture::capture::render_views(template, &evaluator, roles);
    let captured = dir.path().join("Captured.pop");
    support::pack_rendering(&stage, &captured, Some(&capture)).unwrap();
    for (render, template) in [(&raw, &rendered), (&png(512, 512, 6), &captured)] {
        for name in Artwork::ALL {
            support::artwork(&stage, &name.to_string(), render);
        }
        let supplied = dir.path().join("Supplied.pop");
        assert_eq!(
            support::pack_rendering(&stage, &supplied, None).unwrap(),
            PackedArtwork { preview: Supplied, icon: Supplied }
        );
        for name in Artwork::ALL {
            let stored = file::artwork(template, &[name]).unwrap().unwrap().1;
            assert_eq!(stored, file::artwork(&supplied, &[name]).unwrap().unwrap().1, "the {name}");
        }
    }
    let stored = file::artwork(&rendered, &[Artwork::Icon]).unwrap().unwrap().1;
    assert!(stored.len() < raw.len(), "stored optimized");
    assert_eq!(decoded(&stored), pixels);
    assert!(temporaries(dir.path()).is_empty());
}

/// Supplied artwork wins role by role; the callback still validates views when both
/// images are supplied, with no artwork requested for storage.
#[test]
fn an_icon_that_fails_to_render_fails_the_pack() {
    let dir = tempfile::tempdir().unwrap();
    let stage = stage(dir.path());
    let existing = dir.path().join("Existing.pop");
    support::pack(&stage, &existing).unwrap();
    let before = fs::read(&existing).unwrap();
    // A rendered image is checked as supplied artwork is: an icon is 512 × 512.
    let asked = std::cell::RefCell::new(vec![]);
    let small = renders(&asked, |_| Some(png(64, 48, 6)));
    let error = support::pack_rendering(&stage, &existing, Some(&small)).unwrap_err();
    assert!(error.to_string().contains("artwork/icon.png"), "{error}");
    // The capture renderer's own icon failure fails its batch.
    let evaluator = support::renderer(":", &png(64, 48, 6));
    let capture = |template: &Path, roles: &[Artwork]| pop_capture::capture::render_views(template, &evaluator, roles);
    let error = support::pack_rendering(&stage, &existing, Some(&capture)).unwrap_err();
    assert!(error.to_string().contains("512"), "{error}");
    assert_eq!(fs::read(&existing).unwrap(), before);
    assert!(temporaries(dir.path()).is_empty());
}

type RenderedArtwork = Result<Vec<(Artwork, Option<Vec<u8>>)>, Error>;
/// A stand-in for the capture renderer: it draws each role asked for with `draw` and notes
/// each batch it was asked for, from the staged template.
fn renders<'a>(
    asked: &'a std::cell::RefCell<Vec<Vec<Artwork>>>,
    draw: impl Fn(Artwork) -> Option<Vec<u8>> + 'a,
) -> impl Fn(&Path, &[Artwork]) -> RenderedArtwork + 'a {
    move |template, roles| {
        assert_eq!(file::open(template, true).unwrap().kind, Kind::Template, "renders the staged template");
        asked.borrow_mut().push(roles.to_vec());
        Ok(roles.iter().map(|&name| (name, draw(name))).collect())
    }
}
