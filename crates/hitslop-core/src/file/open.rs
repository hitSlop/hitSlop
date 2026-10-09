//! Bounded summaries and fully checked application reads.
use super::assets::read_asset;
use super::check::{contents, layout, markers, one};
#[cfg(not(target_arch = "wasm32"))]
use super::{Artwork, reader, resolve};
use super::{Kind, assets, rows};
use crate::app::{AppDefinition, AppMetadata, Author, WindowFrame};
#[cfg(not(target_arch = "wasm32"))]
use crate::error::failed;
use crate::error::{Error, Result, invalid, sqlite};
use crate::wire::MANIFEST_BYTES;
use rusqlite::Connection;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::Path;

/// Display metadata is deliberately not a certificate of full acceptance.
#[derive(Clone, Debug)]
pub struct Summary {
    pub kind: Kind,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub metadata: AppMetadata,
    pub bytes: u64,
}
/// Display metadata, bounded; the definition's decoder owns every other rule.
fn scalar_metadata(conn: &Connection) -> Result<AppMetadata> {
    let size = one(
        conn,
        "SELECT length(CAST(slug AS BLOB))+length(CAST(title AS BLOB))+length(CAST(description AS BLOB))+length(CAST(author_name AS BLOB))+coalesce(length(CAST(author_url AS BLOB)),0)+length(CAST(category_primary AS BLOB))+coalesce(length(CAST(category_secondary AS BLOB)),0) FROM app WHERE id=1",
    )?;
    if size > MANIFEST_BYTES as i64 {
        return Err(invalid("App metadata is too large"));
    }
    let (slug,title,description,name,url,primary,secondary) = conn.query_row(
        "SELECT slug,title,description,author_name,author_url,category_primary,category_secondary FROM app WHERE id=1", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?))
    ).map_err(sqlite("read metadata"))?;
    use crate::app::package_format_1::column_category;
    let mut categories = vec![column_category(&primary).map_err(Error::Rejected)?];
    if let Some(secondary) = secondary {
        categories.push(column_category(&secondary).map_err(Error::Rejected)?);
    }
    Ok(AppMetadata { slug, title, description, author: Author { name, url }, categories })
}
/// Common display preamble: marker dispatch, exact layout and bounded catalog fields.
/// No definition, app blob or attachment blob (even its length) is read here.
fn summary_on(conn: &Connection, path: &Path) -> Result<Summary> {
    let version = markers(conn)?;
    layout(conn, version)?;
    let (package_format, runtime_abi) = conn
        .query_row("SELECT package_format,runtime_abi FROM app WHERE id=1", [], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64))
        })
        .map_err(sqlite("read markers"))?;
    let metadata = scalar_metadata(conn)?;
    if one(conn, "SELECT count(*) FROM checkpoint")? != 1 {
        return Err(invalid("The file has no saved state; keep it for recovery"));
    }
    let kind = if one(conn, "SELECT count(*) FROM document")? == 0 { Kind::Template } else { Kind::Document };
    Ok(Summary { kind, package_format, runtime_abi, metadata, bytes: stored_bytes(conn, path)? })
}
#[cfg(not(target_arch = "wasm32"))]
pub fn summary(path: &Path) -> Result<Summary> {
    let conn = reader(path)?;
    let read = conn.unchecked_transaction().map_err(sqlite("read"))?;
    summary_on(&read, path)
}

