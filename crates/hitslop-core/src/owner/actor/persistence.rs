//! Serial storage effects, driven by the native thread or browser worker.
use super::*;

pub(super) enum Work {
    Save {
        generation: u64,
        target: u64,
        job: store::SaveJob,
    },
    Restore {
        generation: u64,
        callback: Completion,
    },
    Store {
        generation: u64,
        action: StorageAction,
        callback: Completion,
    },
    Close {
        generation: u64,
        job: Option<store::SaveJob>,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        callback: Completion,
    },
    /// Publishes a verified copy of the live state at `dest`, then releases the store
    /// without saving.
    #[cfg(not(target_arch = "wasm32"))]
    Handoff {
        generation: u64,
        dest: PathBuf,
        state: Vec<u8>,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        value: serde_json::Value,
        theme: std::collections::BTreeMap<String, String>,
        callback: Completion,
    },
}
/// The artwork a window rendered, by name, leaving out what it did not render.
#[cfg(not(target_arch = "wasm32"))]
fn named_artwork<'a>(preview: &'a Option<Vec<u8>>, icon: &'a Option<Vec<u8>>) -> Vec<(Artwork, &'a [u8])> {
    [(Artwork::Preview, preview), (Artwork::Icon, icon)]
        .into_iter()
        .filter_map(|(name, png)| Some((name, png.as_deref()?)))
        .collect()
}
#[cfg_attr(
    target_arch = "wasm32",
    expect(dead_code, reason = "native path payloads reach the persistence queue but are refused in the browser")
)]
pub(super) enum StorageAction {
    Copy {
        path: PathBuf,
        state: Vec<u8>,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    CaptureSource(PathBuf),
    Backup(PathBuf),
    Artwork(Artwork),
    #[cfg(not(target_arch = "wasm32"))]
    SetArtwork {
        source_version: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
    },
    Attachments,
    ReadAttachment(String),
    PutAttachment(Vec<u8>),
    Share(store::Share),
}
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn persistence(store: Arc<store::Store>, work: mpsc::Receiver<Work>, sender: mpsc::Sender<Message>) {
    for work in work {
        let done = perform(&store, work);
        if let Err(mpsc::SendError(Message::Stored { callback, .. } | Message::Restored { callback, .. })) =
            sender.send(done)
        {
            complete(callback, Err(closed()))
        }
    }
}
/// One serial storage effect, driven by a native thread or a browser worker.
pub(super) fn perform(store: &store::Store, work: Work) -> Message {
    match work {
        Work::Save { generation, target, job } => {
            Message::Saved { generation, target, result: contained(|| store.write(&job).map_err(Failure::from)) }
        }
        Work::Restore { generation, callback } => Message::Restored {
            generation,
            callback,
            result: contained(|| store.document().map(Box::new).map_err(Failure::from)),
        },
        Work::Store { generation, action, callback } => {
            let refresh_status = matches!(action, StorageAction::Share(_));
            let result = contained(|| {
                Ok(match action {
                    #[cfg(not(target_arch = "wasm32"))]
                    StorageAction::Copy { path, state, preview, icon } => {
                        store.copy_clean(&path, &state, &named_artwork(&preview, &icon))?;
                        Reply::Unit
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    StorageAction::CaptureSource(path) => {
                        store.capture_source(&path)?;
                        Reply::Unit
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    StorageAction::Backup(path) => {
                        store.backup(&path)?;
                        Reply::Unit
                    }
                    #[cfg(target_arch = "wasm32")]
                    StorageAction::Copy { .. } | StorageAction::CaptureSource(_) | StorageAction::Backup(_) => {
                        return Err(Failure::rejected(
                            Code::InvalidRequest,
                            "Native path operation is unavailable in the browser",
                        ));
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    StorageAction::SetArtwork { source_version, preview, icon } => {
                        if store.document()?.reading()?.version != source_version {
                            return Err(Failure::new(FailureKind::Replaced, "Artwork source changed"));
                        }
                        store.set_artwork(&named_artwork(&preview, &icon))?;
                        Reply::Unit
                    }
                    StorageAction::Artwork(name) => Reply::Bytes { bytes: store.artwork(name)? },
                    StorageAction::Attachments => Reply::Attachments { items: store.attachments()? },
                    StorageAction::ReadAttachment(id) => Reply::Bytes { bytes: Some(store.attachment(&id)?) },
                    StorageAction::PutAttachment(bytes) => Reply::Attachment { item: store.put_attachment(&bytes)? },
                    StorageAction::Share(share) => {
                        store.set_share(&share)?;
                        Reply::Unit
                    }
                })
            });
            Message::Stored { generation, result, callback, closing: false, refresh_status }
        }
        Work::Close { generation, job, preview, icon, callback } => {
            let result = contained(|| {
                // Housekeeping cannot fail a close whose final save succeeded.
                if let Some(job) = job {
                    let _ = store.write(&job);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let artwork = named_artwork(&preview, &icon);
                    if !artwork.is_empty() {
                        let _ = store.set_artwork(&artwork);
                    }
                }
                #[cfg(target_arch = "wasm32")]
                let _ = (preview, icon);
                // After the final save, when no import can be waiting for its
                // reference: the page's barrier drained its imports, and an agent's
                // blobs arrive in the batch that references them.
                let _ = store.reclaim_attachments();
                store.close()?;
                Ok(Reply::Unit)
            });
            Message::Stored { generation, result, callback, closing: true, refresh_status: false }
        }
        #[cfg(not(target_arch = "wasm32"))]
        Work::Handoff { generation, dest, state, preview, icon, value, theme, callback } => {
            let result = contained(|| {
                hand_off(store, &dest, &state, &named_artwork(&preview, &icon), &value, &theme)?;
                store.close()?;
                Ok(Reply::Unit)
            });
            Message::Stored { generation, result, callback, closing: true, refresh_status: false }
        }
    }
}
/// Publishes the live `state` at `dest` as a document of its own, never replacing anything:
/// a clean copy is staged beside `dest`, must open as a document holding the live `value`
/// and palette, and only then is published. The staged file is removed unless published.
/// A published copy whose folder sync failed stays in place and is reported as an unknown
/// outcome, with the owner still open.
#[cfg(not(target_arch = "wasm32"))]
fn hand_off(
    store: &store::Store,
    dest: &Path,
    state: &[u8],
    artwork: &[(Artwork, &[u8])],
    value: &serde_json::Value,
    theme: &std::collections::BTreeMap<String, String>,
) -> Result<()> {
    file::document_destination(dest)?;
    let dest = file::resolve(dest)?;
    if std::fs::symlink_metadata(&dest).is_ok() {
        return Err(Failure::rejected(Code::Exists, "A file already exists there"));
    }
    let staged = file::Staged::beside(&dest)?;
    store.copy_clean_unplaced(staged.path(), state, artwork)?;
    verify_copy(staged.path(), value, theme)?;
    Ok(staged.publish_new(&dest, true)?.confirmed()?)
}
/// Refuses unless `copy` opens as a document holding the live `value` and palette, so a
/// handoff can never lose what the copy does not have.
#[cfg(not(target_arch = "wasm32"))]
fn verify_copy(
    copy: &Path,
    value: &serde_json::Value,
    theme: &std::collections::BTreeMap<String, String>,
) -> Result<()> {
    let unusable =
        |why: String| Failure::new(FailureKind::Failed, format!("The copy cannot replace this document: {why}"));
    let snapshot = store::Store::open(copy, store::Mode::Snapshot).map_err(|e| unusable(e.to_string()))?;
    let reading =
        snapshot.document().map_err(|e| e.to_string()).and_then(|document| document.reading().map_err(|e| e.message));
    let _ = snapshot.close();
    let reading = reading.map_err(unusable)?;
    if reading.value != *value || reading.theme != *theme {
        return Err(unusable("it does not match the document".into()));
    }
    Ok(())
}
