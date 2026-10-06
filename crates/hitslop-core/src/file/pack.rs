//! Packing a build's stage into a template, and the authoring check of its app values.

use super::artwork::{check_artwork, optimize_png};
use super::assets::{assets_within, encode, valid_asset_path};
use super::copy::Staged;
use super::{APPLICATION_ID, App, Artwork, Kind, SCHEMA, STORAGE_VERSION};
use super::{check, check_app, check_app_values, configure_writer, reader, requirements, rows, writer};
use crate::error::{Error, Result, failed, invalid, sqlite};
use crate::wire::{APP_TEXT_BYTES, MANIFEST_BYTES};
use std::borrow::Cow;
use std::fs;
use std::path::Path;

/// Writes a template: the app's row, its initial checkpoint, its assets and its artwork, in
/// one transaction.
fn write_template(
    path: &Path,
    app: &App,
    initial: &[u8],
    assets: &[(String, Vec<u8>)],
    artwork: &[(Artwork, Vec<u8>)],
) -> Result<()> {
    let conn = writer(path, true)?;
    conn.execute_batch("PRAGMA auto_vacuum=FULL;").map_err(sqlite("create"))?;
    configure_writer(&conn)?;
    let tx = super::initialize(&conn)?;
    tx.execute_batch(&format!(
        "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={STORAGE_VERSION}; {SCHEMA}"
    ))
    .map_err(sqlite("create"))?;
    rows::put_app(&tx, app)?;
    rows::put_checkpoint(&tx, initial)?;
    for (key, bytes) in assets {
        let (encoding, stored) = encode(key, bytes)?;
        rows::put_asset(&tx, key, encoding, bytes.len(), &stored)?;
    }
    for (name, png) in artwork {
        rows::put_artwork(&tx, *name, png)?;
    }
    tx.commit()?;
    conn.close().map_err(|(_, e)| sqlite("close")(e))
}