/// Fully accepted app and resources, kept by the owner for its entire lifetime.
pub struct OpenedApp {
    pub kind: Kind,
    /// A document's immutable logical UUID. Templates have none; independent copies get
    /// their own. This is never used as a Loro peer or as the file's writer-lock identity.
    pub document_uuid: Option<String>,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub app: AppDefinition,
    pub skin: Option<Vec<u8>>,
    pub style: bool,
    pub commands: Option<std::sync::Arc<String>>,
    pub bytes: u64,
}
pub(crate) fn opened(conn: &Connection, path: &Path, integrity: bool) -> Result<OpenedApp> {
    let _read = if conn.is_autocommit() { Some(conn.unchecked_transaction().map_err(sqlite("read"))?) } else { None };
    let summary = summary_on(conn, path)?;
    contents(conn, integrity)?;
    let definition: String = conn
        .query_row("SELECT definition_json FROM app WHERE id=1", [], |r| r.get(0))
        .map_err(sqlite("read definition"))?;
    let app = AppDefinition::decode(
        summary.package_format,
        summary.runtime_abi,
        &crate::encode(&summary.metadata),
        &definition,
    )
    .map_err(Error::Rejected)?;
    let mut keys = conn.prepare("SELECT key FROM assets ORDER BY key").map_err(sqlite("read assets"))?;
    let mut ui = false;
    let mut style = false;
    let mut skin = None;
    let mut commands = None;
    for key in keys.query_map([], |r| r.get::<_, String>(0)).map_err(sqlite("read assets"))? {
        let key = key.map_err(sqlite("read assets"))?;
        let kind = crate::media::asset_key(&key).ok_or_else(|| invalid("Invalid app asset key"))?;
        let bytes = read_asset(conn, &key)?.ok_or_else(|| invalid("Missing app asset"))?;
        // `pack` checked each asset's signature and the app row seals them; a later
        // build's signature rules never judge a saved app. Its content address is a
        // damage check that cannot change meaning.
        assets::check_resource(&key, &bytes, integrity)?;
        if key == "ui.css" {
            style = true;
        }
        if key == "ui.js" {
            ui = true;
        }
        if let WindowFrame::Skin { skin: skin_key } = &app.window().frame
            && &key == skin_key
        {
            if kind.media_type != "image/png" {
                return Err(invalid("Window skins must be PNG"));
            }
            crate::images::check(
                &bytes,
                crate::images::Purpose::Skin { width: app.window().width, height: app.window().height },
            )
            .map_err(Error::Rejected)?;
            skin = Some(bytes.clone());
        }
        if key == "commands.js" {
            commands = Some(std::sync::Arc::new(String::from_utf8(bytes).map_err(invalid)?));
        }
    }
    if !ui {
        return Err(invalid("Missing ui.js"));
    }
    if matches!(app.window().frame, WindowFrame::Skin { .. }) && skin.is_none() {
        return Err(invalid("Missing window skin"));
    }
    if commands.is_some() == app.commands().is_empty() {
        return Err(invalid("Commands and their private program must be present together"));
    }
    Ok(OpenedApp {
        kind: summary.kind,
        document_uuid: rows::document_uuid(conn)?,
        package_format: summary.package_format,
        runtime_abi: summary.runtime_abi,
        app,
        skin,
        style,
        commands,
        bytes: summary.bytes,
    })
}
#[cfg(not(target_arch = "wasm32"))]
pub fn open(path: &Path, integrity: bool) -> Result<OpenedApp> {
    opened(&reader(path)?, path, integrity)
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn checked(path: &Path) -> Result<(Connection, Kind)> {
    let conn = reader(path)?;
    let kind = summary_on(&conn, path)?.kind;
    Ok((conn, kind))
}
#[cfg(not(target_arch = "wasm32"))]
pub fn artwork(path: &Path, preferred: &[Artwork]) -> Result<Option<(Artwork, Vec<u8>)>> {
    let conn = reader(path)?;
    let read = conn.unchecked_transaction().map_err(sqlite("read"))?;
    summary_on(&read, path)?;
    for &name in preferred {
        if let Some(png) = rows::read_artwork(&read, name)? {
            return Ok(Some((name, png)));
        }
    }
    Ok(None)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn kind(path: &Path) -> Result<Kind> {
    Ok(summary(path)?.kind)
}
/// Copies existing cosmetic artwork without running the app or changing its file.
/// A caller provides a fresh output path; existing files are never overwritten.
#[cfg(not(target_arch = "wasm32"))]
pub fn export_artwork(path: &Path, name: Artwork, output: &Path) -> Result<bool> {
    use std::io::Write;
    let Some((_, png)) = artwork(path, &[name])? else {
        return Ok(false);
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| failed(format!("Create artwork output: {e}")))?;
    file.write_all(&png).map_err(|e| failed(format!("Write artwork output: {e}")))?;
    Ok(true)
}

/// A summary for `slop inspect`: kind, markers, assets, artwork and the document's sizes,
/// of a file every open would accept.
#[cfg(not(target_arch = "wasm32"))]
pub fn inspect(path: &Path) -> Result<crate::engine::InspectInfo> {
    use crate::engine::{AttachmentTotals, FileKind, InspectInfo, NamedSize, StateSizes};
    let conn = reader(path)?;
    // The summary reads the state the checks passed.
    let conn = conn.unchecked_transaction().map_err(sqlite("read"))?;
    let opened = opened(&conn, path, true)?;
    let (updates, update_bytes, checkpoint_bytes) = rows::state_sizes(&conn)?;
    let (attachments, _, attachment_bytes) = rows::attachment_sizes(&conn)?;
    let size = |value: i64| u64::try_from(value).map_err(|_| invalid("Negative stored size"));
    let list = |sql: &str| -> Result<Vec<NamedSize>> {
        let mut statement = conn.prepare(sql).map_err(sqlite("inspect"))?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(sqlite("inspect"))?;
        rows.map(|row| {
            let (name, bytes) = row.map_err(sqlite("inspect"))?;
            Ok(NamedSize { name, bytes: size(bytes)? })
        })
        .collect()
    };
    Ok(InspectInfo {
        kind: match opened.kind {
            Kind::Template => FileKind::Template,
            Kind::Document => FileKind::Document,
        },
        package_format: opened.package_format,
        runtime_abi: opened.runtime_abi,
        metadata: opened.app.metadata().clone(),
        window: opened.app.page_window(),
        views: opened.app.views(),
        assets: list("SELECT key, size FROM assets ORDER BY key")?,
        artwork: list("SELECT name, length(png) FROM artwork ORDER BY name")?,
        defaults: opened.app.spec().theme_tokens().iter().cloned().collect(),
        attachments: AttachmentTotals { count: size(attachments)?, bytes: size(attachment_bytes)? },
        state: StateSizes {
            checkpoint_bytes: size(checkpoint_bytes)?,
            updates: size(updates)?,
            update_bytes: size(update_bytes)?,
        },
        bytes: opened.bytes,
        stored_asset_bytes: size(
            conn.query_row("SELECT coalesce(sum(length(bytes)),0) FROM assets", [], |r| r.get(0))
                .map_err(sqlite("inspect"))?,
        )?,
        live: crate::registry::discovery(path)?.is_some(),
    })
}

/// The app's descriptor, for `slop schema`, from a file every open would accept.
#[cfg(not(target_arch = "wasm32"))]
pub fn descriptor(path: &Path) -> Result<String> {
    Ok(open(path, false)?.app.document_json().into())
}

#[cfg(not(target_arch = "wasm32"))]
fn stored_bytes(_: &Connection, path: &Path) -> Result<u64> {
    Ok(fs::metadata(resolve(path)?).map(|m| m.len()).unwrap_or(0))
}
#[cfg(target_arch = "wasm32")]
fn stored_bytes(conn: &Connection, _: &Path) -> Result<u64> {
    Ok((one(conn, "PRAGMA page_count")? * one(conn, "PRAGMA page_size")?) as u64)
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    use crate::file::{APPLICATION_ID, SCHEMA};
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    #[test]
    fn summary_cannot_read_the_definition_or_app_and_document_payloads() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(&format!("PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version=1;
            INSERT INTO checkpoint VALUES(1,x'00');
            INSERT INTO assets VALUES('ui.js','text/javascript','identity',1,x'00');
            INSERT INTO app VALUES(1,1,1,'fixture','Fixture','A fixture','Author',NULL,'utilities',NULL,'invalid JSON');")).unwrap();
        conn.authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Read { table_name, column_name }
                if table_name == "assets"
                    || table_name == "attachments"
                    || table_name == "updates"
                    || (table_name == "app" && column_name == "definition_json")
                    || (table_name == "checkpoint" && column_name == "bytes") =>
            {
                Authorization::Deny
            }
            _ => Authorization::Allow,
        }))
        .unwrap();
        for query in [
            "SELECT definition_json FROM app",
            "SELECT bytes FROM assets",
            "SELECT bytes FROM attachments",
            "SELECT bytes FROM checkpoint",
        ] {
            assert!(conn.prepare(query).is_err(), "authorizer must refuse {query}");
        }
        let summary = summary_on(&conn, Path::new("unused.slop")).unwrap();
        assert_eq!(summary.kind, Kind::Template);
        assert_eq!(summary.metadata.title, "Fixture");
        // An unreadable payload does not make a summary a validity certificate.
        assert!(opened(&conn, Path::new("unused.slop"), false).is_err());
    }
}
