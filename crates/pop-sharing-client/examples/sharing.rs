//! Disposable fixture driver for the deployed sharing check. Never ships in the app.
//! Invitations and credentials are written only to owner-readable files, never stdout.
use pop_library::snapshot;
use pop_runtime::owner::{Owner, Reply, Request};
use pop_sharing_client::Client;
use pop_storage::{file, store::Mode};
use serde::Deserialize;
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
#[derive(Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
enum Control {
    Attach { path: PathBuf },
    State,
    Edit { batch: pop_core::Batch },
    Export { path: PathBuf },
    Pause,
    Resume,
    Stop,
    Quit,
}
fn save(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new().write(true).truncate(true).create(true).mode(0o600).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn reply(value: serde_json::Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    let template = Path::new(args.get(1).ok_or("template required")?);
    let root = Path::new(args.get(2).ok_or("output folder required")?);
    let secret = Path::new(args.get(3).ok_or("Firebase token file required")?);
    let guest = args.get(4).is_some_and(|value| value == "--guest");
    std::fs::create_dir_all(root)?;
    pop_storage::registry::use_folder(&root.join("registry"))?;
    let path = root.join("native.pop");
    let invitation = if guest { Some(std::fs::read_to_string(secret)?) } else { None };
    if let Some(link) = &invitation {
        pop_sharing_client::install(&path, link.trim(), &root.join("relay-state"))?;
    } else {
        file::create_document(template, &path)?;
    }
    let owner = Arc::new(Owner::open(&path, Mode::Document, Arc::new(|_| {}))?);
    let credentials = root.join("credentials.json");
    let token = if guest { String::new() } else { std::fs::read_to_string(secret)? };
    let published = if let Some(link) = &invitation {
        pop_sharing_client::joined_credential(link.trim())?
    } else {
        // This disposable local fixture has no CloudKit catalog authority.
        let prepared = pop_sharing_client::reserve(owner.app().document_uuid.as_deref().unwrap(), token.trim())?;
        prepared.activate(&owner, token.trim(), None, &root.join("relay-state"))?.credential
    };
    save(&credentials, published.as_bytes())?;
    let client =
        Client::restore(owner.clone(), &published, &root.join("relay-state"), (!guest).then_some(token.trim()))?;
    client.set_creator(!guest);
    let link = client.status().link.ok_or("No invitation")?;
    save(&root.join("invitation.txt"), link.as_bytes())?;
    client.connect()?;
    reply(serde_json::json!({"ready":true,"document":owner.app().document_uuid,"path":path}));
    for line in std::io::stdin().lock().lines() {
        let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            match serde_json::from_str::<Control>(&line?)? {
                Control::Attach { path } => {
                    let Reply::Attachment { item } =
                        snapshot::request(&owner, Request::PutAttachment { bytes: std::fs::read(path)? })?
                    else {
                        return Err("Attachment missing".into());
                    };
                    return Ok(serde_json::json!({"attachment":item.id}));
                }
                Control::Quit => return Ok(serde_json::json!({"quit":true})),
                Control::Edit { batch } => {
                    snapshot::request(&owner, Request::Apply { batch, origin: pop_core::Origin::Agent })?;
                }
                Control::Export { path } => {
                    snapshot::request(&owner, Request::Backup { destination: path })?;
                }
                Control::Stop => {
                    if guest {
                        return Err("Stop the disposable room in its signed-in owner app".into());
                    }
                    let frozen = client.freeze(token.trim())?;
                    let ended = client.finalize(token.trim(), &frozen, None)?;
                    save(&credentials, ended.as_bytes())?;
                }
                Control::Pause => client.pause(),
                Control::Resume => client.connect()?,
                Control::State => {}
            }
            let Reply::State { reading, .. } = snapshot::request(&owner, Request::State)? else {
                return Err("State missing".into());
            };
            Ok(serde_json::json!({"state":reading,"phase":format!("{:?}",client.status().phase)}))
        })();
        match result {
            Ok(value) => {
                let quit = value.get("quit").is_some();
                reply(value);
                if quit {
                    break;
                }
            }
            Err(_) => reply(serde_json::json!({"error":"Fixture operation failed"})),
        }
    }
    client.pause();
    snapshot::request(&owner, Request::Close { preview: None, icon: None })?;
    Ok(())
}