/// Packs a build's stage into a template at `dest`. The stage is the build's folder:
/// `app.json` (the `app` row: requirements, manifest, descriptor and theme, and the initial
/// values its checkpoint is made from), `assets/` and optional `artwork/preview.png` and
/// `artwork/icon.png`. Everything is checked before the template is published; a rebuild
/// replaces a template, never a document.
pub fn pack(stage: &Path, dest: &Path) -> Result<()> {
    let file = stage.join("app.json");
    let size = fs::metadata(&file).map_err(|e| invalid(format!("app.json: {e}")))?.len();
    if size > APP_INPUT_BYTES as u64 {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    let text = fs::read_to_string(&file).map_err(|e| invalid(format!("app.json: {e}")))?;
    let (app, initial) = parse_app(&text)?;
    let assets = stage_assets(&stage.join("assets"))?;
    let mut artwork = vec![];
    for name in Artwork::ALL {
        let file = stage.join("artwork").join(format!("{name}.png"));
        match fs::symlink_metadata(&file) {
            Ok(meta) if meta.is_file() => {
                let bytes = fs::read(&file).map_err(failed)?;
                check_artwork(&format!("artwork/{name}.png"), &bytes)?;
                // Packing runs once per build, so it can afford oxipng's default level, 2.
                artwork.push((name, optimize_png(bytes, 2)));
            }
            Ok(_) => return Err(invalid(format!("artwork/{name}.png must be a regular file"))),
            Err(_) => {}
        }
    }
    let lookup = |key: &str| Ok(assets.iter().find(|(k, _)| k == key).map(|(_, b)| Cow::Borrowed(b.as_slice())));
    let checked = check_app(&app, &lookup)?;
    let initial = initial_checkpoint(&checked.spec, &initial)?;
    let staged = Staged::beside(dest)?;
    write_template(staged.path(), &app, &initial, &assets, &artwork)?;
    if check(&reader(staged.path())?, true)? != Kind::Template {
        return Err(failed("Packing produced document state"));
    }
    staged.publish_template(dest)
}
/// Maximum evaluated app row accepted by packing and authoring validation.
pub const APP_INPUT_BYTES: usize = MANIFEST_BYTES + 2 * APP_TEXT_BYTES + crate::wire::THEME_LIMIT;

/// The app row and its initial values, from a build's `app.json`.
fn parse_app(input: &str) -> Result<(App, String)> {
    if input.len() > APP_INPUT_BYTES {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    let row: crate::wire::AppRow = serde_json::from_str(input).map_err(|e| invalid(format!("app.json: {e}")))?;
    requirements(row.packageFormat.try_into().unwrap_or(i64::MAX), row.runtimeABI.try_into().unwrap_or(i64::MAX))?;
    crate::manifest::validate(row.manifest.get()).map_err(Error::Rejected)?;
    if row.manifest.get().len() > MANIFEST_BYTES
        || row.descriptor.get().len() > APP_TEXT_BYTES
        || row.initial.get().len() > APP_TEXT_BYTES
        || row.theme.get().len() > crate::wire::THEME_LIMIT
    {
        return Err(invalid("The app's manifest, schema, initial values or theme is too large"));
    }
    let app = App {
        package_format: row.packageFormat,
        runtime_abi: row.runtimeABI,
        manifest: compact(row.manifest.get()),
        descriptor: compact(row.descriptor.get()),
        theme: compact(row.theme.get()),
    };
    Ok((app, row.initial.get().to_owned()))
}
/// The template's initial state: `initial`, checked against the app, as its checkpoint.
fn initial_checkpoint(app: &crate::AppSpec, initial: &str) -> Result<Vec<u8>> {
    let checkpoint = crate::Document::initial_checkpoint(app, initial).map_err(Error::Rejected)?;
    if checkpoint.len() + 512 > crate::STORAGE_BYTES {
        return Err(invalid("The app's initial values are too large"));
    }
    Ok(checkpoint)
}
/// Checks evaluated app values with the same rules as packing and opening; assets are
/// checked later when they exist. Refuses unsupported markers before interpreting values.
pub fn validate_app(input: &str) -> Result<()> {
    let (app, initial) = parse_app(input)?;
    let (_, spec, _) = check_app_values(&app)?;
    initial_checkpoint(&spec, &initial).map(|_| ())
}
/// Parsed JSON text without the whitespace between its tokens, in the order written: the
/// `app` row's text is one line, so hosts pass it on without re-encoding it.
fn compact(json: &str) -> String {
    let (mut out, mut quoted, mut escaped) = (String::with_capacity(json.len()), false, false);
    for c in json.chars() {
        if quoted {
            (escaped, quoted) = (!escaped && c == '\\', escaped || c != '"');
        } else if c.is_ascii_whitespace() {
            continue;
        } else {
            quoted = c == '"';
        }
        out.push(c);
    }
    out
}
/// The stage's assets: regular files only, within the budgets.
fn stage_assets(root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut assets = vec![];
    let (mut count, mut largest, mut total) = (0usize, 0usize, 0usize);
    let mut pending = vec![root.to_owned()];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).map_err(|e| invalid(format!("assets: {e}")))? {
            let path = entry.map_err(failed)?.path();
            let meta = fs::symlink_metadata(&path).map_err(failed)?;
            if meta.is_dir() {
                pending.push(path);
                continue;
            }
            if !meta.is_file() {
                return Err(invalid("Assets must be regular files, without symbolic links"));
            }
            let key = path
                .strip_prefix(root)
                .map_err(|_| invalid("An asset escapes its folder"))?
                .to_string_lossy()
                .into_owned();
            if !valid_asset_path(&key) {
                return Err(invalid(format!("Unsafe asset path {key}")));
            }
            // Checked before reading, so a stage of huge files is refused, never loaded.
            (count, largest, total) = (count + 1, largest.max(meta.len() as usize), total + meta.len() as usize);
            assets_within(count, largest, total)?;
            assets.push((key, fs::read(&path).map_err(failed)?));
        }
    }
    assets.sort();
    Ok(assets)
}
